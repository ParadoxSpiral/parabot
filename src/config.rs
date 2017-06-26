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

use irc::client::server::IrcServer;
use irc::client::data::config::Config as IrcConfig;
use toml::de;

use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(rename = "server")]
    pub servers: Vec<Server>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub nickname: String,
    #[serde(rename = "alternative_nicknames")]
    pub alt_nicknames: Option<Vec<String>>,
    #[serde(rename = "nickserv_password")]
    pub nick_password: String,
    pub server_password: Option<String>,
    pub address: String,
    pub port: u16,
    #[serde(rename = "channel")]
    pub channels: Vec<Channel>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub name: String,
    pub password: Option<String>,
    pub modules: Vec<String>,
}

impl<'a> From<&'a Server> for IrcServer {
    fn from(srv: &'a Server) -> IrcServer {
        let srv = IrcServer::from_config(IrcConfig {
            nickname: Some(srv.nickname.clone()),
            alt_nicks: srv.alt_nicknames.clone(),
            nick_password: Some(srv.nick_password.clone()),
            server: Some(srv.address.clone()),
            port: Some(srv.port),
            password: srv.server_password.clone(),
            use_ssl: Some(true),
            channels: Some(srv.channels.iter().map(|c| c.name.clone()).collect()),
            channel_keys: {
                if srv.channels.iter().all(|c| c.password.is_none()) {
                    None
                } else {
                    let mut hm = HashMap::with_capacity(srv.channels.len());
                    for c in &srv.channels {
                        if c.password.is_some() {
                            hm.insert(c.name.clone(), c.password.as_ref().unwrap().clone());
                        }
                    }
                    hm.shrink_to_fit();
                    Some(hm)
                }
            },
            should_ghost: Some(true),
            version: Some(format!(
                "Parabot {} brought to you by {}",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_AUTHORS")
            )),
            source: Some("https://github.com/ParadoxSpiral/parabot".into()),
            ..Default::default()
        });
        match srv {
            Err(e) => {
                crit!(::SLOG_ROOT, "IrcServer creation failed: {:?}", e);
                panic!("")
            }
            Ok(srv) => srv,
        }
    }
}

pub fn parse_config(input: &str) -> Config {
    let de = de::from_str(input);
    if de.is_err() {
        crit!(::SLOG_ROOT, "Failed to parse config file: {:?}", de);
        panic!("")
    } else {
        de.unwrap()
    }
}
