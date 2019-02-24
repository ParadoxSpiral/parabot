use parabot::prelude::*;
use tokio::prelude::*;
use tokio::runtime::Runtime;

static CONFIG: &str = r##"database = "parabot_empty.db"
[[server]]
    address = "irc.rizon.net"
    port = 6697
    nick = "parabot-example"
    use_ssl = true
[[server.channel]]
    name = "#parabot-testing"
[[server.channel.module]]
    name = "logger"
    triggers = ["<ALWAYS>"]
"##;

fn main() {
    env_logger::init();

    let mut rt = Runtime::new().unwrap();
    let conns = Builder::new()
        .with_config(Config::from_str(CONFIG).unwrap())
        .with_loader(&|_, cfg| match &*cfg.name {
            "logger" => Ok(Some(Box::new(Logger))),
            _ => Ok(None),
        })
        .build()
        .unwrap();

    for conn in conns {
        rt.spawn(conn);
    }

    rt.shutdown_on_idle().wait().unwrap();
}

#[module(
    help = "Log all stages to stdout, this is not a command!",
    connected,
    received,
    pre_send,
    post_send
)]
struct Logger;

#[module(Logger, connected)]
fn connected() {
    println!("Connected!");
}

#[module(Logger, pre_send)]
fn pre_send(msg: &Message) {
    println!("Should send: {:?}", msg)
}

#[module(Logger, post_send)]
fn post_send(msg: &Message) {
    println!("Sent: {:?}", msg)
}

#[module(Logger, received)]
fn received(msg: &Message, trigger: Trigger) {
    if let Trigger::Always = trigger {
        println!("{:?}", msg);
    } else {
        panic!("Logger module's triggers wrongly configured");
    }
}
