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

use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use irc::client::prelude::*;
use parking_lot::RwLock;
use slog::Logger;
use unicode_segmentation::UnicodeSegmentation;

use std::collections::HashMap;

use config::{Config, ServerCfg};

mod help;
mod tell;
mod weather;

const COMMAND_MODIFIER: char = '.';
// TODO: I'm not sure what the actual limit is, I read that the server may add crap to your msg,
// so there's 30 bytes for that
const MESSAGE_BYTES_LIMIT: usize = 478;

lazy_static!{
    static ref HOSTNAMES: RwLock<HashMap<String, String>> = {
        RwLock::new(HashMap::new())
    };
}

pub fn init(cfg: &Config, log: &Logger) {
    tell::init(cfg, log);
    weather::init(cfg, log);
}

#[allow(needless_pass_by_value)]
pub fn handle(cfg: &ServerCfg, srv: &IrcServer, log: &Logger, msg: Message) {
    match msg.command {
        // Currently uninteresting messages
        Command::NOTICE(..) |
        Command::Response(Response::RPL_WELCOME, ..) |
        Command::Response(Response::RPL_YOURHOST, ..) |
        Command::Response(Response::RPL_CREATED, ..) |
        Command::Response(Response::RPL_MYINFO, ..) |
        Command::Response(Response::RPL_ISUPPORT, ..) |
        Command::Response(Response::RPL_LUSERCLIENT, ..) |
        Command::Response(Response::RPL_LUSEROP, ..) |
        Command::Response(Response::RPL_LUSERUNKNOWN, ..) |
        Command::Response(Response::RPL_LUSERCHANNELS, ..) |
        Command::Response(Response::RPL_LUSERME, ..) |
        Command::Response(Response::RPL_MOTDSTART, ..) |
        Command::Response(Response::RPL_MOTD, ..) |
        Command::Response(Response::RPL_ENDOFMOTD, ..) |
        Command::Response(Response::ERR_NOTREGISTERED, ..) |
        Command::Response(Response::RPL_ENDOFNAMES, ..) |
        Command::Response(Response::RPL_TOPIC, ..) |
        Command::PART(..) |
        Command::ChannelMODE(..) |
        Command::PING(..) |
        Command::PONG(..) |
        Command::QUIT(..) |
        Command::Response(Response::RPL_TOPICWHOTIME, ..) => trace!(log, "{:?}", msg),
        Command::Raw(ref s, ..) if s == "250" || s == "265" || s == "266" => {
            trace!(log, "{:?}", msg)
        }
        Command::Raw(ref s, ..) if s == "MODE" => {
            trace!(log, "Received MODE, hostname: {:?}", msg.prefix);

            HOSTNAMES
                .write()
                .entry(cfg.address.clone())
                .or_insert_with(|| msg.prefix.as_ref().unwrap().clone());
        }
        Command::JOIN(..) => {
            // The case of the bot joining a channel is handled by RPL_NAMREPLY
            if msg.source_nickname().unwrap() != cfg.nickname {
                // We don't check if the module is enabled, because it's our responsibility to
                // deliver the msg asap without fail, even if the bot owner disabled the module;
                // If they *really* want, they can clean the database
                tell::handle_user_join(cfg, srv, log, &msg);
            }
        }
        Command::Response(Response::RPL_NAMREPLY, ..) => {
            // The bot joined a channel, and asked for nicknames to see if they have any
            // pending tells. (NOTE: something, maybe the irc crate, asks automatically)
            tell::handle_names_reply(cfg, srv, log, &msg);
        }
        Command::PRIVMSG(ref target, ref content) => {
            debug!(log, "PRIVMSG to {}: {}", target, content);

            let reply_target = msg.response_target().unwrap();
            // Test if this msg was sent to a channel. When replying, we want to use NOTICE
            let private = !(target == reply_target);

            // Check if msg is a command, handle command/context modules
            if content.chars().nth(0).unwrap() == COMMAND_MODIFIER {
                if &content[1..] == "bots" || &content[1..] == "bot" {
                    trace!(log, "Replying to .bots");
                    let reply = "Beep boop, I'm a bot! For help, try `.help`~";
                    send_segmented_message(cfg, srv, log, reply_target, reply, private);
                } else if content[1..].starts_with("help") {
                    trace!(log, "Replying to .help");
                    let reply = help::handle(cfg, &*target, content, private);
                    send_segmented_message(cfg, srv, log, reply_target, &reply, private);
                } else if (private || module_enabled_channel(cfg, &*target, "tell")) &&
                           content[1..].starts_with("tell")
                {
                    trace!(log, "Starting .tell");
                    let reply = tell::add(cfg, log, &msg, private);
                    send_segmented_message(cfg, srv, log, reply_target, &reply, private);
                } else if (private || module_enabled_channel(cfg, &*target, "weather")) &&
                           content[1..].starts_with("weather")
                {
                    trace!(log, "Starting .weather");
                    let nick = msg.source_nickname().unwrap();
                    let reply = weather::handle(cfg, srv, log, &content[8..], nick);
                    send_segmented_message(cfg, srv, log, reply_target, &reply, private);
                } else {
                    warn!(log, "Unknown command {}", &content[1..]);
                }
            } else {
                // TODO: e.g. URL Regex + fetch
            }
        }
        _ => {
            warn!(log, "Unhandled message: {:?}", msg);
        }
    }
}

