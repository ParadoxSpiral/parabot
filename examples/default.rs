use parabot::prelude::*;
use tokio::prelude::*;
use tokio::runtime::Runtime;

use std::{error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let config_base = shellexpand::full("$XDG_CONFIG_HOME/parabot/")
        .unwrap_or_else(|_| shellexpand::tilde("~/.config/parabot/"))
        .into_owned();

    let mut rt = Runtime::new()?;
    for entry in fs::read_dir(Path::new(&config_base))? {
        let conf = entry?.path();
        if conf.is_file() {
            rt.spawn(Builder::new().with_config_file(&conf).build().unwrap());
        }
    }
    rt.shutdown_on_idle().wait().unwrap();
    Ok(())
}
