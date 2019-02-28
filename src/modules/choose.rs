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

use std::sync::Arc;

use crate::prelude::*;

// We declare this struct as the state of a module, that has the given (mandatory) help message,
// and will implement a handler for Stage::Received
#[module(help = ".choose \"one\" option\\ of 'some'", received)]
pub struct Choose;

// We declare this function as the Stage::received handler for the Choose module.
// The parameters are the same as the corresponding ones of the Module trait, but with the macro
// we can omit ones we don't need to keep our definition simpler (in this case &mut Choose,
// &Arc<IrcClient>, &mut ModuleCfg)
#[module(Choose, received)]
fn received(mctx: &Arc<MessageContext>, msg: &Message, trigger: Trigger) {
    // This module only uses the explicit trigger type, i.e. `.choose something or another`
    let opts = trigger.as_explicit();

    let split = shlex::split(opts).unwrap();
    let choice = split.choose(&mut thread_rng()).unwrap();

    // The reply macro sends either to a channel or in a query, depending how the msg was sent
    // to parabot. The message will be split if too long, and sent ASAP.
    mctx.reply(msg, choice.to_owned());
}
