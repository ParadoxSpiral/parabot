// Copyright (C) 2017  ParadoxSpiral
//
// This file is part of parabot.
//
// Parabot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Parabot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Parabot.  If not, see <http://www.gnu.org/licenses/>.

use encoding::DecoderTrap;
use encoding::label::encoding_from_whatwg_label;
use html5ever;
use html5ever::rcdom::{NodeData, RcDom, Handle};
use html5ever::tendril::TendrilSink;
use humansize::{FileSize, file_size_opts as Options};
use percent_encoding::percent_decode;
use reqwest;
use reqwest::header::{ContentLength, ContentType, Headers};
use reqwest::mime::{Attr, Value};
use reqwest::Response;
use serde_json;
use serde_json::Value as JValue;
use wolfram_alpha::query;

use std::borrow::Borrow;
use std::io::{Cursor, Read};

use config::ServerCfg;
use errors::*;

pub fn handle(cfg: &ServerCfg, mut response: Response, regex_match: bool) -> Result<String> {
    let domain = response.url().domain().unwrap().to_owned();

    // Invoke either site specific or generic handler
    if domain.ends_with("youtube.com") || domain.ends_with("youtu.be") {
        let path = response.url().path_segments().unwrap().last().unwrap();
        let mut query = response.url().query_pairs();
        if path == "watch" || domain.ends_with("youtu.be") {
            let mut body = String::new();
            let v = query
                .find(|&(ref k, _)| if k == "v" { true } else { false })
                .unwrap()
                .1;
            let mut resp = reqwest::get(&format!(
                "https://www.googleapis.com/youtube/v3/videos?part=status,snippet,contentDetails,\
                 statistics&key={}&id={}",
                cfg.youtube_key.as_ref().unwrap(),
                if domain.ends_with("youtube.com") {
                    v.as_ref()
                } else {
                    path
                }
            ))?;
            resp.read_to_string(&mut body)?;
            let resp: JValue = serde_json::from_str(&body)?;
            let channel = resp.pointer("/items/0/snippet/channelTitle")
                .unwrap()
                .as_str()
                .unwrap();
            let title = resp.pointer("/items/0/snippet/title")
                .unwrap()
                .as_str()
                .unwrap();
            let duration = resp.pointer("/items/0/contentDetails/duration")
                .unwrap()
                .as_str()
                .unwrap();
            let definition = resp.pointer("/items/0/contentDetails/definition")
                .unwrap()
                .as_str()
                .unwrap();
            let dimension = resp.pointer("/items/0/contentDetails/dimension")
                .unwrap()
                .as_str()
                .unwrap();
            let restricted = resp.pointer("/items/0/contentDetails/regionRestriction/blocked")
                .is_some();
            let ratings_disabled = !resp.pointer("/items/0/status/publicStatsViewable")
                .unwrap()
                .as_bool()
                .unwrap();
            let views = resp.pointer("/items/0/statistics/viewCount")
                .unwrap()
                .as_str()
                .unwrap();
            Ok(format!(
                "^ {} [{}] ({}) {} views {}{}{}",
                title,
                duration.replace('P', "").replace('T', "").to_lowercase(),
                channel,
                pretty_number(views),
                definition.to_uppercase(),
                {
                    if dimension == "3d" {
                        " (3D)"
                    } else {
                        ""
                    }
                },
                {
                    if restricted && ratings_disabled {
                        " [Region restricted|Ratings disabled]"
                    } else if restricted {
                        " [Region restricted]"
                    } else if ratings_disabled  {
                        " [Ratings disabled]"
                    } else {
                        ""
                    }
                }
            ))
        } else if path == "results" {
            unimplemented!()
        } else {
            unimplemented!("{}, {:?}", path, query)
        }
    } else if domain.ends_with("wolframalpha.com") {
        let query = percent_decode(response.url().query().unwrap().as_bytes())
            .decode_utf8()?;
        let query: &str = query.borrow();
        assert_eq!("i=", &query[..2]);
        let resp = query::query(
            None,
            cfg.wolframalpha_appid.as_ref().unwrap(),
            &query[2..],
            Some(query::QueryParameters {
                includepodid: Some("Result"),
                reinterpret: Some("true"),
                ..Default::default()
            }),
        )?;
        if let Some(pods) = resp.pods {
            if regex_match {
                Ok(format!(
                    "^ {}",
                    pods[0].subpods[0].plaintext.as_ref().unwrap()
                ))
            } else {
                Ok(pods[0].subpods[0].plaintext.clone().unwrap())
            }
        } else {
            Err(ErrorKind::NoExtractableData.into())
        }
    } else if domain.ends_with("jisho.org") {
        jisho::handle(
            percent_decode(
                response
                    .url()
                    .path_segments()
                    .unwrap()
                    .last()
                    .unwrap()
                    .as_bytes(),
            ).decode_utf8()?
                .borrow(),
            regex_match,
        )
    } else {
        let mut bytes = Vec::new();
        response.read_to_end(&mut bytes)?;
        let headers = response.headers();
        let body = body_from_charsets(bytes, headers).and_then(|body| Ok(Cursor::new(body)));

        match (
            body.and_then(|mut body| {
                Ok(html5ever::parse_document(
                    RcDom::default(),
                    Default::default(),
                ).from_utf8()
                    .read_from(&mut body)?)
            }),
            headers.get::<ContentLength>(),
            headers.get::<ContentType>().and_then(|ct| {
                let ct = &ct.0;
                Some((&ct.0, &ct.1))
            }),
        ) {
            (Ok(dom), _, _) => {
                let mut title = String::new();
                let mut description = String::new();
                walk_for_metadata(dom.document, &mut title, &mut description);
                if title.trim().is_empty() {
                    Err(ErrorKind::NoExtractableData.into())
                } else {
                    if description.is_empty() || domain.ends_with("imgur.com") {
                        Ok(format!("^ {}", title))
                    } else {
                        if description.starts_with(&title) || description.ends_with(&title) {
                            Ok(format!("^ {}", description))
                        } else {
                            Ok(format!("^ {} - {}", title, description))
                        }
                    }
                }
            }
            (Err(_), Some(l), Some((top, sub))) => {
                Ok(format!(
                    "^ {}: {}; {}",
                    top,
                    sub,
                    l.file_size(Options::BINARY).unwrap()
                ))
            }
            (_, None, Some((top, sub))) => Ok(format!("^ {}: {}", top, sub)),
            (Err(_), None, None) |
            (Err(_), Some(_), None) => Err(ErrorKind::NoExtractableData.into()),
        }
    }
}

