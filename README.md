# parabot
Yet another modular irc bot (framework)

# Requirements
- Rust >= 1.31
- pkg-config
- OpenSSL
- SQLite 3

# Usage
Run the default modules with `cargo run --release --example default`. A configuration file (see example\_conf.toml) per server is expected at either `$XDG_CONFIG_HOME`|`~/.config``/parabot/conf.toml`. Note that you must use a different sqlite database per server, because modules may assume that they exist as such, e.g. the weather module assumes that nicknames stored in it are unique.

# Modules
A module is a struct that provides some funtions that are (potentially) called during various stages: when first connected to the server, on a received message, and before/after a message is sent. Each module has a configurable list of triggers that when matched in a received message, are handed to the module, e.g. `.command` or `/me command`.

When the bot is built, each module is loaded with a module loader, which maps the names in the config file to the struct implementing the Module trait. The default module loader only knows about already included modules, so when a custom module is required, you need a loader that constructs the module. It only needs to handle your own module, as the default loader is chained after it (if default modules are enabled).

To simplify implementing your own module, there is a derive-macro. See `src/modules/choose.rs` as a commented example, or any other module in that directory of how to implement a module.
