use parabot::prelude::*;
use tokio::prelude::*;
use tokio::runtime::Runtime;

use std::env;
use std::path::Path;

fn main() {
    env_logger::init();

    let mut rt = Runtime::new().unwrap();
    let conns = Builder::new()
        .with_config_file(Path::new(&env::args().nth(1).unwrap_or_else(|| {
            shellexpand::full("$XDG_CONFIG_HOME/parabot/conf.toml")
                .unwrap_or_else(|_| shellexpand::tilde("~/.config/parabot/conf.toml"))
                .into_owned()
        })))
        .build()
        .unwrap();

    for conn in conns {
        rt.spawn(conn);
    }

    rt.shutdown_on_idle().wait().unwrap();
}