fn pretty_number(num: &str) -> String {
    let mut ret = String::with_capacity(num.len() + num.len() / 3);
    let mut iter = num.chars().rev().peekable();
    while {
        let x = iter.peek();
        x.is_some()
    } {
        let x = iter.next();
        let y = iter.next();
        let z = iter.next();
        let a = iter.peek();
        if x.is_some() && y.is_some() && z.is_some() && a.is_some() {
            ret.push(x.unwrap());
            ret.push(y.unwrap());
            ret.push(z.unwrap());
            ret.push('.');
        } else if x.is_some() && y.is_some() && z.is_some() {
            ret.push(x.unwrap());
            ret.push(y.unwrap());
            ret.push(z.unwrap());
        } else if x.is_some() && y.is_some() {
            ret.push(x.unwrap());
            ret.push(y.unwrap());
        } else {
            ret.push(x.unwrap());
        }
    }
    ret.chars().rev().collect()
}

fn body_from_charsets(bytes: Vec<u8>, headers: &Headers) -> Result<String> {
    Ok(if let Some(&(_, ref charset)) =
        headers.get::<ContentType>().and_then(|ct| {
            let ct = &ct.0;
            ct.2.iter().find(|e| e.0 == Attr::Charset)
        }) {
        if *charset == Value::Utf8 {
            String::from_utf8(bytes)?
        } else {
            encoding_from_whatwg_label(charset.as_str())
                .unwrap()
                .decode(&bytes, DecoderTrap::Replace)
                .unwrap()
        }
    } else {
        String::from_utf8(bytes)?
    })
}

