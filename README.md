# parabot
An irc bot written in Rust, that aims to be reasonably fast and offers easy extensibility (via PRs ;)).

# Usage
Have nightly rust installed, preferably via rustup.rs

Compile with ```$ rustup run nightly cargo build --release```, or ```$ rustup run nightly cargo rustc --release -- -C lto```.

Run with ```$ target/release/parabot /path/to/config```, see exaple_config.toml for an example configuration.

parabot uses an sqlite3 database for persistence, to integrate into an existing db run all ```up.sql``` files in the ```migrations/*``` directories, or ```down.sql``` to add/remove tables. The ```parabot_empty.db``` can be used as a fresh db with all migrations pre-applied, but do not use it directly, because it will change if new migrations are added.

# Modules
Modules are self contained bits of functionality that get triggered by mainly PRIVMSGs.

The list of modules is as follows:
* tell: tell another user something when they or the bot join a shared channel.
* url-info: reply to URLs with some metadata [generic, wolframalpha, jisho, youtube]
* weather: get weather information of a location

The list of planned modules is as follows:
* syncplay: manage [syncplay](http://syncplay.pl/) group watch sessions
* remind: let the bot remind you of something at some time
* vote: vote on something

If you have any ideas for more, feel free to open an issue.

# Contributing
All PRs welcome. Before you commit: format code with rustfmt-nightly, fix clippy warnings.
