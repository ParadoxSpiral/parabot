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

#![feature(const_fn, inclusive_range, inclusive_range_syntax)]
#![allow(unknown_lints)]

extern crate chrono;
extern crate chrono_tz;
extern crate crossbeam;
extern crate forecast;
extern crate irc;
extern crate parking_lot;
extern crate regex;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate slog_async;
extern crate slog_term;
extern crate threadpool;
extern crate toml;
extern crate unicode_segmentation;

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
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

mod config;
pub mod models;
mod modules;
pub mod schema;

// Init logging
lazy_static!{
	static ref SLOG_ROOT: Logger = {
		let decorator = slog_term::TermDecorator::new().build();
	    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
	    let drain = slog_async::Async::new(drain).build().fuse();

		Logger::root(drain, o!("version" => env!("CARGO_PKG_VERSION")))
	};
}

fn main() {
    // Read and parse config file
    let cfg_path = env::args().nth(1).or_else(|| {
        warn!(
            SLOG_ROOT,
            "No config file specified, using ./example_conf.toml"
        );
        Some("./example_conf.toml".into())
    });

    let mut cfg = String::new();
    let file = File::open(cfg_path.unwrap()).or_else(|e| {
        crit!(SLOG_ROOT, "Failed to open config file: {}", e);
        Err(e)
    });
    file.unwrap()
        .read_to_string(&mut cfg)
        .or_else(|e| {
            crit!(SLOG_ROOT, "Failed to read config file: {}", e);
            Err(e)
        })
        .unwrap();

    let config = config::parse_config(&cfg);

    // Spawn two threads per channel, incase modules lag on e.g. IO
    // TODO: Needs testing if this scales/is even necessary
    let num_threads = config.servers.iter().fold(
        0,
        |acc, srv| srv.channels.iter().fold(acc, |acc, _| acc + 2),
    );
    let pool = ThreadPool::new(num_threads);
    info!(
        SLOG_ROOT,
        "Created threadpool for {} threads in {} channels",
        num_threads,
        num_threads / 2
    );

    // Init modules that require init
    modules::init(&config, &SLOG_ROOT);

    // Init state of each server
    let mut state = Vec::with_capacity(config.servers.len());
    for cfg in config.servers {
        // Avoid premature move of cfg into first tuple elem
        let srv = Arc::new(IrcServer::from(&cfg));
        let log = Arc::new(SLOG_ROOT.new(o!(
			    			"Server" => format!("{} on {}:{}",cfg.nickname, cfg.address, cfg.port),
			    			"Channels" => format!("{:?}", cfg.channels))));
        state.push((Arc::new(cfg), srv, log));
    }
    crossbeam::scope(move |scope| for &(ref cfg, ref srv, ref log) in &state {
        // TODO: Is there a way to do less cloning?
        let pool = pool.clone();
        let cfg = cfg.clone();
        let srv1 = srv.clone();
        let srv2 = srv.clone();
        let log = log.clone();
        scope.spawn(move || {
            // Handle registration etc, TODO: log errors
            srv1.identify().unwrap();
            srv1.send_mode(&cfg.nickname, &[Mode::Plus(UserMode::Invisible, None)])
                .unwrap();
            // Listen for, and handle, messages
            srv1.for_each_incoming(|msg| {
                let cfg = cfg.clone();
                let srv = srv2.clone();
                let log = log.clone();
                pool.execute(move || modules::handle(&cfg, &srv, &log, msg));
            }).unwrap();
        });
    });
}
