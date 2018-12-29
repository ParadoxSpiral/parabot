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

use {super::*, error::*, message::MessageContext};

#[cfg(feature = "modules")]
pub use self::choose::Choose;
#[cfg(feature = "modules")]
pub use self::dice::Dice;

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
pub type ModuleContext = HashMap<(String, String), (ModuleCfg, Box<Module>)>;

macro_rules! handles {
    ($self:ident, $stage:path) => {
        if $self.handles($stage) {
            unimplemented!()
        } else {
            unreachable!(
                "Modules does not handle {:?} but was configured to do so",
                $stage
            )
        }
    };
}

pub trait Module: Send {
    fn handles(&self, Stage) -> bool;
    fn help<'t>(&self) -> String;

    #[inline]
    fn connected(&mut self, &Arc<IrcClient>, &MessageContext, &mut ModuleCfg) {
        handles!(self, Stage::Connected)
    }
    #[inline]
    fn message_received<'m>(
        &mut self,
        &Arc<IrcClient>,
        &MessageContext,
        &mut ModuleCfg,
        &'m Message,
        Trigger<'m>,
    ) {
        handles!(self, Stage::MessageReceived)
    }
    #[inline]
    fn pre_message_send(
        &mut self,
        &Arc<IrcClient>,
        &MessageContext,
        &mut ModuleCfg,
        &Message,
    ) -> bool {
        handles!(self, Stage::PreMessageSend)
    }
    #[inline]
    fn post_message_send(&mut self, &Arc<IrcClient>, &MessageContext, &mut ModuleCfg, &Message) {
        handles!(self, Stage::PostMessageSend)
    }
}

#[macro_export]
macro_rules! module {
    ( $mod:path, $( $stage:path ),+; $help:expr; $(connected => $connected:expr;)* $(received => $received:expr;)* $(pre_message => $pre_message:expr;)* $(post_message => $post_message:expr;)*) => {
        impl Module for $mod {
            #[inline]
            fn handles(&self, stage: Stage) -> bool {
                match stage {
                    $($stage)|+ => true,
                    _ => false,
                }
            }
            #[inline]
            fn help<'t>(&self) -> String {
                $help(self)
            }

            $(
            #[inline]
            fn connected(&mut self, c: &Arc<IrcClient>, msgs: &MessageContext, cfg: &mut ModuleCfg) {
                $connected(self, c, msgs, cfg)
            })*

            $(
            #[inline]
            fn message_received<'m>(&mut self, c: &Arc<IrcClient>, msgs: &MessageContext, cfg: &mut ModuleCfg, msg: &'m Message, trigger: Trigger<'m>) {
                $received(self, c, msgs, cfg, msg, trigger)
            })*

            $(
            #[inline]
            fn pre_message_send(&mut self, c: &Arc<IrcClient>, msgs: &MessageContext, cfg: &mut ModuleCfg, msg: &Message) -> bool {
                $pre_message(self, c, msgs, cfg, msg)
            })*

            $(
            #[inline]
            fn post_message_send(&mut self, &Arc<IrcClient>, msgs: &MessageContext, cfg: &mut ModuleCfg, msg: &Message) {
                $post_message(self, c, msgs, cfg, msg)
            })*
        }
    }
}

// FIXME: These need to be below here because >fucking macro ordering
#[cfg(feature = "modules")]
mod choose;
#[cfg(feature = "modules")]
mod dice;
