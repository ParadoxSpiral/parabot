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

use super::config::Server;

const COMMAND_MODIFIER: &'static str = ".";
// https://tools.ietf.org/html/rfc2812#section-1.3
const CHANNEL_PREFIXES: &'static [char] = &['#', '&', '+', '!'];

pub fn handle(cfg: &Server, srv: &IrcServer, log: &Logger, msg: Message) {
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
        Command::Response(Response::RPL_NAMREPLY, ..) |
        Command::Response(Response::RPL_ENDOFNAMES, ..) |
        Command::Response(Response::RPL_TOPIC, ..) |
        Command::Join(..) => trace!(log, "{:?}", msg),
        Command::Raw(ref s, ..) if s == "MODE" || s == "265" || s == "266" => {
            trace!(log, "{:?}", msg)
        }
        Command::PRIVMSG(target, content) => {
            debug!(log, "private message to {}: {}", target, content);

            // Test if this msg was sent to a channel. When replying, 
            // we want to use NOTICE in that case
            let first_char = target.chars().nth(1).unwrap();
            let chan_msg = CHANNEL_PREFIXES.iter().any(|p| &first_char == p);
            trace!(log, "channel message: {}", chan_msg);
        }
        _ => {
            warn!(log, "Unhandled message: {:?}", msg);
        }
    }
}
