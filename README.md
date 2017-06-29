# parabot
An irc bot written in Rust, that aims to be reasonably fast and offers easy extensibility (via PRs ;)).

# Usage
Have rust installed, preferably via rustup.rs

Compile with ```$ cargo build --release```, or ```$ cargo rustc --release -- -C lto```.

Run with ```$ target/release/parabot /path/to/config```, see exaple_config.toml for an example configuration.

parabot uses an sqlite3 database for persistence, to integrate into an existing db run ```$ sqlite3 my_db.db < migrations/20170627120831_pending_tells/up.sql && sqlite3 my_db.db < migrations/20170629102134_last_weather_search/up.sql```, or ```[...]down.sql``` to remove tables.

# Contributing
All PRs welcome. Before you commit: format code with rustfmt-nightly, fix clippy warnings.
