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

use reqwest;
use serde_json::Value;

use config::ServerCfg;
use errors::*;

pub fn handle(cfg: &ServerCfg, msg: &str) -> Result<String> {
    let body: Value = reqwest::get(&format!(
        "https://www.googleapis.com/customsearch/v1?num=3&fields=items\
         &cx={}&key={}&q={}",
        cfg.google_search_id.as_ref().unwrap(),
        cfg.google_search_key.as_ref().unwrap(),
        msg
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
        if n != 3 {
            formatted.push_str(&format!(
                "\x02{}\x02: {} [{}]; ",
                n + 1,
                item.pointer("/link").unwrap().as_str().unwrap(),
                item.pointer("/snippet").unwrap().as_str().unwrap().replace('\n', "")
            ));
        } else {
            formatted.push_str(&format!(
                "\x02{}\x02: {} [{}]",
                n + 1,
                item.pointer("/link").unwrap().as_str().unwrap(),
                item.pointer("/snippet").unwrap().as_str().unwrap().replace('\n', "")
            ));
        }
    }
    Ok(formatted)
}
