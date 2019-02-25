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

// TODO: Remove this once diesel's macros can be refered to as 2018 paths
#[cfg(feature = "modules")]
#[cfg_attr(feature = "modules", macro_use)]
extern crate diesel;

pub mod config;
pub mod error;
pub mod message;
pub mod modules;
pub mod prelude {
    pub use crate::{
        config::{Config, Module as ModuleCfg},
        message::{IrcMessageExt, Message, MessageContext, Stage, Trigger},
        modules::{module, Module},
        Builder,
    };
    pub use diesel::SqliteConnection as DbConn;
    pub use irc::client::IrcClient;
    pub use std::sync::Arc;
}

use chrono::Utc;
use diesel::{sqlite::SqliteConnection, Connection};
use futures::sync::mpsc;
use irc::client::{ext::ClientExt, Client, IrcClient};
use tokio::{prelude::*, timer::Delay};

use std::{collections::HashMap, path::Path, sync::Arc, time::Instant};

use crate::{config::ConfigTrigger, error::*, modules::ModuleContext, prelude::*};

enum ConfigKind<'p> {
    File(&'p Path),
    Parsed(Config),
}

// FIXME: The dyn Fn can go with existial types
pub struct Builder<'c, 'l> {
    config: Option<ConfigKind<'c>>,
    loader: Option<&'l dyn Fn(&DbConn, &mut ModuleCfg) -> Result<Option<Box<dyn Module>>>>,
}

impl<'c, 'l> Builder<'c, 'l> {
    #[inline]
    pub fn new() -> Builder<'c, 'l> {
        Builder {
            config: None,
            loader: None,
        }
    }

    #[inline]
    pub fn with_loader<F>(self, loader: &'l F) -> Self
    where
        F: Fn(&DbConn, &mut ModuleCfg) -> Result<Option<Box<dyn Module>>>,
    {
        Builder {
            config: self.config,
            loader: Some(loader),
        }
    }

    #[inline]
    pub fn with_config(self, config: Config) -> Self {
        Builder {
            config: Some(ConfigKind::Parsed(config)),
            loader: self.loader,
        }
    }

    #[inline]
    pub fn with_config_file(self, path: &'c Path) -> Self {
        Builder {
            config: Some(ConfigKind::File(path)),
            loader: self.loader,
        }
    }

    // TODO: Handle shutdown
    #[inline]
    /// #Panics
    /// * No config file was provided
    /// * Default modules were disabled, and no module loader was specified
    pub fn build(self) -> Result<impl Future<Item = (), Error = ()>> {
        let mut config = match self.config.expect("No config file provided to builder") {
            ConfigKind::Parsed(c) => c,
            ConfigKind::File(p) => Config::from_path(p)?,
        };

        #[cfg(not(feature = "modules"))]
        let loader = self
            .loader
            .expect("No module loader specified, but default modules disabled");

        // Initialize database connection
        let db = SqliteConnection::establish(&config.database)?;

        // Setup modules
        let mut modules = ModuleContext::new();
        for channel in &mut config.channels {
            for mut cfg in &mut channel.modules {
                #[cfg(feature = "modules")]
                let module = if let Some(ref loader) = self.loader {
                    loader(&db, &mut cfg)?
                        .map_or_else(|| modules::load_module(&db, &mut cfg), |v| Ok(Some(v)))?
                        .ok_or_else(|| Error::ModuleNotFound(cfg.name.clone()))?
                } else {
                    modules::load_module(&db, &mut cfg)?
                        .ok_or_else(|| Error::ModuleNotFound(cfg.name.clone()))?
                };
                #[cfg(not(feature = "modules"))]
                let module =
                    loader(&mut cfg)?.ok_or_else(|| Error::ModuleNotFound(cfg.name.clone()))?;

                modules.insert(
                    (channel.name.clone(), cfg.name.clone()),
                    (cfg.clone(), module),
                );
            }
        }

        Ok(IrcClient::new_future(config.as_irc_config())?
            .map_err(|e| panic!("{}", e))
            .map(move |client| {
                // This is the future that will drive message sends to completion
                tokio::spawn(client.1.map_err(|e| panic!("{}", e)));
                client.0.identify().unwrap();

                let client = Arc::new(client.0);
                let (mctx, mctx_receiver) = mpsc::unbounded();
                let mctx = Arc::new(MessageContext(mctx));

                for (ref mut cfg, module) in modules.values_mut() {
                    if module.handles(Stage::Connected) {
                        module.connected(&client, &mctx, &db, cfg);
                    }
                }

                // Deliver ready messages to send future
                // TODO: Pre/Post-MessageSend
                let client2 = Arc::clone(&client);
                tokio::spawn(mctx_receiver.for_each(move |(msg, due, mode)| {
                    match due {
                        message::DueBy::Now => {
                            message::send(&client2, &msg, &mode);
                        }
                        message::DueBy::At(at) => {
                            // This will fail on a negative duration, i.e. if at < now
                            if let Ok(dur) = (at - Utc::now()).to_std() {
                                let now_std = Instant::now();
                                let client3 = Arc::clone(&client2);
                                tokio::spawn(
                                    Delay::new(now_std + dur).map_err(|e| panic!("{}", e)).map(
                                        move |_| {
                                            message::send(&client3, &msg, &mode);
                                        },
                                    ),
                                );
                            } else {
                                message::send(&client2, &msg, &mode);
                            }
                        }
                    };

                    Ok(())
                }));

                tokio::spawn(
                    client
                        .stream()
                        .map_err(|e| panic!("{}", e))
                        .for_each(move |msg| {
                            match message::cfg_trigger_match(
                                &msg,
                                &[ConfigTrigger::Explicit(".help".to_string())],
                            ) {
                                // .help with no modules specified
                                Some(Trigger::Explicit("")) => {
                                    let mut mods = String::new();
                                    for (_, name) in modules.keys() {
                                        if mods.is_empty() {
                                            mods.push_str(&name);
                                        } else {
                                            mods.push_str(", ");
                                            mods.push_str(&name);
                                        }
                                    }
                                    mctx.reply_priv(&msg, format!("Modules: {}", mods));
                                }
                                // Other .help
                                Some(trigger) => {
                                    if let Some((_, (_, ref mut module))) =
                                        modules.iter_mut().find(|(_, (cfg, _))| {
                                            cfg.triggers.iter().any(|t| t.help_relevant(&trigger))
                                        })
                                    {
                                        mctx.reply_priv(&msg, module.help());
                                    } else {
                                        mctx.reply_priv(&msg, "No module with that alias found");
                                    }
                                }
                                // Regular message
                                _ => {
                                    for (_, (ref mut cfg, ref mut module)) in modules
                                        .iter_mut()
                                        .filter(|(_, (_, m))| m.handles(Stage::Received))
                                    {
                                        if let Some(t) =
                                            message::cfg_trigger_match(&msg, &cfg.triggers)
                                        {
                                            module.received(&client, &mctx, &db, cfg, &msg, t);
                                        }
                                    }
                                }
                            }

                            Ok(())
                        }),
                );
            }))
    }
}
