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
use html5ever::rcdom::{Handle, NodeData, RcDom};
use html5ever::tendril::TendrilSink;
use humansize::{FileSize, file_size_opts as Options};
use percent_encoding::percent_decode;
use reqwest;
use reqwest::{Client, Url};
use reqwest::header::{ContentLength, ContentType, Headers};
use reqwest::mime;
use serde_json::Value;
use urlshortener::{Provider, UrlShortener};
use wolfram_alpha::query;

use std::borrow::Borrow;
use std::io::{Cursor, Read};

use config::ServerCfg;
use errors::*;
use super::module_enabled_channel;

pub fn handle(cfg: &ServerCfg, url: Url, target: &str, regex_match: bool) -> Result<String> {
    let domain = url.domain().unwrap().to_owned();
    let sign = if regex_match { "┗━ " } else { "" };

    // Invoke either site specific or generic handler
    if module_enabled_channel(cfg, target, "youtube") &&
        (domain.ends_with("youtube.com") || domain.ends_with("youtu.be"))
    {
        let path = url.path_segments().unwrap().last().unwrap();
        let mut query = url.query_pairs();
        if path == "watch" || domain.ends_with("youtu.be") {
            let resp: Value = if domain.ends_with("youtube.com") {
                let v = query.find(|&(ref k, _)| k == "v").unwrap().1;
                let v = percent_decode(v.as_bytes()).decode_utf8()?;
                reqwest::get(&format!(
                    "https://www.googleapis.com/youtube/v3/videos?part=status,snippet,\
                     contentDetails,statistics&key={}&id={}",
                    cfg.youtube_key.as_ref().unwrap(),
                    v.as_ref()
                ))
            } else {
                reqwest::get(&format!(
                    "https://www.googleapis.com/youtube/v3/videos?part=status,snippet,\
                     contentDetails,statistics&key={}&id={}",
                    cfg.youtube_key.as_ref().unwrap(),
                    path.split('?').next().unwrap()
                ))
            }?
                .json()?;
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
                "┗━ {} [{}] ({}) {} views {}{}{}",
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
    } else if module_enabled_channel(cfg, target, "wolframalpha") &&
        domain.ends_with("wolframalpha.com")
    {
        let i = url.query_pairs().find(|&(ref k, _)| k == "i").unwrap().1;
        let i = percent_decode(i.as_bytes()).decode_utf8()?;
        let resp = query::query(
            None,
            cfg.wolframalpha_appid.as_ref().unwrap(),
            &i,
            Some(query::QueryParameters {
                includepodid: Some("Result"),
                reinterpret: Some("true"),
                ..Default::default()
            }),
        )?;
        if let Some(pods) = resp.pods {
            Ok(format!(
                "{}{}",
                sign,
                pods[0].subpods[0].plaintext.as_ref().unwrap()
            ))
        } else {
            Err(ErrorKind::NoExtractableData.into())
        }
    } else if module_enabled_channel(cfg, target, "jisho") && domain.ends_with("jisho.org") {
        jisho::handle(
            percent_decode(url.path_segments().unwrap().last().unwrap().as_bytes())
                .decode_utf8()?
                .borrow(),
            sign,
        )
    } else if module_enabled_channel(cfg, target, "google") && domain.contains(".google.") {
        let body: Value = reqwest::get(&format!(
            "https://www.googleapis.com/customsearch/v1?num=3&fields=items\
             &cx={}&key={}&q={}",
            cfg.google_search_id.as_ref().unwrap(),
            cfg.google_search_key.as_ref().unwrap(),
            percent_decode(
                url.query_pairs()
                    .find(|&(ref k, _)| k == "q")
                    .unwrap()
                    .1
                    .as_bytes()
            ).decode_utf8()?
        ))?
            .json()?;

        let mut formatted = String::new();
        for (n, item) in body.pointer("/items")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let url = item.pointer("/link").unwrap().as_str().unwrap();
            formatted.push_str(&format!(
                "\x02{}\x02: {} [{}]{}",
                n + 1,
                UrlShortener::new()?
                    .try_generate(url, Some(&[Provider::IsGd, Provider::VGd, Provider::HmmRs]))?,
                item.pointer("/snippet")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .replace('\n', ""),
                if n != 2 { "; " } else { "" }
            ));
        }
        Ok(formatted)
    } else {
        let client = Client::new()?;
        let response = client.head(url.as_str())?.send()?;
        let headers = response.headers();
        let content_length = headers.get::<ContentLength>();
        let content_type = headers.get::<ContentType>();

        match (content_length, content_type) {
            (Some(l), Some(mime)) if mime.0.subtype() != mime::HTML => Ok(format!(
                "{}{}; {}",
                sign,
                mime,
                l.file_size(Options::BINARY).unwrap()
            )),
            (None, Some(mime)) if mime.0.subtype() != mime::HTML => Ok(format!("┗━ {}", mime)),
            (_, Some(mime)) if mime.0.subtype() == mime::HTML => {
                let mut response = client.get(url.as_str())?.send()?;
                let mut bytes = Vec::new();
                response.read_to_end(&mut bytes)?;

                let dom = body_from_charsets(bytes, headers).and_then(|body| {
                    Ok(html5ever::parse_document(
                        RcDom::default(),
                        Default::default(),
                    ).from_utf8()
                        .read_from(&mut Cursor::new(body))?)
                })?;

                let show_description = cfg!(feature = "show_description");

                let mut title = String::new();
                let mut description = String::new();
                walk_for_metadata(dom.document, &mut title, &mut description);
                let title = title.trim();
                let description = description.trim();
                if title.is_empty() {
                    Err(ErrorKind::NoExtractableData.into())
                } else if !show_description ||
                    ((description.is_empty() || domain.ends_with("imgur.com") ||
                        domain.ends_with("github.com")) &&
                        !description.contains(title))
                {
                    Ok(format!("{}{}", sign, title))
                } else if description.contains(title) {
                    Ok(format!("{}{}", sign, description))
                } else {
                    Ok(format!("{}{} - {}", sign, title, description))
                }
            }
            _ => Err(ErrorKind::NoExtractableData.into()),
        }
    }
}

fn pretty_number(num: &str) -> String {
    let len = num.chars().count();
    let mut ret = String::with_capacity(len + len / 3);
    for (n, e) in num.chars().rev().enumerate() {
        ret.insert(0, e);
        if (n + 1) % 3 == 0 && n != 0 && n + 1 != len {
            ret.insert(0, ',');
        }
    }
    ret
}

fn body_from_charsets(bytes: Vec<u8>, headers: &Headers) -> Result<String> {
    Ok(if let Some(charset) = headers
        .get::<ContentType>()
        .and_then(|ct| ct.get_param(mime::CHARSET))
    {
        if charset == mime::UTF_8 {
            String::from_utf8(bytes)?
        } else {
            encoding_from_whatwg_label(charset.as_ref())
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
        } => if &*name.local == "title" && title.is_empty() {
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
        },
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
    use percent_encoding::percent_decode;
    use reqwest;

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

    pub fn handle(input: &str, sign: &str) -> Result<String> {
        let resp: ApiResponse = reqwest::get(
            &(API_BASE.to_owned() + &percent_decode(input.as_bytes()).decode_utf8()?),
        )?
            .json()?;
        let resp = resp.data;

        let mut ret = String::from(sign);

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
                        parts_of_speech.push_str(p);
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
                            out.push_str(t);
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
