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
    pub modules: Vec<String>,
}

// This unidiomatically does not use TryFrom, because of the way we do error handling
impl<'a> From<&'a ServerCfg> for IrcServer {
    fn from(srv: &'a ServerCfg) -> IrcServer {
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
            max_messages_in_burst: srv.max_burst_messages.clone(),
            burst_window_length: srv.burst_window_length.clone(),
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
                panic!("IrcServer creation failed: {:?}", e)
            }
            Ok(srv) => srv,
        }
    }
}

pub fn parse_config(input: &str) -> Config {
    let de = de::from_str(input);
    // Why does the type not get inferred
    let ret: Config = if de.is_err() {
        crit!(::SLOG_ROOT, "Failed to parse config file: {:?}", de);
        panic!("Failed to parse config file: {:?}", de)
    } else {
        de.unwrap()
    };
    for srv in &ret.servers {
        if srv.weather_secret.is_none() &&
            srv.channels
                .iter()
                .any(|c| c.modules.iter().any(|m| m == "weather"))
        {
            crit!(
                ::SLOG_ROOT,
                "Weather modules enabled on {:?}, but no weather API secret given",
                &srv.address
            );
            panic!(
                "Weather modules enabled on {:?}, but no weather API secret given",
                &srv.address
            );
        }
    }

    ret
}
