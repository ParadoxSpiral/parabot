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

use irc::client::server::IrcServer;
use irc::proto::message::Message;
use slog::Logger;

use super::config::Server;
use super::errors::*;

const COMMAND_MODIFIER: &'static str = ".";

pub fn handle(cfg: &Server, srv: &IrcServer, log: &Logger, msg: Message) {
    debug!(log, "Received message: {:?}", msg);
}
