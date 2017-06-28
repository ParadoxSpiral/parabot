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

use config::ServerCfg;

pub fn handle(cfg: &ServerCfg, target: &str, msg: &str, private: bool) -> String {
    if &msg[1..] == "help" {
        if private {
            "Hi! For more information, use .help <module>. This is a private \
             message, so I cannot tell you about any channel's enabled modules."
                .to_owned()
        } else {
            format!(
                "Hi! For more information, use .help <module>. \
                 Enabled modules: {:?}",
                cfg.channels
                    .iter()
                    .find(|c| &*c.name == target)
                    .unwrap()
                    .modules
                    .as_slice()
            )
        }
    } else {
        // Starts with help, e.g. more args
        match &msg[6..] {
            "bots" | ".bots" => {
                ".bots will (hopefully) cause all bots in the channel to reply.".to_owned()
            }
            "tell" | ".tell" => {
                ".tell <nick> <message> will tell the user with <nick> <message>, \
                 when they join a channel shared with me."
                    .to_owned()
            }
            "weather" | ".weather" => {
                unimplemented!();
                ".weather . (Powered by Dark Sky)".to_owned()
            }
            "wa" | ".wa" => {
                unimplemented!();
                ".wa <query> will query wolfram-alpha about <query>.".to_owned()
            }
            "url" => {
                unimplemented!();
                "url fetches urls posted in the channel, and displays their \
                 metadata, and depending on the website, \
                 more e.g. youtube views."
                    .to_owned()
            }
            _ => "Unknown or undocumented module, sorry.".to_owned(),
        }
    }
}
