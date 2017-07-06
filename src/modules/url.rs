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
use reqwest::header::ContentType;
use reqwest::mime::{Attr, Value};
use reqwest::Response;

use std::io::{Cursor, Read};

use errors::*;

pub fn handle(mut response: Response) -> Result<String> {
    let body = body_from_charsets(&mut response)?;
    let size = body.len();
    let size = size.file_size(Options::CONVENTIONAL).unwrap();
    let mut body = Cursor::new(body);
    let headers = response.headers();

    Ok(match (
        headers.get::<ContentType>(),
        html5ever::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut body),
    ) {
        (_, Ok(dom)) => {
            let mut title = String::new();
            let mut description = String::new();
            walk(dom.document, &mut title, &mut description);

            // Imgur is a piece of shit that sets the title with JS dynamically
            // TODO: Find more pieces of shit that behave shittily
            if let Some("imgur.com") = response.url().domain() {
                format!("[{}; {}]", description, size)
            } else if description.is_empty() || description == title {
                format!("[{}; {}]", title, size)
            } else {
                format!("[{}: {}; {}]", title, description, size)
            }
        }
        (Some(ct), Err(_)) => {
            let ct = &ct.0;
            format!("[{}: {}; {}]", ct.0.as_str(), ct.1.as_str(), size)
        }
        _ => format!("[{}]", size),
    })
}

fn body_from_charsets(resp: &mut Response) -> Result<String> {
    let mut body = String::new();
    if let Some((_, ref charset)) =
        resp.headers()
            .get::<ContentType>()
            .and_then(|ct| {
                let ct = &ct.0;
                ct.2.iter().find(|e| e.0 == Attr::Charset)
            })
            // FIXME: .cloned can go once NLL hits
            .cloned()
    {
        if *charset == Value::Utf8 {
            resp.read_to_string(&mut body)?;
        } else {
            body.push_str(&encoding_from_whatwg_label(charset.as_str())
                .unwrap()
                .decode(
                    &resp.bytes()
                        .collect::<::std::result::Result<Vec<u8>, _>>()
                        .unwrap(),
                    DecoderTrap::Replace,
                )
                .unwrap());
        }
    } else {
        // Pray that it's utf8
        resp.read_to_string(&mut body)?;
    }
    Ok(body)
}

fn walk(node: Handle, title: &mut String, description: &mut String) {
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
        walk(child.clone(), title, description);
    }
}
