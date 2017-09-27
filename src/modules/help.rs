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

pub fn handle(cfg: &ServerCfg, target: &str, msg: &str, private: bool) -> Option<String> {
    if &msg[1..] == "help" {
        if private {
            Some(
                "Hi! For more information, use .help <module>. You can use these modules: \
                 `duckduckgo`, `url-info`, `weather`, `tell`."
                    .to_owned(),
            )
        } else {
            let mut modules = cfg.channels
                .iter()
                .find(|c| &*c.name == target)
                .unwrap()
                .modules
                .clone();
            modules.sort();
            Some(format!(
                "For more information, use .help <module>. \
                 Enabled modules: {:?}",
                &modules
            ))
        }
    } else {
        // Starts with help, e.g. more args
        match msg[6..].trim() {
            "bots" | ".bots" => {
                Some(".bots will (hopefully) cause all bots in the channel to reply.".to_owned())
            }
            "ddg" | ".ddg" => {
                Some(".ddg <search> uses ddg's instant answer API to perform a search.".to_owned())
            }
            "tell" | ".tell" => Some(
                ".tell <nick> <message> will tell the user with <nick> <message>, \
                 when they join a channel shared with me."
                    .to_owned(),
            ),
            "weather" | ".weather" => Some(
                "`.weather [<n|x-y><d|h>] [location]` will show weather information \
                 powered by Dark Sky. If you specify `n` and `d` xor `h`, data of the next \
                 `n`th`d|h` will be replied with. Specifying a range of `x-y` will use data of \
                 that range. Data is available for the next 168h, or 7d. If you omit `location`, \
                 the location you last used will be used."
                    .to_owned(),
            ),
            "url-info" | "url" => Some(
                "url-info fetches urls posted in the channel and displays their metadata, \
                 and, depending on the website, more. Current additional metadata: wolframalpha\
                 , jisho, youtube."
                    .to_owned(),
            ),
            "who" | ".who" => Some(
                "If parabot runs as part of a wormy hivemind(e.g. sees messages from other \
                 bots as his own nick), this will inform the user whether parabot answered the \
                 last command."
                    .to_owned(),
            ),
            _ => if super::module_enabled_channel(cfg, &*target, "wormy") {
                None
            } else {
                Some("Unknown or undocumented module, sorry.".into())
            },
        }
    }
}