fn module_enabled_channel(cfg: &ServerCfg, target: &str, module: &str) -> bool {
    cfg.channels.iter().any(|c| {
        c.name == target && c.modules.iter().any(|m| m == module)
    })
}

fn send_segmented_message(
    cfg: &ServerCfg,
    srv: &IrcServer,
    log: &Logger,
    target: &str,
    msg: &str,
    private: bool,
) {
    let graphemes = UnicodeSegmentation::graphemes(msg, true);
    let msg_bytes = msg.bytes().len();
    // :<hostname> <PRIVMSG|NOTICE> <target> :<message> [potential added escape chars]
    let fix_bytes = 1 + HOSTNAMES.read().get(&cfg.address).unwrap().bytes().len() +
        1 + if private { 7 } else { 6 } + 1 + target.bytes().len() + 2 + 7;
    trace!(log, "Msg bytes: {}; Fix bytes: {}", msg_bytes, fix_bytes);

    let send_err = |msg: &str| {
        if let Err(e) = if private {
            srv.send_privmsg(target, msg)
        } else {
            srv.send_notice(target, msg)
        } {
            crit!(log, "Failed to send message to {}: {:?}", target, e)
        };
    };

    if msg_bytes + fix_bytes <= MESSAGE_BYTES_LIMIT {
        trace!(log, "Message does not exceed limit");
        send_err(msg);
    } else {
        let mut count = 0;
        let mut unescaped_controls = [false, false, false, false, false, false, false];
        let mut msg = String::with_capacity(MESSAGE_BYTES_LIMIT - fix_bytes);
        for g in graphemes {
            let len = g.bytes().len();
            // TODO: Is there any better way to do this?
            // For magic values see  https://stackoverflow.com/questions/1391610/embed-mirc-color-
            // codes-into-a-c-sharp-literal/13382032#13382032
            // FIXME: This is actually broken for color, because we don't insert the specific color
            // code again
            if g == "\x02" {
                if unescaped_controls[0] {
                    unescaped_controls[0] = false;
                } else {
                    unescaped_controls[0] = true;
                }
            } else if g == "\x03" {
                if unescaped_controls[1] {
                    unescaped_controls[1] = false;
                } else {
                    unescaped_controls[1] = true;
                }
            } else if g == "\x09" {
                if unescaped_controls[2] {
                    unescaped_controls[2] = false;
                } else {
                    unescaped_controls[2] = true;
                }
            } else if g == "\x13" {
                if unescaped_controls[3] {
                    unescaped_controls[3] = false;
                } else {
                    unescaped_controls[3] = true;
                }
            } else if g == "\x15" {
                if unescaped_controls[4] {
                    unescaped_controls[4] = false;
                } else {
                    unescaped_controls[4] = true;
                }
            } else if g == "\x1f" {
                if unescaped_controls[5] {
                    unescaped_controls[5] = false;
                } else {
                    unescaped_controls[5] = true;
                }
            } else if g == "\x16" {
                if unescaped_controls[6] {
                    unescaped_controls[6] = false;
                } else {
                    unescaped_controls[6] = true;
                }
            }
            if count + len >= MESSAGE_BYTES_LIMIT - fix_bytes {
                let if_any_unescaped_push = |out: &mut String| if unescaped_controls[0] {
                    out.push_str("\x02");
                } else if unescaped_controls[1] {
                    out.push_str("\x03");
                } else if unescaped_controls[2] {
                    out.push_str("\x09");
                } else if unescaped_controls[3] {
                    out.push_str("\x13");
                } else if unescaped_controls[4] {
                    out.push_str("\x15");
                } else if unescaped_controls[5] {
                    out.push_str("\x1f");
                } else if unescaped_controls[6] {
                    out.push_str("\x16");
                };

                if_any_unescaped_push(&mut msg);
                trace!(log, "Sending {} cut msg: {:?}", target, &msg);

                send_err(&msg);
                count = 0;
                msg.clear();
                if_any_unescaped_push(&mut msg);
            }
            count += len;
            msg.push_str(g);
        }
        if !msg.is_empty() {
            send_err(&msg);
        }
    }
}

fn establish_database_connection(cfg: &ServerCfg, log: &Logger) -> SqliteConnection {
    let ret = SqliteConnection::establish(&cfg.database);
    if ret.is_err() {
        // The T does not impl Debug, so no .unwrap_err
        if let Err(e) = ret {
            crit!(log, "Failed to connect to database {}: {}", cfg.database, e);
            panic!("Failed to connect to database {}: {}", cfg.database, e)
        } else {
            unreachable!()
        }
    } else {
        trace!(
            log,
            "Successfully established connection to {}",
            cfg.database
        );
        ret.unwrap()
    }
}
