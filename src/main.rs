use std::{error::Error, sync::Arc, time::Duration};

use clap::{crate_version, App, Arg};
use mqtt::{
    packet::{DecodablePacket, EncodablePacket, SubscribePacket, VariablePacket},
    QualityOfService, TopicFilter,
};
use serde::{Deserialize, Serialize};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use pid::{ControlOutput, Pid};
use tokio::{
    sync::{self, broadcast, mpsc, RwLock},
    time,
};

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize, Clone, Copy)]
enum OperatingMode {
    ForceOff,
    ForceOn,
    Normal,
}

impl Default for OperatingMode {
    fn default() -> Self {
        OperatingMode::ForceOff
    }
}

#[derive(Debug, PartialEq, PartialOrd, Default, Serialize, Deserialize, Clone, Copy)]
struct PIDSettings {
    setpoint: f32,
    kp: f32,
    ki: f32,
    kd: f32,
    mode: OperatingMode,
}

impl Into<Pid<f32>> for PIDSettings {
    fn into(self) -> Pid<f32> {
        Pid::new(
            self.kp,
            self.ki,
            self.kd,
            f32::MAX,
            f32::MAX,
            f32::MAX,
            f32::MAX,
            self.setpoint,
        )
    }
}
#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
struct ControllerState {
    settings: PIDSettings,
    pid: Pid<f32>,
    input: f32,
    output: f32,
}

impl ControllerState {
    fn new() -> ControllerState {
        let settings = PIDSettings::default();
        ControllerState {
            pid: settings.into(),
            settings,
            input: 0.0,
            output: 0.0,
        }
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_max_level(Level::INFO)
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_default())
            .finish(),
    )?;

    let matches = App::new("rusty-fridge")
        .version(crate_version!())
        .author("Simon Rasmussen <zetlon@gmail.com>")
        .about("Controls an MQTT on/off switch to regulate the temperature of a fridge for beer fermenting")
        .arg(Arg::with_name("server").env("RF_SERVER").required(true).takes_value(true).help("MQTT server address (host:port)"))
        .arg(Arg::with_name("username").env("RF_USERNAME").takes_value(true).required(true))
        .arg(Arg::with_name("password").env("RF_PASSWORD").takes_value(true).required(true).help("Password provided as raw password or a path from which to read the pw from"))
        .arg(Arg::with_name("client_id").env("RF_CLIENT_ID").takes_value(true).default_value("rusty-fridge"))
        .arg(Arg::with_name("prefix").takes_value(true).default_value("rusty-fridge"))
        .get_matches();

    let pw = {
        let pw = matches.value_of("password").unwrap();
        if let Ok(f) = std::fs::read_to_string(pw) {
            f
        } else {
            pw.to_owned()
        }
    };

    let prefix = "foobar";
    SubscribePacket::new(
        0,
        vec![(
            TopicFilter::new(format!("{}/control", prefix)).unwrap(),
            QualityOfService::Level2,
        )],
    );

    Ok(())
}

async fn setting_task(
    state: RwLock<ControllerState>,
    sub_queue: mpsc::Sender<SubscribePacket>,
    mut msg_queue: broadcast::Receiver<Arc<VariablePacket>>,
) {
    let cf = TopicFilter::new(format!("{}/control", "asd")).unwrap();
    sub_queue
        .send(SubscribePacket::new(
            0,
            vec![(cf, QualityOfService::Level2)],
        ))
        .await
        .expect("Should always be able to subscribe");
    loop {
        if let Ok(arc) = msg_queue.recv().await {
            match *arc {
                VariablePacket::PublishPacket(ref pkg) => {
                    /* if cf.matches(pkg.topic_name()) {
                        let payload = pkg.payload();
                        //json?
                    } */
                }
                _ => {}
            }
        }
    }
}

async fn pid_task() {
    let mut state = ControllerState::new();
    let mut int = time::interval(Duration::from_secs(60 * 5));

    loop {
        int.tick().await;
        let res = state.pid.next_control_output(state.input);

        //tokio::sync::watch::
    }
}
