# parabot
Yet another modular irc bot (framework)

# Requirements
- Rust >= 1.26
- pkg-config
- OpenSSL
- SQLite 3

# Usage
Clone, and compile with `cargo build --release`. The resulting binary is in `target/release/parabot`.
A configuration file (see example\_conf.toml) is expected at either `$XDG_CONFIG_HOME`|`~/.config``/parabot/conf.toml`.

# Modules
See the example config for available modules and options.

To see how to implement your own modules, see `src/modules/choose.rs` for a relatively small example.
