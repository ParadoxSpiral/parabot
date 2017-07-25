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

use ddg::{RelatedTopic, Type, Query};
use ddg::response::TopicResult;
use reqwest;

use config::ServerCfg;
use errors::*;

pub fn handle(
    cfg: &ServerCfg,
    msg: &str,
    show_redirect: bool,
    regex_match: bool,
) -> Result<String> {
    let resp = Query::new(msg, "parabot").execute()?;

    match resp.response_type {
        Type::Disambiguation | Type::Category => {
            let mut ret = String::new();
            for (n, related) in resp.related_topics
                .iter()
                .filter(|rt| if let &RelatedTopic::TopicResult { .. } = *rt {
                    true
                } else {
                    false
                })
                .take(3)
                .enumerate()
            {
                if let RelatedTopic::TopicResult(TopicResult { ref text, .. }) = *related {
                    if n == 0 {
                        if resp.response_type == Type::Disambiguation {
                            ret.push_str(&format!("\x021:\x02 {}", text));
                        } else {
                            ret.push_str(&format!("{}: \x021:\x02 {}", resp.abstract_url, text));
                        }
                    } else {
                        ret.push_str(&format!(" \x02{}:\x02 {}", n + 1, text));
                    }
                } else {
                    unreachable!()
                }
            }
            ret.push_str(" ...");
            Ok(ret)
        }
        Type::Article | Type::Name => Ok(format!("{}: {}", resp.abstract_url, resp.abstract_text)),
        Type::Exclusive => {
            let client = reqwest::Client::new()?;
            let res = client.get(&resp.redirect)?.send()?;
            if show_redirect {
                Ok(
                    format!("{}: ", resp.redirect) +
                        &super::url::handle(cfg, client, res, regex_match)?,
                )
            } else {
                Ok(super::url::handle(cfg, client, res, regex_match)?)
            }
        }
        Type::Nothing => unimplemented!("{:?}", resp),
    }
}
