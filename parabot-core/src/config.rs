// Copyright (C) 2018  ParadoxSpiral
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

use irc::client::data::config::Config as IrcConfig;
use toml::{de, Value};

use super::error::*;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub database: String,

    #[serde(rename = "server")]
    pub servers: Vec<Server>,
}

impl Config {
    #[inline]
    pub fn from_path<T: AsRef<Path>>(path: T) -> Result<Config> {
        let mut file = ::std::fs::File::open(path)?;
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        let mut toml: Config = de::from_str(&s)?;

        // Test if all modules have unique names, and unique triggers respectively
        if toml.servers.iter_mut().any(|s| {
            s.channels.iter_mut().any(|c| {
                let n = c.modules.len();

                c.modules.sort_unstable_by(|m1, m2| m1.name.cmp(&m2.name));
                c.modules.dedup_by(|m1, m2| m1.name == m2.name);

                n != c.modules.len()
            })
        }) {
            Err(Error::ModuleDuplicate)
        } else if toml.servers.iter().any(|s| {
            s.channels.iter().any(|c| {
                let mut triggers = vec![];
                c.modules
                    .iter()
                    .map(|m| triggers.extend(&m.triggers))
                    .count();
                let l = triggers.len();

                triggers.sort_unstable();
                triggers.dedup();

                l != triggers.len()
            })
        }) {
            Err(Error::TriggerDuplicate)
        } else {
            Ok(toml)
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Server {
    pub address: String,
    password: Option<String>,
    pub nick: String,
    nick_password: Option<String>,
    pub max_burst_messages: u32,
    pub burst_window_length: u32,
    pub use_ssl: bool,
    pub port: u16,

    #[serde(rename = "channel")]
    pub channels: Vec<Channel>,
}

#[derive(Deserialize, Debug)]
pub struct Channel {
    pub name: String,
    pub password: Option<String>,

    #[serde(rename = "module")]
    pub modules: Vec<Module>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Module {
    pub name: String,
    pub triggers: Option<Vec<String>>,
    #[serde(default = "default_triggers_always")]
    pub triggers_always: bool,

    #[serde(flatten)]
    pub fields: HashMap<String, Value>,
}

fn default_triggers_always() -> bool {
    false
}

impl Server {
    #[inline]
    pub(crate) fn as_irc_config(&self) -> IrcConfig {
        IrcConfig {
            server: Some(self.address.clone()),
            port: Some(self.port),
            use_ssl: Some(self.use_ssl),

            nickname: Some(self.nick.clone()),
            nick_password: self.nick_password.clone(),
            password: self.password.clone(),

            max_messages_in_burst: Some(self.max_burst_messages),
            burst_window_length: Some(self.burst_window_length),

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

            version: Some(format!(
                "Parabot {} brought to you by {}",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_AUTHORS")
            )),
            source: Some("https://github.com/ParadoxSpiral/parabot".into()),

            ..Default::default()
        }
    }
}
