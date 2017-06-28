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

use hyper::client;
use irc::client::prelude::*;
use regex::Regex;
use slog::Logger;

use config::ServerCfg;

pub fn handle(cfg: &ServerCfg, srv: &IrcServer, log: &Logger, msg: &str) -> String {
    // Only compile the regex once
    lazy_static! {
        static ref REGEX: Regex = Regex::new("\
        	(?P<plus>\\+){0,1}\
        	(?P<digits>\\d+){0,1}\
        	(?:(?P<h>h)|\
        	    (?P<d>d)|\
        	    (?:\
        	   	    \\s{0,}\
	        	    (?: (?P<hours>hours)|\
	        	    	(?P<days>days)))\
        	){0,1}\
        	\\s{0,}\
        	(?P<location>.+)").unwrap();
    }
    let captures = if let Some(caps) = REGEX.captures(msg) {
        trace!(log, "Weather captures: {:?}", caps);
        caps
    } else {
        debug!(log, "No captures");
        return "Invalid `.weather` syntax, try: `.help weather`".into();
    };

    let (future, n, hours, days, location) = {
        (
            captures.name("plus").is_some(),
            captures.name("digits"),
            captures.name("h").is_some() || captures.name("hours").is_some(),
            captures.name("d").is_some() || captures.name("days").is_some(),
            captures.name("location"),
        )
    };
    let location = if location.is_none() {
        debug!(log, "No location found");
        return "Invalid `.weather` syntax, try: `.help weather`".into();
    } else {
        location.unwrap()
    };

    unimplemented!()
}
