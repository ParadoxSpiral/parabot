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

use irc::client::IrcClient;

use std::sync::Arc;

use crate::{message::MessageContext, *};

#[cfg(feature = "modules")]
mod choose;
#[cfg(feature = "modules")]
mod dice;

#[cfg(feature = "modules")]
pub use self::choose::Choose;
#[cfg(feature = "modules")]
pub use self::dice::Dice;

pub use parabot_derive::module;

#[inline]
#[cfg(feature = "modules")]
pub(crate) fn load_module(cfg: &mut ModuleCfg) -> Result<Option<Box<Module>>> {
    match &*cfg.name {
        "dice" => Ok(Some(Box::new(Dice::new()))),
        "choose" => Ok(Some(Box::new(Choose))),
        _ => Ok(None),
    }
}

/// Key: Channel name, module name. Value: Module configuration, Module pointer
pub(crate) type ModuleContext = HashMap<(String, String), (ModuleCfg, Box<Module>)>;

pub trait Module: Send {
    fn handles(&self, _stage: Stage) -> bool;
    fn help(&self) -> &'static str;

    #[inline]
    fn connected(
        &mut self,
        _client: &Arc<IrcClient>,
        _mctx: &MessageContext,
        _cfg: &mut ModuleCfg,
    ) {
        unreachable!()
    }
    #[inline]
    fn received<'m>(
        &mut self,
        _client: &Arc<IrcClient>,
        _mctx: &MessageContext,
        _cfg: &mut ModuleCfg,
        _msg: &'m Message,
        _trigger: Trigger<'m>,
    ) {
        unreachable!()
    }
    #[inline]
    fn pre_send(
        &mut self,
        _client: &Arc<IrcClient>,
        _mctx: &MessageContext,
        _cfg: &mut ModuleCfg,
        _msg: &Message,
    ) -> bool {
        unreachable!()
    }
    #[inline]
    fn post_send(
        &mut self,
        _client: &Arc<IrcClient>,
        _mctx: &MessageContext,
        _cfg: &mut ModuleCfg,
        _msg: &Message,
    ) {
        unreachable!()
    }
}
