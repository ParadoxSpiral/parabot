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
use regex::Regex;
use reqwest;
use slog::Logger;
use unicode_segmentation::UnicodeSegmentation;

use std::collections::HashMap;

use config::{Config, ServerCfg};
use errors::*;

mod ddg;
mod help;
mod tell;
pub mod url;
mod weather;

const COMMAND_MODIFIER: char = '.';
// The spec does not define a limit, but it's 500b in most cases. However, the server may
// add crap to your message, you cannot know. Hopefully 30b is enough..
const MESSAGE_BYTES_LIMIT: usize = 470;

lazy_static!{
    static ref HOSTNAMES: RwLock<HashMap<String, String>> = {
        RwLock::new(HashMap::new())
    };
}

pub fn init(cfg: &Config, log: &Logger) -> Result<()> {
    tell::init(cfg, log)?;
    weather::init(cfg, log)
}

pub fn handle(cfg: &ServerCfg, srv: &IrcServer, log: &Logger, msg: &Message) -> Result<()> {
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
        Command::Response(Response::ERR_NOCHANMODES, ref content, ..) => {
            // Happens if the bot tries to join a protected channel before registration
            debug!(
                log,
                "Probably joined protected channel {:?} before registration, rejoining",
                content[1]
            );
            if let Some(key) = cfg.channels
                .iter()
                .find(|c| c.name == content[1])
                .unwrap()
                .password
                .clone()
            {
                srv.send_join_with_keys(&content[1], &key)?
            } else {
                srv.send_join(&content[1])?
            }
        }
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
                tell::handle_user_join(cfg, srv, log, &msg)?;
            }
        }
        Command::Response(Response::RPL_NAMREPLY, ..) => {
            // The bot joined a channel, and asked for nicknames to see if they have any
            // pending tells. (NOTE: something, maybe the irc crate, asks automatically)
            tell::handle_names_reply(cfg, srv, log, &msg)?;
        }
        Command::PRIVMSG(ref target, ref content) => {
            debug!(log, "PRIVMSG to {}: {}", target, content);
            let reply_target = msg.response_target().unwrap();
            let private = !(target == reply_target);

            // Check if msg is a command, handle command/context modules
            if content.chars().nth(0).unwrap() == COMMAND_MODIFIER {
                if &content[1..] == "bots" || &content[1..] == "bot" {
                    trace!(log, "Replying to .bots");
                    // TODO: Add owner config option/ bots reply
                    let reply = "I am the slave of ParadoxSpiral… at least 123% safer & faster \
                                 than m's & l's shit. For a list of commands, try `.help`";
                    send_segmented_message(cfg, srv, log, reply_target, reply, false)?;
                } else if content[1..].starts_with("help") {
                    trace!(log, "Replying to .help");
                    let reply = help::handle(cfg, &*target, content, private);
                    send_segmented_message(
                        cfg,
                        srv,
                        log,
                        msg.source_nickname().unwrap(),
                        &reply,
                        true,
                    )?;
                } else if (private || module_enabled_channel(cfg, &*target, "tell")) &&
                           content[1..].starts_with("tell")
                {
                    trace!(log, "Starting .tell");
                    let reply = tell::add(cfg, log, &msg, private)?;
                    send_segmented_message(cfg, srv, log, reply_target, &reply, false)?;
                } else if (private || module_enabled_channel(cfg, &*target, "duckduckgo")) &&
                           content[1..].starts_with("ddg")
                {
                    trace!(log, "Starting .ddg");
                    let reply = ddg::handle(cfg, content[4..].trim(), true, false)?;
                    send_segmented_message(cfg, srv, log, reply_target, &reply, false)?;
                } else if (private || module_enabled_channel(cfg, &*target, "wolframalpha")) &&
                           content[1..].starts_with("wa")
                {
                    trace!(log, "Starting .ddg !wa");
                    let reply = ddg::handle(
                        cfg,
                        &("!wa ".to_owned() + content[3..].trim()),
                        false,
                        false,
                    )?;
                    send_segmented_message(cfg, srv, log, reply_target, &reply, false)?;
                } else if (private || module_enabled_channel(cfg, &*target, "jisho")) &&
                           content[1..].starts_with("jisho")
                {
                    trace!(log, "Starting .ddg !jisho");
                    let reply = ddg::handle(
                        cfg,
                        &("!jisho ".to_owned() + content[6..].trim()),
                        false,
                        false,
                    )?;
                    send_segmented_message(cfg, srv, log, reply_target, &reply, false)?;
                } else if (private || module_enabled_channel(cfg, &*target, "weather")) &&
                           content[1..].starts_with("weather")
                {
                    trace!(log, "Starting .weather");
                    let nick = msg.source_nickname().unwrap();
                    let reply = weather::handle(cfg, srv, log, &content[8..], nick)?;
                    send_segmented_message(cfg, srv, log, reply_target, &reply, false)?;
                } else {
                    warn!(log, "Unknown command {}", &content[1..]);
                }
            } else if private || module_enabled_channel(cfg, &*target, "url-info") {
                lazy_static! (
                        static ref URL_REGEX: Regex = Regex::new("\
                            .*?\
                            (?:\
                                (?:\
                                    <\
                                    (?P<url_v1>\
                                        (?P<protocol_v1>(?:(?:http)|(?:https))://){0,1}\
                                            (?:\
                                                [^\\s]*?\
                                                \\.\
                                            ){0,1}\
                                        [^\\s]*?\
                                        \\.{1}\
                                        [^\\s]*\
                                    )\
                                    >\
                                )|\
                                (?:\
                                    (?P<url_v2>\
                                        (?P<protocol_v2>(?:(?:http)|(?:https))://){0,1}\
                                            (?:\
                                                [^\\s]*?\
                                                \\.\
                                            ){0,1}\
                                        [^\\s]*?\
                                        \\.{1}\
                                        [^\\s]*\
                            )))\
                            .*?\
                            ").unwrap();
                );
                for cap in URL_REGEX.captures_iter(content) {
                    let url = cap.name("url_v1").or_else(|| cap.name("url_v2"));
                    let proto = cap.name("protocol_v1").or_else(|| cap.name("protocol_v2"));
                    if let Some(url) = url {
                        trace!(log, "URL match: {:?}", url);
                        let url = if proto.is_none() {
                            // Fuck everything that uses http in these let's encrypt days
                            let mut u = String::with_capacity(url.as_str().len() + 8);
                            u.push_str(url.as_str());
                            u.push_str("https://");
                            u
                        } else {
                            url.as_str().to_owned()
                        };
                        let res = reqwest::get(&url);
                        if let Ok(res) = res {
                            if res.status().is_success() {
                                // FIXME: Can be made to (elegantly) not clone with NLL
                                let domain = res.url().domain().unwrap().to_owned();
                                if private ||
                                    !cfg.channels.iter().any(|c| {
                                        &*c.name == &*target &&
                                            c.url_blacklisted_domains
                                                .iter()
                                                .any(|ds| ds.iter().any(|d| &*d == &*domain))
                                    }) {
                                    let reply_target = msg.response_target().unwrap();
                                    let reply = url::handle(cfg, res, true)?;
                                    send_segmented_message(
                                        cfg,
                                        srv,
                                        log,
                                        reply_target,
                                        &reply,
                                        false,
                                    )?;
                                }
                            }
                        } else {
                            trace!(log, "Failed reqwest; Res: {:?}", res);
                        }
                    }
                }
            }
        }
        _ => {
            warn!(log, "Unhandled message: {:?}", msg);
        }
    }
    Ok(())
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
    notice: bool,
) -> Result<()> {
    let msg_bytes = msg.bytes().len();
    // :<hostname> <PRIVMSG|NOTICE> <target> :\u{200B}<message>
    let fix_bytes = 1 + HOSTNAMES.read().get(&cfg.address).unwrap().bytes().len() + 1 +
        if notice { 6 } else { 7 } + 1 + target.bytes().len() + 3;
    trace!(log, "Msg bytes: {}; Fix bytes: {}", msg_bytes, fix_bytes);

    let send = |msg: &str| if notice {
        srv.send_notice(target, &msg.replace("\n", " "))
    } else {
        srv.send_privmsg(target, &msg.replace("\n", " "))
    };

    if msg_bytes + fix_bytes <= MESSAGE_BYTES_LIMIT {
        trace!(log, "Message does not exceed limit: {}", msg);
        send(&("\u{200B}".to_owned() + msg))?;
    } else {
        let mut count = 0;
        let mut unescaped_controls = [false, false, false, false, false, false, false];
        let mut color_code = String::with_capacity(5);
        let mut current_msg = String::with_capacity(MESSAGE_BYTES_LIMIT - fix_bytes);
        current_msg.push_str("\u{200B}");
        let mut graphemes = UnicodeSegmentation::graphemes(msg, true).peekable();
        // We don't use a for loop because we need to mutably access graphemes below
        loop {
            if let Some(next) = graphemes.next() {
                // For magic values see https://stackoverflow.com/questions/1391610/embed-
                //mirc-color-codes-into-a-c-sharp-literal/13382032#13382032
                if next == "\x02" {
                    if unescaped_controls[0] {
                        count -= 1;
                        unescaped_controls[0] = false;
                    } else {
                        count += 1;
                        unescaped_controls[0] = true;
                    }
                } else if next == "\x03" {
                    if unescaped_controls[1] {
                        count -= 1 + color_code.len();
                        color_code.clear();
                        unescaped_controls[1] = false;
                    } else {
                        // worst case: \x0315,15
                        let first = graphemes.next().unwrap();
                        if *graphemes.peek().unwrap() == "," {
                            // \x031,1
                            let _ = graphemes.next().unwrap();
                            let second = graphemes.next().unwrap();
                            if graphemes.peek().unwrap().parse::<usize>().is_ok() {
                                // \x031,15
                                let third = graphemes.next().unwrap();
                                count += 5;
                                color_code.push_str(first);
                                color_code.push_str(",");
                                color_code.push_str(second);
                                color_code.push_str(third);
                            } else {
                                // \x031,1
                                count += 4;
                                color_code.push_str(first);
                                color_code.push_str(",");
                                color_code.push_str(second);
                            }
                        } else if graphemes.peek().unwrap().parse::<usize>().is_ok() {
                            // \x0315
                            let second = graphemes.next().unwrap();
                            if *graphemes.peek().unwrap() == "," {
                                // \x0315,1
                                let _ = graphemes.next().unwrap();
                                let third = graphemes.next().unwrap();
                                if graphemes.peek().unwrap().parse::<usize>().is_ok() {
                                    // \x0315,15
                                    count += 6;
                                    let fourth = graphemes.next().unwrap();
                                    color_code.push_str(first);
                                    color_code.push_str(second);
                                    color_code.push_str(",");
                                    color_code.push_str(third);
                                    color_code.push_str(fourth);
                                } else {
                                    // \x0315,1
                                    count += 5;
                                    color_code.push_str(first);
                                    color_code.push_str(second);
                                    color_code.push_str(",");
                                    color_code.push_str(third);
                                }
                            } else {
                                // \x0315
                                let second = graphemes.next().unwrap();
                                count += 3;
                                color_code.push_str(first);
                                color_code.push_str(second);
                            }
                        } else {
                            // \x031
                            count += 2;
                            color_code.push_str(first);
                        }
                        unescaped_controls[1] = true;
                    }
                } else if next == "\x09" {
                    if unescaped_controls[2] {
                        count -= 1;
                        unescaped_controls[2] = false;
                    } else {
                        count += 1;
                        unescaped_controls[2] = true;
                    }
                } else if next == "\x13" {
                    if unescaped_controls[3] {
                        count -= 1;
                        unescaped_controls[3] = false;
                    } else {
                        count += 1;
                        unescaped_controls[3] = true;
                    }
                } else if next == "\x15" {
                    if unescaped_controls[4] {
                        count -= 1;
                        unescaped_controls[4] = false;
                    } else {
                        count += 1;
                        unescaped_controls[4] = true;
                    }
                } else if next == "\x1f" {
                    if unescaped_controls[5] {
                        count -= 1;
                        unescaped_controls[5] = false;
                    } else {
                        count += 1;
                        unescaped_controls[5] = true;
                    }
                } else if next == "\x16" {
                    if unescaped_controls[6] {
                        count -= 1;
                        unescaped_controls[6] = false;
                    } else {
                        count += 1;
                        unescaped_controls[6] = true;
                    }
                }

                let len = next.bytes().len();
                if count + len > MESSAGE_BYTES_LIMIT - fix_bytes {
                    let if_any_unescaped_push = |out: &mut String, new_line| {
                        if unescaped_controls[0] {
                            out.push_str("\x02");
                        }
                        if unescaped_controls[1] {
                            out.push_str("\x03");
                            if new_line {
                                out.push_str(&color_code);
                            }
                        }
                        if unescaped_controls[2] {
                            out.push_str("\x09");
                        }
                        if unescaped_controls[3] {
                            out.push_str("\x13");
                        }
                        if unescaped_controls[4] {
                            out.push_str("\x15");
                        }
                        if unescaped_controls[5] {
                            out.push_str("\x1f");
                        }
                        if unescaped_controls[6] {
                            out.push_str("\x16");
                        }
                    };

                    if_any_unescaped_push(&mut current_msg, false);
                    trace!(log, "Sending {} cut msg: {:?}", target, &current_msg);

                    send(&current_msg)?;
                    count = 0;
                    current_msg.clear();
                    current_msg.push_str("\u{200B}");
                    if_any_unescaped_push(&mut current_msg, true);
                }
                count += len;
                current_msg.push_str(next);
            } else {
                if !current_msg.is_empty() {
                    send(&current_msg)?;
                }
                break;
            }
        }
    }
    Ok(())
}

fn establish_database_connection(cfg: &ServerCfg, log: &Logger) -> Result<SqliteConnection> {
    SqliteConnection::establish(&cfg.database)
        .or_else(|err| {
            crit!(
                log,
                "Failed to connect to database {}: {}",
                cfg.database,
                err
            );
            Err(ErrorKind::DieselConn(err).into())
        })
        .and_then(|db| {
            trace!(
                log,
                "Successfully established connection to {}",
                cfg.database
            );
            Ok(db)
        })
}
