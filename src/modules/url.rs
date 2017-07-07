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
            walk(dom.document, &mut title);

            // TODO: More website specific stuff
            Ok(format!("[{}]", title))
        }
        (Err(_), Some(l), Some((top, sub))) => {
            Ok(format!(
                "[{}: {}; {}]",
                top,
                sub,
                l.file_size(Options::CONVENTIONAL).unwrap()
            ))
        }
        (_, None, Some((top, sub))) => Ok(format!("[{}: {}]", top, sub)),
        (Err(_), None, None) |
        (Err(_), Some(_), None) => Err(ErrorKind::NoExtractableData.into()),
    }
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

fn walk(node: Handle, title: &mut String) {
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
            }
        }
        NodeData::ProcessingInstruction { .. } => unreachable!(),
        NodeData::Document { .. } |
        NodeData::Doctype { .. } |
        NodeData::Comment { .. } |
        NodeData::Text { .. } => {}
    }
    for child in node.children.borrow().iter() {
        walk(child.clone(), title);
    }
}
