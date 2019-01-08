use parabot_core::prelude::*;
use tokio::prelude::*;
use tokio::runtime::Runtime;

use std::env;
use std::path::Path;

fn main() {
    let mut rt = Runtime::new().unwrap();
    let bot = Builder::new()
        .with_config_file(Path::new(&env::args().nth(1).unwrap_or_else(|| {
            shellexpand::full("$XDG_CONFIG_HOME/parabot/conf.toml")
                .unwrap_or_else(|_| shellexpand::tilde("~/.config/parabot/conf.toml"))
                .into_owned()
        })))
        .build()
        .unwrap();

    rt.spawn(bot);
    rt.shutdown_on_idle().wait().unwrap();
}
