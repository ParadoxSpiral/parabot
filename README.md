# parabot
Yet another modular irc bot (framework)

# Requirements
- Rust >= 1.31
- pkg-config
- OpenSSL
- SQLite 3

# Usage
Run the default modules with `cargo run --release --example default`.
A configuration file (see example\_conf.toml) is expected at either `$XDG_CONFIG_HOME`|`~/.config``/parabot/conf.toml`.

# Modules
See the example config for available modules and options.

To see how to implement your own modules, see `src/modules/choose.rs` for a relatively small example.
