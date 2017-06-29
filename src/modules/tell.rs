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
use models;
use schema;
use schema::pending_tells::dsl;

lazy_static!{
    static ref PENDING_TELLS_HM: Mutex<HashMap<String, Mutex<usize>>> = {
        Mutex::new(HashMap::new())
    };
}

// Read DB to get init values
pub fn init(cfg: &Config, log: &Logger) {
    let mut hm = PENDING_TELLS_HM.lock();
    for srv in &cfg.servers {
        let conn = super::establish_database_connection(srv, log);
        let tells = dsl::pending_tells
            .filter(dsl::server_addr.eq(&srv.address))
            .load::<models::PendingTell>(&conn);
        if tells.is_err() {
            crit!(
                log,
                "Failed to load pending tells: {:?}",
                tells.as_ref().unwrap_err()
            );
            panic!("Failed to load pending tells: {:?}", tells.unwrap_err())
        } else {
            debug!(log, "Pending tells: {:?}", tells.as_ref().unwrap());
            hm.insert(srv.address.clone(), Mutex::new(tells.unwrap().len()));
            hm.shrink_to_fit();
        }
    }
}

pub fn handle_user_join(cfg: &ServerCfg, srv: &IrcServer, log: &Logger, msg: &Message) {
    let hm = PENDING_TELLS_HM.lock();
    let mut pending = hm.get(&cfg.address).unwrap().lock();
    if *pending != 0 {
        if let Command::JOIN(ref chan, ..) = msg.command {
            let target_nick = msg.source_nickname().unwrap();

            let conn = super::establish_database_connection(cfg, log);
            let tells = dsl::pending_tells
                .filter(dsl::server_addr.eq(&cfg.address))
                .filter(dsl::target_nick.eq(&target_nick))
                .filter(dsl::channel.eq(&*chan).or(dsl::channel.is_null()))
                .load::<models::PendingTell>(&conn);
            let tells = if let Err(e) = tells {
                crit!(log, "Failed to load pending tells: {:?}", e);
                panic!("")
            } else {
                tells.unwrap()
            };

            *pending -= 1;
            drop(pending);
            debug!(log, "Found pending tells: {:?}", tells);

            if let Err(e) = diesel::delete(
                dsl::pending_tells
                    .filter(dsl::server_addr.eq(&cfg.address))
                    .filter(dsl::target_nick.eq(&target_nick))
                    .filter(dsl::channel.eq(&*chan).or(dsl::channel.is_null())),
            ).execute(&conn)
            {
                crit!(log, "Failed to delete tells: {:?}", e);
                let res = srv.send_privmsg(
                    target_nick,
                    "You have some pending tells, but I failed \
                     at a step. Try rejoining, or notifying my owner.",
                );
                if let Err(e) = res {
                    crit!(log, "Failed to send message to {}: {}", &target_nick, e);
                }
            }

            send_tells(cfg, srv, log, &tells);
        } else {
            unreachable!()
        }
    }
}

pub fn handle_names_reply(cfg: &ServerCfg, srv: &IrcServer, log: &Logger, msg: &Message) {
    let hm = PENDING_TELLS_HM.lock();
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
                .map(|u| u.replace('@', "").replace('+', ""))
                .collect::<Vec<_>>();

            let conn = super::establish_database_connection(cfg, log);
            let tells = dsl::pending_tells
                .filter(dsl::server_addr.eq(&cfg.address))
                .filter(dsl::target_nick.eq_any(&target_nicks))
                .filter(dsl::channel.eq(&*chan).or(dsl::channel.is_null()))
                .load::<models::PendingTell>(&conn);
            let tells = if let Err(e) = tells {
                crit!(log, "Failed to load pending tells: {:?}", e);
                panic!("")
            } else {
                tells.unwrap()
            };
            *pending -= tells.len();
            drop(pending);
            debug!(log, "Found pending tells: {:?}", tells);

            if let Err(e) = diesel::delete(
                dsl::pending_tells
                    .filter(dsl::server_addr.eq(&cfg.address))
                    .filter(dsl::target_nick.eq_any(&target_nicks))
                    .filter(dsl::channel.eq(&*chan).or(dsl::channel.is_null())),
            ).execute(&conn)
            {
                crit!(log, "Failed to delete tells: {:?}", e);
                for nick in &target_nicks {
                    let res = srv.send_privmsg(
                        nick,
                        "You have some pending tells, but I failed \
                         at a step. Try rejoining, or notifying my owner.",
                    );
                    if let Err(e) = res {
                        crit!(log, "Failed to send message to {}: {}", nick, e);
                    }
                }
                panic!("");
            }

            send_tells(cfg, srv, log, &tells);
        } else {
            unreachable!()
        }
    }
}

fn send_tells(cfg: &ServerCfg, srv: &IrcServer, log: &Logger, tells: &[models::PendingTell]) {
    for t in tells {
        let msg = format!(
            "{}: {} wanted to tell you on {} UTC: {}",
            &t.target_nick,
            t.source_nick,
            t.date,
            t.message
        );
        super::send_segmented_message(cfg, srv, log, &t.target_nick, &msg, t.channel.is_none());
    }
}

pub fn add(cfg: &ServerCfg, log: &Logger, msg: &Message, private: bool) -> String {
    if let Command::PRIVMSG(ref target, ref content) = msg.command {
        let source_nick = msg.source_nickname().unwrap();

        let mut split = content[6..].splitn(2, ' ');
        let target_nick = split.next().unwrap();
        let target_msg = if let Some(s) = split.next() {
            s
        } else {
            trace!(log, "invalid tell: {:?}", msg);
            return "Invalid `.tell` syntax, try: `.tell <nick> <message>`".into();
        };

        let date = &Utc::now().to_rfc2822()[..26];
        let pending_tell = models::NewPendingTell {
            date: date,
            server_addr: &cfg.address,
            channel: { if private { None } else { Some(target) } },
            source_nick: source_nick,
            target_nick: target_nick,
            message: target_msg,
        };

        let conn = super::establish_database_connection(cfg, log);
        let res = diesel::insert(&pending_tell)
            .into(schema::pending_tells::table)
            .execute(&conn);
        if res.is_err() {
            crit!(
                log,
                "Failed to insert {:?} into {}",
                pending_tell,
                cfg.database
            );
            panic!("");
        } else {
            let mut hm = PENDING_TELLS_HM.lock();
            *hm.get_mut(&cfg.address).unwrap().lock() += 1;

            format!(
                "{}: I will tell {}: {}",
                source_nick,
                target_nick,
                target_msg
            )
        }
    } else {
        unreachable!()
    }
}
