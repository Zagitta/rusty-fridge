mod actors;
mod controller;
mod messages;

use actix::prelude::*;
use actors::{
    config::ConfigActor,
    io::{InputActor, OutputActor},
    mqtt::MqttActor,
    pid::PidActor,
};
use std::{error::Error, fmt::Debug, time::Duration};

use clap::{crate_version, AppSettings, Clap};
use mqtt_async_client::client::{Client, KeepAlive};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, trace, warn, Level};

#[derive(Debug, PartialEq, Eq, enum_utils::FromStr)]
enum OutputMode {
    JsonOnOffState,
    JsonBooleanState,
    Integer,
    Boolean,
}

#[derive(Clap)]
#[clap(name = "rusty-fridge")]
#[clap(version = crate_version!(), author = "Simon Rasmussen <zetlon@gmail.com>")]
#[clap(
    about = "Controls an MQTT on/off switch to regulate the temperature of a fridge for beer fermenting"
)]
#[clap(setting = AppSettings::ColoredHelp)]
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
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum State {
    Off,
    On,
}

#[actix::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    /* tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_max_level(Level::TRACE)
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_default())
            .finish(),
    )?; */
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
        //.set_client_id(Some("yikes2".into()))
        //.set_tls_client_config(cc)
        .build()?;

    let mqtt = MqttActor::new(client).start();

    let output = OutputActor::new(
        args.output_topic,
        format!("{}/debug", base_name),
        70.0,
        mqtt.clone().recipient(),
    )
    .start();
    /* output_topic: "ESPRUNA/server-relay/relay/0/set".into(),
    state_output_topic: "rusty-fridge/debug".into(), */

    let pid = PidActor::new(output.recipient(), Duration::from_secs(60)).start();

    let _input =
        InputActor::new(args.input_topic, mqtt.clone().recipient(), pid.recipient()).start();

    let _config = ConfigActor::new(
        format!("{}/config", base_name),
        mqtt.clone().recipient(),
        mqtt.clone().recipient(),
    )
    .start();

    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
