use parabot::prelude::*;

static CONFIG: &str = r##"database = "parabot_empty.db"
address = "irc.rizon.net"
port = 6697
nick = "parabot-example"
use_ssl = true

[[channel]]
name = "#parabot-testing"
    [[channel.module]]
    name = "logger"
    triggers = ["<ALWAYS>"]
"##;

fn main() {
    env_logger::init();

    let conn = Builder::new()
        .with_config(Config::from_str(CONFIG).unwrap())
        .with_loader(&|_, cfg| match &*cfg.name {
            "logger" => Ok(Some(Box::new(Logger))),
            _ => Ok(None),
        })
        .build()
        .unwrap();

    tokio::run(conn);
}

#[module(
    help = "Log all stages to stdout, this is not a command!",
    handles = "connected",
    handles = "received",
    handles = "pre_send",
    handles = "post_send"
)]
struct Logger;

#[module(belongs_to = "Logger", handles = "connected")]
fn connected() {
    println!("Connected!");
}

#[module(belongs_to = "Logger", handles = "pre_send")]
fn pre_send(msg: &Message) {
    println!("Should send: {:?}", msg)
}

#[module(belongs_to = "Logger", handles = "post_send")]
fn post_send(msg: &Message) {
    println!("Sent: {:?}", msg)
}

#[module(belongs_to = "Logger", handles = "received")]
fn received(msg: &Message, trigger: Trigger) {
    if let Trigger::Always = trigger {
        println!("{:?}", msg);
    } else {
        panic!("Logger module's triggers wrongly configured");
    }
}
