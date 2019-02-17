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

use chrono::{DateTime, Duration, Utc};
use futures::sync::mpsc;
pub use irc::proto::Message;
use irc::{
    client::{ext::ClientExt, IrcClient},
    proto::Command,
};
use linkify::{LinkFinder, LinkKind};
use unicode_segmentation::UnicodeSegmentation;

use std::sync::Arc;

use crate::config::ConfigTrigger;

pub type MessageContext = Arc<mpsc::UnboundedSender<(Message, DueBy, SendMode)>>;

const MAX_PRIVMSG_LEN: usize = 510 - 9;

#[macro_export]
macro_rules! reply {
    ($mctx:expr, $msg:ident, $($repl:expr),+) => {
        $mctx.unbounded_send($msg.reply(format!($($repl),+))).unwrap();
    }
}

#[macro_export]
macro_rules! reply_priv {
    ($mctx:expr, $msg:ident, $($repl:expr),+) => {
        $mctx.unbounded_send($msg.reply_priv(format!($($repl),+))).unwrap();
    }
}

#[macro_export]
macro_rules! reply_priv_pub {
    ($mctx:ident, $msg:ident, $($priv:expr),+; $($pub:expr),+) => {
        $mctx.unbounded_send($msg.reply(if $msg.private() {
            format!($($priv),+)
        } else {
            format!($($pub),+)
        })).unwrap();
    }
}

#[macro_export]
macro_rules! no_mention {
    ($str:expr) => {{
        let mut eval: String = $str;
        eval.insert(1, '\u{200B}');
        eval
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// The stages, or points in time, at which a module can be called
pub enum Stage {
    Connected,
    Received,
    PreSend,
    PostSend,
}

#[derive(Debug, Clone)]
pub enum Trigger<'msg> {
    /// The module is called on every message
    Always(&'msg Message),
    /// The module is called when a type of command was received, e.g. `JOIN`
    Command(&'msg Command),
    /// The module is called when a specific string was at the start of a PRIVMSG, the data is what
    /// came after
    Explicit(&'msg str),
    /// The module is called when a `/me <THING>` was at the start of a PRIVMSG, the data is the ca <THING>
    Action(&'msg str),
    /// THe module is called if there were matching URL(s) in a PRIVMSG
    Urls(Vec<&'msg str>),
}

pub trait IrcMessageExt {
    fn private(&self) -> bool;
    fn reply(&self, content: String) -> (Message, DueBy, SendMode);
    fn reply_priv(&self, content: String) -> (Message, DueBy, SendMode);
}

impl IrcMessageExt for Message {
    #[inline]
    fn private(&self) -> bool {
        self.response_target().eq(&self.source_nickname())
    }

    #[inline]
    fn reply(&self, content: String) -> (Message, DueBy, SendMode) {
        (
            Message {
                tags: None,
                prefix: None,
                command: Command::PRIVMSG(self.response_target().unwrap().to_owned(), content),
            },
            DueBy::Now,
            SendMode::Split,
        )
    }

    #[inline]
    fn reply_priv(&self, content: String) -> (Message, DueBy, SendMode) {
        (
            Message {
                tags: None,
                prefix: None,
                command: Command::PRIVMSG(self.source_nickname().unwrap().to_owned(), content),
            },
            DueBy::Now,
            SendMode::Split,
        )
    }
}

// TODO: Fix unicode indexing
pub(crate) fn cfg_trigger_match<'m>(
    msg: &'m Message,
    triggers: &[ConfigTrigger],
) -> Option<Trigger<'m>> {
    for t in triggers {
        match t {
            ConfigTrigger::Always => {
                return Some(Trigger::Always(msg));
            }
            ConfigTrigger::Action(act) => {
                if let Command::PRIVMSG(_, ref content) = msg.command {
                    if content.to_lowercase().starts_with("action") && content[7..].starts_with(act)
                    {
                        return Some(Trigger::Action(
                            content[7 + act[3..].len()..content.len() - 1].trim(),
                        ));
                    }
                }
            }
            ConfigTrigger::Explicit(exp) => {
                if let Command::PRIVMSG(_, ref content) = msg.command {
                    if content.starts_with(exp) {
                        return Some(Trigger::Explicit(content[exp.len()..].trim()));
                    }
                }
            }
            ConfigTrigger::Domains(allowed, ignored) => {
                if let Command::PRIVMSG(_, ref content) = msg.command {
                    let mut finder = LinkFinder::new();
                    finder.kinds(&[LinkKind::Url]);

                    let urls: Vec<&str> = finder
                        .kinds(&[LinkKind::Url])
                        .links(content)
                        .map(|l| l.as_str())
                        // FIXME: .contains fails to resolve for some reason…
                        .filter(|l| {
                            allowed.iter().any(|a| a == l) && !ignored.iter().any(|i| i == l)
                        })
                        .collect();

                    if !urls.is_empty() {
                        return Some(Trigger::Urls(urls));
                    }
                }
            }
            ConfigTrigger::Command(cmd) => {
                if cmd
                    == &*String::from(&msg.command)
                        .split(' ')
                        .next()
                        .unwrap()
                        .to_lowercase()
                {
                    return Some(Trigger::Command(&msg.command));
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SendMode {
    Truncated,
    Split,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DueBy {
    Now,
    At(DateTime<Utc>),
}

impl DueBy {
    #[inline]
    pub fn from_now(dur: Duration) -> DueBy {
        DueBy::At(Utc::now() + dur)
    }
}

pub enum ColorCode {
    White = 0,
    Black = 1,
    DarkBlue = 2,
    DarkGreen = 3,
    Red = 4,
    DarkRed = 5,
    DarkViolet = 6,
    Orange = 7,
    Yellow = 8,
    LightGreen = 9,
    Cyan = 10,
    LightCyan = 11,
    Blue = 12,
    Violet = 13,
    DarkGray = 14,
    LightGray = 15,
}

pub enum ControlCode {
    Bold = 0x02,
    Color = 0x03,
    Italic = 0x09,
    StrikeThrough = 0x13,
    Reset = 0x0f,
    Underline = 0x15,
    Reverse = 0x16,
}

pub(crate) fn send(ctx: &IrcClient, msg: &Message, mode: &SendMode) {
    let (msg_limit, msg_bytes, target, msg) =
        if let Command::PRIVMSG(ref target, ref msg) = msg.command {
            (MAX_PRIVMSG_LEN - target.len(), msg.len(), target, msg)
        } else {
            unimplemented!()
        };

    if msg_limit >= msg_bytes {
        ctx.send_privmsg(target, msg).unwrap();
    } else if *mode == SendMode::Truncated {
        ctx.send_privmsg(target, &msg[..msg_bytes]).unwrap();
    } else if !msg.contains('\x02')
        && !msg.contains('\x03')
        && !msg.contains('\x09')
        && !msg.contains('\x13')
        && !msg.contains('\x0f')
        && !msg.contains('\x15')
        && !msg.contains('\x16')
    {
        // No control codes that need to be kept intact
        let (mut bytes, mut start) = (0, 0);
        for g in msg.graphemes(true) {
            // >= so as to have at least 2 bytes left, instead of 1
            if bytes + g.len() >= msg_limit {
                ctx.send_privmsg(target, &msg[start..bytes]).unwrap();
                start = bytes;
            } else {
                bytes += g.len();
            }
        }
    } else {
        unimplemented!()
    }
}
