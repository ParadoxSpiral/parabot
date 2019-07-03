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

use irc::client::IrcClient;
use irc::client::data::config::Config as IrcConfig;
use toml::de;

use std::collections::HashMap;

use errors::*;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(rename = "server")] pub servers: Vec<ServerCfg>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerCfg {
    pub address: String,
    pub port: u16,
    pub nickname: String,
    #[serde(rename = "alternative_nicknames")] pub alt_nicknames: Option<Vec<String>>,
    #[serde(rename = "nickserv_password")] pub nick_password: String,
    pub server_password: Option<String>,
    pub database: String,
    #[serde(rename = "weather_api_secret")] pub weather_secret: Option<String>,
    #[serde(rename = "geocoding_api_key")] pub geocoding_key: Option<String>,
    pub wolframalpha_appid: Option<String>,
    #[serde(rename = "youtube_api_key")] pub youtube_key: Option<String>,
    pub google_search_id: Option<String>,
    pub google_search_key: Option<String>,
    pub max_burst_messages: Option<u32>,
    pub burst_window_length: Option<u32>,
    pub owners: Vec<String>,
    pub wormy_nick: Option<String>,
    #[serde(rename = "channel")] pub channels: Vec<ChannelCfg>,
    pub use_ssl: bool,
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
    pub fn new_irc_client(&self) -> Result<IrcClient> {
        Ok(IrcClient::from_config(IrcConfig {
            nickname: Some(self.nickname.clone()),
            alt_nicks: self.alt_nicknames.clone(),
            nick_password: Some(self.nick_password.clone()),
            server: Some(self.address.clone()),
            port: Some(self.port),
            password: self.server_password.clone(),
            use_ssl: Some(self.use_ssl),
            channels: Some(self.channels.iter().map(|c| c.name.clone()).collect()),
            channel_keys: {
                if self.channels.iter().all(|c| c.password.is_none()) {
                    None
                } else {
                    let mut hm = HashMap::with_capacity(self.channels.len());
                    for c in &self.channels {
                        if let Some(ref key) = c.password {
                            hm.insert(c.name.clone(), key.clone());
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
    let ret = de::from_str::<Config>(input)?;
    for srv in &ret.servers {
        if (srv.weather_secret.is_none() || srv.geocoding_key.is_none())
            && srv.channels
                .iter()
                .any(|c| c.modules.iter().any(|m| m == "weather"))
        {
            panic!(
                "Weather module enabled on {:?}, but no weather API secret or geocoding key given",
                &srv.address
            );
        } else if srv.wolframalpha_appid.is_none()
            && srv.channels
                .iter()
                .any(|c| c.modules.iter().any(|m| m == "wolframaplha"))
        {
            panic!(
                "Wolframalpha module enabled on {:?}, but no appid given",
                &srv.address
            );
        } else if srv.youtube_key.is_none()
            && srv.channels
                .iter()
                .any(|c| c.modules.iter().any(|m| m == "youtube"))
        {
            panic!(
                "Youtube module enabled on {:?}, but no key given",
                &srv.address
            );
        } else if srv.wormy_nick.is_none()
            && srv.channels
                .iter()
                .any(|c| c.modules.iter().any(|m| m == "wormy"))
        {
            panic!(
                "Wormy module enabled on {:?}, but no nick given",
                &srv.address
            );
        } else if (srv.google_search_id.is_none() || srv.google_search_key.is_none())
            && srv.channels
                .iter()
                .any(|c| c.modules.iter().any(|m| m == "google"))
        {
            panic!(
                "Google module enabled on {:?}, but no search id given",
                &srv.address
            );
        }
    }
    Ok(ret)
}
