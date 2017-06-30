# parabot
An irc bot written in Rust, that aims to be reasonably fast and offers easy extensibility (via PRs ;)).

# Usage
Have rust installed, preferably via rustup.rs

Compile with ```$ cargo build --release```, or ```$ cargo rustc --release -- -C lto```.

Run with ```$ target/release/parabot /path/to/config```, see exaple_config.toml for an example configuration.

parabot uses an sqlite3 database for persistence, to integrate into an existing db run all ```up.sql``` files in the ```migrations/*``` directories, or ```down.sql``` to add/remove tables.

# Modules
Modules are self contained bits of functionality that get triggered by mainly PRIVMSGs.

The list of modules is as follows:
* tell: tell another user something when they or the bot join a shared channel.
* weather: get weather information of a location

The list of planned modules is as follows:
* wolfram-alpha: send a query to [WolframAlpha](https://www.wolframalpha.com/)
* url: reply to URLs with some metadata
* syncplay: manage [syncplay](http://syncplay.pl/) group watch sessions
* remind: let the bot remind you of something at some time

If you have any ideas for more, feel free to open an issue.

# Contributing
All PRs welcome. Before you commit: format code with rustfmt-nightly, fix clippy warnings.
