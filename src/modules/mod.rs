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

// TODO: Possibly switch to clap for msg parsing

use irc::client::prelude::*;
use parking_lot::RwLock;
use slog::Logger;
use unicode_segmentation::UnicodeSegmentation;

use std::collections::HashMap;

use super::config::{Config, ServerCfg};

mod tell;

const COMMAND_MODIFIER: char = '.';
// https://tools.ietf.org/html/rfc2812#section-1.3
const CHANNEL_PREFIXES: &[char] = &['#', '&', '+', '!'];
// TODO: I'm not sure what the actual limit is, I read that the server may add crap to your msg,
// so there's 30 bytes for that
const MESSAGE_BYTES_LIMIT: usize = 482;

lazy_static!{
    static ref HOSTNAMES: RwLock<HashMap<String, String>> = {
        RwLock::new(HashMap::new())
    };
}

pub fn init(cfg: &Config, log: &Logger) {
    tell::init(cfg, log);
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
        Command::ChannelMODE(..) => trace!(log, "{:?}", msg),
        Command::Raw(ref s, ..) if s == "MODE" => {
            trace!(log, "Received MODE, hostname: {:?}", msg.prefix);

            HOSTNAMES
                .write()
                .entry(cfg.address.clone())
                .or_insert_with(||msg.prefix.as_ref().unwrap().clone());
        }
        Command::Raw(ref s, ..) if s == "250" || s == "265" || s == "266" => {
            trace!(log, "{:?}", msg)
        }
        Command::JOIN(..) => {
            // The case of the bot joining a channel is handled by RPL_NAMREPLY
            if msg.source_nickname().unwrap() != cfg.nickname {
                // We don't check if the module is enabled, because it's our responsibility to
                // deliver the msg asap without fail, even if the bot owner disabled the module;
                // If they *really* want, they can clean the database
                tell::handle_join(cfg, srv, log, &msg);
            }
        }
        Command::Response(Response::RPL_NAMREPLY, ..) => {
            // The bot joined a channel, and asked for nicknames to see if they have any
            // pending tells. (NOTE: something, maybe the irc crate, asks automatically)
            tell::handle_reply(cfg, srv, log, &msg);
        }
        Command::PRIVMSG(ref target, ref content) => {
            debug!(log, "PRIVMSG to {}: {}", target, content);

            // Test if this msg was sent to a channel. When replying,
            // we want to use NOTICE in that case
            let first_char = target.chars().nth(0).unwrap();
            let chan_msg = CHANNEL_PREFIXES.iter().any(|p| &first_char == p);
            let priv_or_notice = |to_send: &str| {
                if let Err(e) = if chan_msg {
                    srv.send_notice(target, to_send)
                } else {
                    srv.send_privmsg(msg.source_nickname().unwrap(), to_send)
                } {
                    crit!(
                        log,
                        "Failed to send message to {}: {:?}",
                        if chan_msg {
                            target
                        } else {
                            msg.source_nickname().unwrap()
                        },
                        e
                    )
                };
            };
            trace!(log, "in channel: {}", chan_msg);

            // Check if msg is a command, handle command/context modules
            if content.chars().nth(0).unwrap() == COMMAND_MODIFIER {
                if &content[1..] == "bots" {
                    trace!(log, "Replying to .bots");
                    priv_or_notice("Beep boop, I'm a bot!");
                } else if content[1..].starts_with("help") {
                    let reply = if &content[1..] == "help" {
                        trace!(log, "Replying to .help");
                        if chan_msg {
                            format!(
                                "Hi! For more information, use .help <module>. \
                                 Enabled modules: {:?}",
                                cfg.channels
                                    .iter()
                                    .find(|c| *c.name == *target)
                                    .unwrap()
                                    .modules
                                    .as_slice()
                            )
                        } else {
                            "Hi! For more information, use .help <module>. This is a private \
                             message, so I cannot tell you about any channel's enabled modules."
                                .to_owned()
                        }
                    } else {
                        // Starts with help, e.g. more args
                        match &content[6..] {
                            "bots" | ".bots" => {
                                ".bots will (hopefully) cause all bots in the channel to reply."
                                    .to_owned()
                            }
                            "tell" | ".tell" => {
                                ".tell <nick> <message> will tell the user with <nick> <message>, \
                                 when they join a channel shared with me."
                                    .to_owned()
                            }
                            "weather" | ".weather" => {
                                unimplemented!();
                                ".weather . (Powered by Dark Sky)".to_owned()
                            }
                            "wa" | ".wa" => {
                            	unimplemented!();
                                ".wa <query> will query wolfram-alpha about <query>.".to_owned()
                            }
                            "url" => {
                            	unimplemented!();
                                "url fetches urls posted in the channel, and displays their \
                                 metadata, and depending on the website, \
                                 more e.g. youtube views."
                                    .to_owned()
                            }
                            _ => "Unknown or undocumented module, sorry.".to_owned(),
                        }
                    };
                    send_segmented_message(
                        cfg,
                        srv,
                        log,
                        if chan_msg {
                            target
                        } else {
                            msg.source_nickname().unwrap()
                        },
                        &reply,
                        !chan_msg,
                    );
                } else if (!chan_msg || module_enabled_channel(cfg, &*target, "tell")) &&
                           content[1..].starts_with("tell")
                {
                    trace!(log, "Starting .tell");
                    priv_or_notice(&tell::add(cfg, log, !chan_msg, &msg));
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
    // :<hostname> <PRIVMSG|NOTICE> <target> :<message>
    let fix_bytes = 1 + HOSTNAMES.read().get(&cfg.address).unwrap().bytes().len() + 1 +
        if private { 7 } else { 6 } + 1 + target.bytes().len() + 2;
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
        let mut count = fix_bytes;
        let mut msg = String::with_capacity(MESSAGE_BYTES_LIMIT - fix_bytes);
        for g in graphemes {
            let len = g.bytes().len();
            if count + len >= MESSAGE_BYTES_LIMIT - fix_bytes {
                trace!(log, "Sending {} cut msg: {:?}", target, &msg);
                send_err(&msg);
                count = fix_bytes;
                msg.clear();
            }
            count += len;
            msg.push_str(g);

        }
        if !msg.is_empty() {
            send_err(&msg);
        }
    }
}
