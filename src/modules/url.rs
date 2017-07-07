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
use reqwest::header::{ContentLength, ContentType, Headers};
use reqwest::mime::{Attr, Value};
use reqwest::Response;

use std::io::{Cursor, Read};

use errors::*;

pub fn handle(mut response: Response) -> Result<String> {
    let mut bytes = Vec::new();
    response.read_to_end(&mut bytes)?;
    let headers = response.headers();

    Ok(if let Ok(body) = body_from_charsets(bytes, headers) {
        let mut body = Cursor::new(body);
        match (
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
                    format!("[{}]", description)
                } else if description.is_empty() || description == title && title != "" {
                    format!("[{}]", title)
                } else {
                    format!("[{}: {}]", title, description)
                }
            }
            (Some(ct), _) => {
                let ct = &ct.0;
                format!("[{}: {}]", ct.0.as_str(), ct.1.as_str())
            }
            (None, Err(_)) => unimplemented!(),
        }
    } else {
        // Most likely an image or similar
        match (
            headers.get::<ContentType>().and_then(|ct| {
                let ct = &ct.0;
                Some((&ct.0, &ct.1))
            }),
            headers.get::<ContentLength>(),
        ) {
            (Some((top, sub)), Some(l)) => {
                format!(
                    "[{}: {}; {}]",
                    top,
                    sub,
                    l.file_size(Options::CONVENTIONAL).unwrap()
                )
            }
            (Some((top, sub)), None) => format!("[{}: {}]", top, sub),
            (None, _) => unimplemented!(),
        }
    })
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
        // Pray that it's utf8
        String::from_utf8(bytes)?
    })
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