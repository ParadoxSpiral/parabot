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
use irc::client::ext::ClientExt;
use irc::client::IrcClient;
use irc::proto::Command;
pub use irc::proto::Message;
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

use std::sync::Arc;

pub type MessageContext = Arc<mpsc::UnboundedSender<(Message, DueBy, SendMode)>>;

pub const MAX_PRIVMSG_LEN: usize = 510 - 9;

lazy_static! {
    static ref URL_REGEX: Regex = Regex::new(
        "\
        .*?\
        (?:\
            (?:<){0,}
            (?P<url>\
                (?:(?:http)|(?:https))://\
                (?:[^\\s>]*?\\.){1,}\
                [^\\s>]*\
            )
            (?:>){0,}
        )\
        .*?"
    )
    .unwrap();
}

pub enum Stage {
    Connected,
    MessageReceived,
    PreMessageSend,
    PostMessageSend,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Trigger<'a> {
    Always,
    Key(&'a str),
    Action(&'a str, &'a str),
    Url(Vec<&'a str>),
}

pub trait IrcMessageExt {
    fn private(&self) -> bool;
    fn reply(&self, content: String) -> (Message, DueBy, SendMode);
    fn reply_priv(&self, content: String) -> (Message, DueBy, SendMode);
    fn trigger_match<'m, T>(&'m self, trigger: &[T]) -> Option<Trigger<'m>>
    where
        T: AsRef<str>;
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
    #[inline]
    fn trigger_match<'m, T>(&'m self, trigger: &[T]) -> Option<Trigger<'m>>
    where
        T: AsRef<str>,
    {
        match self.command {
            Command::PRIVMSG(_, ref content) => {
                for t in trigger.iter().map(|t| t.as_ref()) {
                    if content.len() > 7
                        && t.starts_with("/me")
                        && content[7..].starts_with(&t[3..])
                    {
                        return Some(Trigger::Action(
                            self.source_nickname().as_ref().unwrap(),
                            content[7 + t[3..].len()..content.len() - 1].trim(),
                        ));
                    } else if t == "<url>" {
                        return Some(Trigger::Url(
                            URL_REGEX
                                .captures_iter(content)
                                .map(|cap| cap.name("url").unwrap().as_str())
                                .collect(),
                        ));
                    } else if content.starts_with(&*t) {
                        return Some(Trigger::Key(content[t.len()..].trim()));
                    }
                }
                None
            }
            _ => None,
        }
    }
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

#[inline]
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
    } else if !msg.contains("\x02")
        && !msg.contains("\x03")
        && !msg.contains("\x09")
        && !msg.contains("\x13")
        && !msg.contains("\x0f")
        && !msg.contains("\x15")
        && !msg.contains("\x16")
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
