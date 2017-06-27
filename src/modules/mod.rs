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

use irc::client::prelude::*;
use slog::Logger;

use super::config::{Config, ServerCfg};

mod tell;

const COMMAND_MODIFIER: &str = ".";
// https://tools.ietf.org/html/rfc2812#section-1.3
const CHANNEL_PREFIXES: &[char] = &['#', '&', '+', '!'];

pub fn init(cfg: &Config, log: &Logger) {
    tell::init(cfg, log);
}

pub fn handle(cfg: &ServerCfg, srv: &IrcServer, log: &Logger, msg: Message) {
    match msg.command {
        // All the mostly uninteresting information
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
        Command::PART(..) => trace!(log, "{:?}", msg),
        Command::Raw(ref s, ..) if s == "MODE" || s == "250" || s == "265" || s == "266" => {
            trace!(log, "{:?}", msg)
        }
        Command::JOIN(..) => {
            // The case of the bot joining a channel, is handled by RPL_NAMREPLY
            if msg.source_nickname().unwrap() != cfg.nickname {
                // We don't check if the module is enabled, because it's our responsibility to
                // deliver the msg asap without fail, even if the bot owner disabled the module;
                // If they *really* want, they can clean the database
                tell::handle(cfg, srv, log, &msg, false);
            }
        }
        Command::Response(Response::RPL_NAMREPLY, ..) => {
            // The bot joined a channel, and asked for nicknames to see if they have any
            // pending tells. (NOTE: something, maybe the irc crate, asks automatically)
            tell::handle(cfg, srv, log, &msg, true);
        }
        Command::PRIVMSG(ref target, ref content) => {
            debug!(log, "PRIVMSG to {}: {}", target, content);

            // Test if this msg was sent to a channel. When replying,
            // we want to use NOTICE in that case
            let first_char = target.chars().nth(0).unwrap();
            let chan_msg = CHANNEL_PREFIXES.iter().any(|p| &first_char == p);
            let priv_or_notice = |src: &Message, msg: &str| {
                if let Err(e) = if chan_msg {
                    srv.send_notice(&target, msg)
                } else {
                    srv.send_privmsg(src.source_nickname().unwrap(), msg)
                } {
                    crit!(
                        log,
                        "Failed to send message to {}: {:?}",
                        if chan_msg {
                            target
                        } else {
                            src.source_nickname().unwrap()
                        },
                        e
                    )
                };
            };
            trace!(log, "in channel: {}", chan_msg);

            // Check if msg is a command, handle command/context modules
            if &content[..1] == COMMAND_MODIFIER {
                if &content[1..] == "bots" {
                    trace!(log, "Replying to .bots");
                    priv_or_notice(&msg, "Beep boop, I'm a bot!");
                } else if (!chan_msg || module_enabled_channel(cfg, &*target, "tell")) &&
                           content[1..].starts_with("tell")
                {
                    priv_or_notice(&msg, &tell::add(cfg, log, !chan_msg, &msg));
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
        &c.name == target && c.modules.iter().any(|m| &*m == module)
    })
}
