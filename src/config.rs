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
use serde::{
    de::{self, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use toml::Value;

use std::{collections::HashMap, io::Read, path::Path};

use crate::{error::*, message::Trigger};

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
        let mut toml: Config = toml::de::from_str(&s)?;

        // Test if all modules have unique names
        let unique = toml.servers.iter_mut().any(|s| {
            s.channels.iter_mut().any(|c| {
                let n = c.modules.len();

                c.modules.sort_unstable_by(|m1, m2| m1.name.cmp(&m2.name));
                c.modules.dedup_by(|m1, m2| m1.name == m2.name);

                n == c.modules.len()
            })
        });

        if unique {
            Ok(toml)
        } else {
            Err(Error::ModuleDuplicate)
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Server {
    pub address: String,
    pub port: Option<u16>,
    password: Option<String>,
    pub nick: String,
    nick_password: Option<String>,
    pub max_burst_messages: Option<u32>,
    pub burst_window_length: Option<u32>,
    pub use_ssl: Option<bool>,

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
    pub(crate) triggers: Vec<ConfigTrigger>,

    #[serde(flatten)]
    pub fields: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
// This type needs to be different from Trigger, because the formats are very different
pub(crate) enum ConfigTrigger {
    Always,
    Command(String),
    Explicit(String),
    Action(String),
    // matched, ignored
    Domains(Vec<String>, Vec<String>),
}

impl ConfigTrigger {
    pub(crate) fn help_relevant(&self, other: &Trigger) -> bool {
        match (self, other) {
            (ConfigTrigger::Always, Trigger::Always) => true,
            // This returns true if the commands are of the same enum variant
            (ConfigTrigger::Command(c1), Trigger::Command(c2)) => {
                c1 == &*String::from(*c2).split(' ').next().unwrap().to_lowercase()
            }
            (ConfigTrigger::Explicit(a), Trigger::Explicit(b)) if &a[1..] == *b => true,
            (ConfigTrigger::Action(a), Trigger::Action(b)) if &a[3..] == *b => true,
            // FIXME: .contains fails to resolve for some reason…
            (ConfigTrigger::Domains(m, i), Trigger::Urls(b)) => b
                .iter()
                .all(|b| m.iter().any(|m| m == b) && !i.iter().any(|i| i == b)),
            _ => false,
        }
    }
}

impl<'de> Deserialize<'de> for ConfigTrigger {
    fn deserialize<D>(de: D) -> ::std::result::Result<ConfigTrigger, D::Error>
    where
        D: Deserializer<'de>,
    {
        de.deserialize_str(TriggerVisitor)
    }
}

struct TriggerVisitor;

impl<'de> Visitor<'de> for TriggerVisitor {
    type Value = ConfigTrigger;

    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(formatter, "a trigger in the form of one of: .trigger, <ALWAYS>, <ACTION name>, <DOMAINS \"one.com\",\"two.com\",!\"ignorethis.com\">, <COMMAND JOIN> <COMMAND PART>")
    }

    fn visit_str<E>(self, s: &str) -> ::std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        if s.starts_with('.') {
            Ok(ConfigTrigger::Explicit(s.to_lowercase()))
        } else if s.starts_with('<') && s.ends_with('>') {
            let s = s.to_lowercase();
            if s == "<always>" {
                Ok(ConfigTrigger::Always)
            } else if s.starts_with("<action ") {
                Ok(ConfigTrigger::Action(s[8..s.len() - 1].to_string()))
            } else if s.starts_with("<command ") {
                // TODO: Sanitize name
                Ok(ConfigTrigger::Command(s[9..s.len() - 1].to_string()))
            } else if s.starts_with("<domains ") {
                let (mut allowed, mut ignored) = (vec![], vec![]);
                for dom in &mut (&s[9..s.len() - 1]).split(',') {
                    let ignore = dom.starts_with('!');
                    if !dom.starts_with("http") || !dom.starts_with("!http") {
                        if ignore {
                            ignored.push(format!("https://{}", &dom[1..]));
                        } else {
                            allowed.push(format!("https://{}", dom));
                        }
                    } else if ignore {
                        ignored.push(dom[1..].to_string());
                    } else {
                        allowed.push(dom.to_string());
                    }
                }
                Ok(ConfigTrigger::Domains(allowed, ignored))
            } else {
                Err(de::Error::invalid_value(Unexpected::Str(&s), &self))
            }
        } else {
            Err(de::Error::invalid_value(Unexpected::Str(&s), &self))
        }
    }
}

impl Server {
    pub(crate) fn as_irc_config(&self) -> IrcConfig {
        IrcConfig {
            server: Some(self.address.clone()),
            port: self.port.clone(),
            use_ssl: Some(self.use_ssl.unwrap_or(true)),

            nickname: Some(self.nick.clone()),
            nick_password: self.nick_password.clone(),
            password: self.password.clone(),

            max_messages_in_burst: self.max_burst_messages.clone(),
            burst_window_length: self.burst_window_length.clone(),

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