fn walk_for_metadata(node: Handle, title: &mut String, description: &mut String) {
    match node.data {
        NodeData::Element {
            ref name,
            ref attrs,
            ..
        } => {
            if &*name.local == "title" && title.is_empty() {
                for child in node.children.borrow().iter() {
                    if let NodeData::Text { ref contents } = child.data {
                        let text = contents.borrow();
                        let text = text.trim();
                        if text != "" && text != "\n" {
                            title.push_str(text);
                        }
                    }
                }
            } else if &*name.local == "meta" {
                let mut in_description = false;
                for attr in attrs.borrow().iter() {
                    if &*attr.name.local == "name" && &*attr.value == "description" {
                        in_description = true;
                    } else if &*attr.name.local == "content" && in_description {
                        in_description = false;
                        description.push_str(attr.value.trim());
                    }
                }
            }
        }
        NodeData::ProcessingInstruction { .. } => unreachable!(),
        NodeData::Document { .. } |
        NodeData::Doctype { .. } |
        NodeData::Comment { .. } |
        NodeData::Text { .. } => {}
    }
    for child in node.children.borrow().iter() {
        walk_for_metadata(child.clone(), title, description);
    }
}

mod jisho {
    use reqwest;
    use serde_json;

    use std::io::Read;

    use errors::*;

    const API_BASE: &str = "http://jisho.org/api/v1/search/words?keyword=";

    #[derive(Clone, Debug, PartialEq, Deserialize)]
    pub struct ApiResponse {
        pub meta: Meta,
        pub data: Vec<DataPoint>,
    }

    #[derive(Clone, Debug, PartialEq, Deserialize)]
    pub struct Meta {
        pub status: usize,
    }

    #[derive(Clone, Debug, PartialEq, Deserialize)]
    pub struct DataPoint {
        pub is_common: Option<bool>,
        pub tags: Vec<String>,
        pub japanese: Vec<Japanese>,
        pub senses: Vec<Senses>,
    }

    #[derive(Clone, Debug, PartialEq, Deserialize)]
    pub struct Japanese {
        pub word: Option<String>,
        pub reading: Option<String>,
    }

    #[derive(Clone, Debug, PartialEq, Deserialize)]
    pub struct Senses {
        pub english_definitions: Vec<String>,
        pub parts_of_speech: Vec<String>,
    }

    pub fn handle(input: &str, regex_match: bool) -> Result<String> {
        let mut resp = reqwest::get(&(API_BASE.to_owned() + input))?;
        let resp = if resp.status().is_success() {
            let mut body = String::new();
            resp.read_to_string(&mut body)?;
            let resp: ApiResponse = serde_json::from_str(&body)?;
            if resp.meta.status == 200 {
                resp.data
            } else {
                return Err(ErrorKind::NoExtractableData.into());
            }
        } else {
            return Err(ErrorKind::NoExtractableData.into());
        };

        let mut ret = if regex_match {
            String::from("^ ")
        } else {
            String::new()
        };

        for (n, dp) in resp.iter().take(3).enumerate() {
            if n == 0 {
                ret.push_str("\x021\x02: ");
            } else {
                ret.push_str(&format!("; \x02{}\x02: ", n + 1));
            }
            let mut senses = String::new();
            for (n, s) in dp.senses.iter().take(3).enumerate() {
                let mut parts_of_speech = String::new();
                for (n, p) in s.parts_of_speech.iter().enumerate() {
                    if n == 0 {
                        parts_of_speech.push_str(&p);
                    } else {
                        parts_of_speech.push_str(&format!(", {}", p));
                    }
                }
                if n == 0 {
                    senses.push_str(&format!(
                        "{}: {}",
                        &parts_of_speech,
                        &s.english_definitions[0]
                    ));
                } else {
                    senses.push_str(&format!(", {}", &s.english_definitions[0]));
                }
            }
            if let Some(ref w) = dp.japanese[0].word {
                ret.push_str(w);
                if let Some(ref r) = dp.japanese[0].reading {
                    ret.push_str(&format!("({})", r));
                }
            } else if let Some(ref r) = dp.japanese[0].reading {
                ret.push_str(r);
            }

            ret.push_str(&format!(
                " {}{} [{}]",
                if let Some(c) = dp.is_common {
                    if c {
                        "Common"
                    } else {
                        "Uncommon"
                    }
                } else {
                    ""
                },
                if dp.tags.is_empty() {
                    "".into()
                } else {
                    let mut out = String::from("(");
                    for (n, t) in dp.tags.iter().enumerate() {
                        if n == 0 {
                            out.push_str(&format!("{}", t));
                        } else {
                            out.push_str(&format!(", {}", t));
                        }
                    }
                    out.push_str(")");
                    out
                },
                &senses,
            ));
        }
        if ret != "" {
            Ok(ret)
        } else {
            Err(ErrorKind::NoExtractableData.into())
        }
    }
}
