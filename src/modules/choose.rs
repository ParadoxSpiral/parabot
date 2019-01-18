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

use rand::{seq::SliceRandom, thread_rng};
use shlex;

use crate::prelude::*;

pub struct Choose;

module!(Choose, Stage::MessageReceived;
    |_| ".choose \"one\" option\\ of 'some'".to_owned();
    received => |_, _, mctx: &MessageContext, _, msg: &Message, trigger| {
        if let Trigger::Explicit(opts) = trigger {
            reply!(mctx, msg, "{}", &shlex::split(opts).unwrap().choose(&mut thread_rng()).unwrap())
        } else {
            panic!("choose module's triggers wrongly configured")
        }
    };
);
