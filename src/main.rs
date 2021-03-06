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
#![recursion_limit = "128"]

extern crate chrono;
extern crate chrono_tz;
extern crate crossbeam;
extern crate ddg;
extern crate encoding;
extern crate forecast;
extern crate html5ever;
extern crate humansize;
extern crate irc;
extern crate mime;
extern crate parking_lot;
extern crate percent_encoding;
extern crate rand;
extern crate rayon;
extern crate regex;
extern crate reqwest;
extern crate serde_json;
extern crate shlex;
extern crate slog_async;
extern crate slog_term;
extern crate threadpool;
extern crate toml;
extern crate unicode_segmentation;
extern crate urlshortener;
extern crate wolfram_alpha;

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate slog;

use irc::client::prelude::*;
use threadpool::ThreadPool;
use slog::{Drain, Logger};

use std::env;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

use errors::*;

mod config;
pub mod models;
mod modules;
pub mod schema;

mod errors {
    pub type Result<T> = ::std::result::Result<T, Error>;

    #[derive(Debug)]
    pub enum Error {
        NoExtractableData,

        Toml(::toml::de::Error),
        Diesel(::diesel::result::Error),
        DieselConn(::diesel::result::ConnectionError),
        Reqwest(::reqwest::Error),
        Io(::std::io::Error),
        Irc(::irc::error::IrcError),
        ConfigError(irc::error::ConfigError),
        Utf8String(::std::string::FromUtf8Error),
        Utf8Str(::std::str::Utf8Error),
        Ddg(::ddg::query::Error),
        WolframAlpha(::wolfram_alpha::Error),
        Json(::serde_json::Error),
        UrlParse(::reqwest::UrlError),
    }

    impl From<diesel::result::Error> for Error {
        #[inline]
        fn from(e: diesel::result::Error) -> Error {
            Error::Diesel(e)
        }
    }

    impl From<irc::error::ConfigError> for Error {
        #[inline]
        fn from(e: irc::error::ConfigError) -> Error {
            Error::ConfigError(e)
        }
    }
    impl From<diesel::ConnectionError> for Error {
        #[inline]
        fn from(e: diesel::ConnectionError) -> Error {
            Error::DieselConn(e)
        }
    }

    impl From<std::io::Error> for Error {
        #[inline]
        fn from(e: std::io::Error) -> Error {
            Error::Io(e)
        }
    }

    impl From<std::string::FromUtf8Error> for Error {
        #[inline]
        fn from(e: std::string::FromUtf8Error) -> Error {
            Error::Utf8String(e)
        }
    }

    impl From<std::str::Utf8Error> for Error {
        #[inline]
        fn from(e: std::str::Utf8Error) -> Error {
            Error::Utf8Str(e)
        }
    }

    impl From<irc::error::IrcError> for Error {
        #[inline]
        fn from(e: irc::error::IrcError) -> Error {
            Error::Irc(e)
        }
    }

    impl From<toml::de::Error> for Error {
        #[inline]
        fn from(e: toml::de::Error) -> Error {
            Error::Toml(e)
        }
    }

    impl From<::reqwest::Error> for Error {
        #[inline]
        fn from(e: ::reqwest::Error) -> Error {
            Error::Reqwest(e)
        }
    }

    impl From<::reqwest::UrlError> for Error {
        #[inline]
        fn from(e: ::reqwest::UrlError) -> Error {
            Error::UrlParse(e)
        }
    }

    impl From<::ddg::query::Error> for Error {
        #[inline]
        fn from(e: ::ddg::query::Error) -> Error {
            Error::Ddg(e)
        }
    }

    impl From<::wolfram_alpha::Error> for Error {
        #[inline]
        fn from(e: ::wolfram_alpha::Error) -> Error {
            Error::WolframAlpha(e)
        }
    }
}

// Init logging
lazy_static!{
    static ref SLOG_ROOT: Logger = {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();

        Logger::root(drain, o!("version" => env!("CARGO_PKG_VERSION")))
    };
}

// Allows slog-async to print log msgs, when errored in main, before panic
fn wait_err<T, I: Into<Error>>(res: ::std::result::Result<T, I>) -> T {
    match res {
        Ok(v) => v,
        Err(e) => {
            crit!(SLOG_ROOT, "{:?}", e.into());
            ::std::thread::sleep(::std::time::Duration::from_millis(250));
            panic!("")
        }
    }
}

fn main() {
    // Read and parse config file
    let mut cfg = String::new();
    wait_err(
        wait_err(File::open(
            env::args().nth(1).expect("No config file given"),
        )).read_to_string(&mut cfg),
    );

    let config = wait_err(config::parse_config(&cfg));

    // Spawn two threads per channel, incase modules lag on e.g. IO
    // TODO: Needs testing if this scales/is even necessary
    let threads = config.servers.iter().fold(0, |acc, srv| {
        srv.channels.iter().fold(acc, |acc, _| acc + 2)
    });
    let pool = ThreadPool::new(threads);
    info!(
        SLOG_ROOT,
        "Created threadpool for {} threads in {} channels",
        threads,
        threads / 2
    );

    // Init modules
    wait_err(modules::init(&config, &SLOG_ROOT));

    // Init state of each server
    let mut state = Vec::with_capacity(config.servers.len());
    for cfg in config.servers {
        let log = Arc::new(SLOG_ROOT.new(o!(
                            "Server" => format!("{} on {}:{}", cfg.nickname, cfg.address, cfg.port),
                            "Channels" => format!("{:?}", cfg.channels))));
        state.push((Arc::new(wait_err(cfg.new_irc_client())), Arc::new(cfg), log));
    }
    crossbeam::scope(move |scope| {
        for &(ref srv, ref cfg, ref log) in &state {
            // TODO: Is there a way to do less cloning?
            let pool = pool.clone();
            let cfg = cfg.clone();
            let srv1 = srv.clone();
            let srv2 = srv.clone();
            let log = log.clone();
            scope.spawn(move |_| {
                // Handle registration etc
                wait_err(srv1.identify());
                wait_err(srv1.send_mode(&cfg.nickname, &[Mode::Plus(UserMode::Invisible, None)]));
                // Listen for, and handle, messages
                wait_err(srv1.for_each_incoming(|msg| {
                    let cfg = cfg.clone();
                    let srv = srv2.clone();
                    let log = log.clone();
                    pool.execute(move || {
                        if let Err(e) = modules::handle(&cfg, &srv, &log, &msg) {
                            crit!(&*log, "{:?}", e);
                        }
                    });
                }));
            });
        }
    });
}
