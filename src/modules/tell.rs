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

use chrono::Utc;
use diesel;
use diesel::prelude::*;
use irc::client::prelude::*;
use parking_lot::Mutex;
use slog::Logger;

use std::collections::HashMap;

use config::{Config, ServerCfg};
use errors::*;
use models;
use schema;
use schema::pending_tells::dsl;

lazy_static!{
    static ref PENDING_TELLS: Mutex<HashMap<String, Mutex<usize>>> = {
        Mutex::new(HashMap::new())
    };
}

// Read DB to get init values
pub fn init(cfg: &Config, log: &Logger) -> Result<()> {
    let mut hm = PENDING_TELLS.lock();
    for srv in &cfg.servers {
        let tells = super::with_database(srv, |db| {
            Ok(dsl::pending_tells
                .filter(dsl::server_addr.eq(&srv.address))
                .load::<models::PendingTell>(db)?)
        })?;

        info!(log, "Pending tells: {:?}", &tells);
        hm.insert(srv.address.clone(), Mutex::new(tells.len()));
        hm.shrink_to_fit();
    }
    Ok(())
}

pub fn handle_user_join(
    cfg: &ServerCfg,
    srv: &IrcServer,
    log: &Logger,
    msg: &Message,
) -> Result<()> {
    let hm = PENDING_TELLS.lock();
    let mut pending = hm.get(&cfg.address).unwrap().lock();
    if *pending != 0 {
        if let Command::JOIN(ref chan, ..) = msg.command {
            let target_nick = msg.source_nickname().unwrap();

            let tells = super::with_database(cfg, |db| {
                Ok(dsl::pending_tells
                    .filter(dsl::server_addr.eq(&cfg.address))
                    .filter(dsl::target_nick.eq(&target_nick))
                    .filter(dsl::channel.eq(&*chan).or(dsl::channel.is_null()))
                    .load::<models::PendingTell>(db)?)
            })?;

            *pending -= 1;
            drop(pending);
            debug!(log, "Found pending tells: {:?}", tells);

            super::with_database(cfg, |db| {
                diesel::delete(
                    dsl::pending_tells
                        .filter(dsl::server_addr.eq(&cfg.address))
                        .filter(dsl::target_nick.eq(&target_nick))
                        .filter(dsl::channel.eq(&*chan).or(dsl::channel.is_null())),
                ).execute(db)?;
                Ok(())
            }).or_else(|err| {
                srv.send_privmsg(
                    target_nick,
                    "You have some pending tells, but I failed. \
                     Try rejoining, or notifying my owner.",
                )?;
                Err(err)
            })
                .and_then(|_| send_tells(cfg, srv, log, &tells))
        } else {
            unreachable!()
        }
    } else {
        Ok(())
    }
}

pub fn handle_names_reply(
    cfg: &ServerCfg,
    srv: &IrcServer,
    log: &Logger,
    msg: &Message,
) -> Result<()> {
    let hm = PENDING_TELLS.lock();
    let mut pending = hm.get(&cfg.address).unwrap().lock();
    if *pending != 0 {
        if let Command::Response(Response::RPL_NAMREPLY, ref chan, ref users) = msg.command {
            debug_assert_eq!(cfg.nickname, chan[0]);
            let chan = &chan[2];
            let target_nicks = users
                .as_ref()
                .unwrap()
                .split(' ')
                .filter(|u| u != &cfg.nickname)
                .map(|u| {
                    u.replace('%', "")
                        .replace('~', "")
                        .replace('@', "")
                        .replace('+', "")
                        .replace('&', "")
                })
                .collect::<Vec<_>>();

            let tells = super::with_database(cfg, |db| {
                Ok(dsl::pending_tells
                    .filter(dsl::server_addr.eq(&cfg.address))
                    .filter(dsl::target_nick.eq_any(&target_nicks))
                    .filter(dsl::channel.eq(&*chan).or(dsl::channel.is_null()))
                    .load::<models::PendingTell>(db)?)
            })?;
            *pending -= tells.len();
            drop(pending);
            debug!(log, "Found pending tells: {:?}", tells);

            super::with_database(cfg, |db| {
                diesel::delete(
                    dsl::pending_tells
                        .filter(dsl::server_addr.eq(&cfg.address))
                        .filter(dsl::target_nick.eq_any(&target_nicks))
                        .filter(dsl::channel.eq(&*chan).or(dsl::channel.is_null())),
                ).execute(db)?;
                Ok(())
            }).or_else(|err| {
                for target_nick in target_nicks {
                    srv.send_privmsg(
                        &target_nick,
                        "You have some pending tells, but I failed. \
                         Try rejoining, or notifying my owner.",
                    )?;
                }
                Err(err)
            })
                .and_then(|_| send_tells(cfg, srv, log, &tells))
        } else {
            unreachable!()
        }
    } else {
        Ok(())
    }
}

fn send_tells(
    cfg: &ServerCfg,
    srv: &IrcServer,
    log: &Logger,
    tells: &[models::PendingTell],
) -> Result<()> {
    for t in tells {
        let msg = format!(
            "{}: {} wanted to tell you on {} UTC: {}",
            &t.target_nick,
            t.source_nick,
            t.date,
            t.message
        );
        if t.channel.is_some() {
            super::send_segmented_message(cfg, srv, log, t.channel.as_ref().unwrap(), &msg)?;
        } else {
            super::send_segmented_message(cfg, srv, log, &t.target_nick, &msg)?;
        }
    }
    Ok(())
}

pub fn add(cfg: &ServerCfg, log: &Logger, msg: &Message, private: bool) -> Result<String> {
    if let Command::PRIVMSG(ref target, ref content) = msg.command {
        let source_nick = msg.source_nickname().unwrap();

        let mut split = content[6..].splitn(2, ' ');
        let target_nick = split.next().unwrap();
        let target_msg = if let Some(s) = split.next() {
            s
        } else {
            trace!(log, "invalid tell: {:?}", msg);
            return Ok("Invalid `.tell` syntax, try: `.tell <nick> <message>`".into());
        };

        let date = &Utc::now().to_rfc2822()[..25];
        let pending_tell = models::NewPendingTell {
            date: date,
            server_addr: &cfg.address,
            channel: {
                if private {
                    None
                } else {
                    Some(target)
                }
            },
            source_nick: source_nick,
            target_nick: target_nick,
            message: target_msg.trim(),
        };

        super::with_database(cfg, |db| {
            diesel::insert(&pending_tell)
                .into(schema::pending_tells::table)
                .execute(db)?;
            Ok(())
        }).and_then(|_| {
            let mut hm = PENDING_TELLS.lock();
            *hm.get_mut(&cfg.address).unwrap().lock() += 1;

            Ok(format!(
                "{}: I will tell {}: {}",
                source_nick,
                target_nick,
                target_msg.trim()
            ))
        })
    } else {
        unreachable!()
    }
}
