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

use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use regex::Regex;

use crate::prelude::*;

#[derive(Module)]
#[module(
    help = "'.roll [x]d<n>[, …]': roll 1 or more x (or 1) n sided dice",
    received(handle_received)
)]
pub struct Dice {
    regex: Regex,
}

impl Dice {
    pub fn new() -> Self {
        Dice {
            // Use a fairly vague regex (e.g. . instead of \d) to catch & print errors
            regex: Regex::new(r"(?P<count>.*?)d(?P<sides>[^,\s]*),{0,}\s*").unwrap(),
        }
    }

    fn handle_received(
        &mut self,
        _: &Arc<IrcClient>,
        mctx: &MessageContext,
        _: &mut ModuleCfg,
        msg: &Message,
        trigger: Trigger,
    ) {
        let to_roll = match trigger {
            Trigger::Explicit(r) => r,
            Trigger::Action(r) => r,
            _ => panic!("dice module's triggers wrongly configured"),
        };

        let mut roll = 0;
        let mut err_count = String::new();
        let mut err_sides = String::new();
        for cap in self.regex.captures_iter(to_roll) {
            let mut c_err = false;
            let count = cap.name("count").unwrap().as_str();
            let count = if count.is_empty() {
                1
            } else {
                match count.parse::<usize>() {
                    Ok(c) => c,
                    Err(_) => {
                        if err_count.is_empty() {
                            err_count += &*format!(" `{}`", count);
                        } else {
                            err_count += &*format!(", `{}`", count);
                        }
                        c_err = true;
                        0
                    }
                }
            };
            let sides = cap.name("sides").unwrap().as_str();
            let sides = match sides.parse::<usize>() {
                Ok(s) => s,
                Err(_) => {
                    if err_sides.is_empty() {
                        err_sides += &*format!(" `{}`", sides);
                    } else {
                        err_sides += &*format!(", `{}`", sides);
                    }
                    continue;
                }
            };
            if c_err {
                continue;
            }

            if err_count.is_empty() && err_sides.is_empty() {
                let sampler = Uniform::new_inclusive(1, sides);
                for _ in 0..count {
                    roll += sampler.sample(&mut thread_rng());
                }
            }
        }

        let mut err = String::new();
        if !err_count.is_empty() {
            err += &*format!("Invalid counts:{}", err_count);
        }
        if !err_sides.is_empty() {
            if err.is_empty() {
                err += &*format!("Invalid sides:{}", err_sides);
            } else {
                err += &*format!("; Invalid sides:{}", err_sides);
            }
        }

        if err.is_empty() {
            reply_priv_pub!(mctx, msg, "{}", roll; "{} rolled {}", no_mention!(msg.source_nickname().unwrap().to_owned()), roll);
        } else {
            reply!(mctx, msg, "{}", err);
        }
    }
}
