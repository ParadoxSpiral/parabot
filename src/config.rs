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

use errors::*;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(rename = "server")]
    pub servers: Vec<ServerCfg>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerCfg {
    pub address: String,
    pub port: u16,
    pub nickname: String,
    #[serde(rename = "alternative_nicknames")]
    pub alt_nicknames: Option<Vec<String>>,
    #[serde(rename = "nickserv_password")]
    pub nick_password: String,
    pub server_password: Option<String>,
    pub database: String,
    #[serde(rename = "weather_api_secret")]
    pub weather_secret: Option<String>,
    #[serde(rename = "geocoding_api_key")]
    pub geocoding_key: Option<String>,
    pub max_burst_messages: Option<u32>,
    pub burst_window_length: Option<u32>,
    #[serde(rename = "channel")]
    pub channels: Vec<ChannelCfg>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelCfg {
    pub name: String,
    pub password: Option<String>,
    pub url_blacklisted_domains: Option<Vec<String>>,
    pub modules: Vec<String>,
}

impl ServerCfg {
    pub fn new_ircserver(&self) -> Result<IrcServer> {
        Ok(IrcServer::from_config(IrcConfig {
            nickname: Some(self.nickname.clone()),
            alt_nicks: self.alt_nicknames.clone(),
            nick_password: Some(self.nick_password.clone()),
            server: Some(self.address.clone()),
            port: Some(self.port),
            password: self.server_password.clone(),
            use_ssl: Some(true),
            channels: Some(self.channels.iter().map(|c| c.name.clone()).collect()),
            channel_keys: {
                if self.channels.iter().all(|c| c.password.is_none()) {
                    None
                } else {
                    let mut hm = HashMap::with_capacity(self.channels.len());
                    for c in &self.channels {
                        if c.password.is_some() {
                            hm.insert(c.name.clone(), c.password.as_ref().unwrap().clone());
                        }
                    }
                    hm.shrink_to_fit();
                    Some(hm)
                }
            },
            max_messages_in_burst: self.max_burst_messages,
            burst_window_length: self.burst_window_length,
            encoding: Some("UTF-8".to_owned()),
            should_ghost: Some(true),
            version: Some(format!(
                "Parabot {} brought to you by {}",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_AUTHORS")
            )),
            source: Some("https://github.com/ParadoxSpiral/parabot".into()),
            ..Default::default()
        })?)
    }
}

pub fn parse_config(input: &str) -> Result<Config> {
    let ret: Result<Config> = de::from_str::<Config>(input)
        .or_else(|err| Err(ErrorKind::Serialization(err).into()));
    let ret = ret?;
    for srv in &ret.servers {
        if (srv.weather_secret.is_none() || srv.geocoding_key.is_none()) &&
            srv.channels
                .iter()
                .any(|c| c.modules.iter().any(|m| m == "weather"))
        {
            return Err(format!(
                "Weather modules enabled on {:?}, but no weather API secret or geocoding key given",
                &srv.address
            ).into());
        }
    }
    Ok(ret)
}
