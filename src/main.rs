mod controller;

use clap::Parser;
use mqtt_async_client::client::{Client, KeepAlive};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::Debug,
    str::{self, FromStr},
    time::Duration,
};
use tracing::{info, instrument, trace, warn, Level};

#[derive(Debug, PartialEq, Eq, enum_utils::FromStr)]
pub enum OutputMode {
    JsonOnOffState,
    JsonBooleanState,
    Integer,
    Boolean,
    OnOff,
}

#[derive(Debug, PartialEq, Eq, enum_utils::FromStr)]
pub enum InputMode {
    Raw,
    Json,
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct CLIArgs {
    #[clap(short, long)]
    server: String,
    #[clap(short, long)]
    username: String,
    #[clap(short, long)]
    password: String,
    #[clap(short, long)]
    input_topic: String,
    #[clap(short, long)]
    output_topic: String,
    #[clap(short, long, default_value = "rusty-fridge")]
    topic_prefix: String,
    #[clap(long, default_value = "0")]
    instance_name: String,
    #[clap(long, default_value = "Json")]
    input_mode: String,
    #[clap(long, default_value = "JsonOnOffState")]
    output_mode: String,

    #[clap(short, long)]
    brew_father: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum State {
    Off,
    On,
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let args = {
        let mut a = CLIArgs::parse();
        if let Ok(f) = std::fs::read_to_string(&a.password) {
            a.password = f;
        }
        a
    };

    let base_name = format!("{}/{}", args.topic_prefix, args.instance_name);

    let mut split = args.server.split(':');
    let host = split.next().unwrap().to_owned();
    let port = split.next().unwrap().parse()?;
    let mut cc = rustls::ClientConfig::new();
    cc.root_store
        .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

    let client = Client::builder()
        .set_host(host)
        .set_port(port)
        .set_username(Some(args.username))
        .set_password(Some(args.password.as_bytes().to_vec()))
        .set_packet_buffer_len(1024)
        .set_automatic_connect(true)
        .set_keep_alive(KeepAlive::from_secs(30))
        .build()?;

    Ok(())
}
