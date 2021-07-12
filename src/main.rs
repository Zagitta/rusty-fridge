use std::{collections::HashMap, error::Error, future::Future, sync::Arc, time::Duration};

use clap::{crate_version, AppSettings, Clap};
use mqtt_async_client::client::{Client, QoS, ReadResult, Subscribe, SubscribeTopic};
use serde::{Deserialize, Serialize};
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;

use pid::Pid;
use tokio::{
    sync::{broadcast, mpsc, RwLock},
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

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
struct ControllerState {
    pid: Pid<f32>,
    input: f32,
    output: f32,
}

impl ControllerState {
    pub fn new() -> ControllerState {
        ControllerState {
            pid: Pid::new(1.0, 0.0, 0.0, 100.0, 30.0, 30.0, 100.0, 20.0),
            input: 0.0,
            output: 0.0,
        }
    }
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
    #[clap(short, long, default_value = "rusty-fridge")]
    topic_prefix: String,
    #[clap(short, long)]
    output_topic: String,
    #[clap(short, long)]
    input_topic: String,
}

async fn subscribe<F: Fn(ReadResult)>(client: &mut Client, topic: String, callback: F) {}

struct SubHandler {
    client: Client,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    topic_map: HashMap<
        String,
        Vec<Box<dyn Fn(broadcast::Receiver<ReadResult>) -> dyn Future<Output = ()>>>,
    >,
}

impl SubHandler {
    async fn subscribe(
        &mut self,
        topic: String,
        callback: impl Fn(broadcast::Receiver<ReadResult>) -> dyn Future<Output = ()>,
    ) -> Result<(), Box<dyn Error>> {
        let (sender, receiver) = broadcast::channel(10);

        tokio::spawn(callback(receiver));

        self.tasks.push(tokio::spawn(async move {}));
        //let vec = self.topic_map.entry(topic.clone()).or_default();
        //vec.push(Box::new(tokio::spawn(async {})));
        self.client
            .subscribe(Subscribe::new(vec![SubscribeTopic {
                topic_path: topic,
                qos: QoS::AtLeastOnce,
            }]))
            .await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_max_level(Level::TRACE)
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_default())
            .finish(),
    )?;

    let args = {
        let mut a = CLIArgs::parse();
        if let Ok(f) = std::fs::read_to_string(&a.password) {
            a.password = f;
        }
        a
    };

    let mut split = args.server.split(':');
    let host = split.next().unwrap().to_owned();
    let port = split.next().unwrap().parse()?;

    let mut client = Client::builder()
        .set_host(host)
        .set_port(port)
        .set_username(Some(args.username))
        .set_password(Some(args.password.as_bytes().to_vec()))
        .build()?;

    client.connect().await?;
    info!("connected!");

    let client = RwLock::new(client);

    let topic = args.input_topic.clone();
    let t1 = tokio::spawn(async {
        info!(?topic, "subscribing to topic");
        let client = client;
        let mut lock = client.write().await;
        let subs = lock
            .subscribe(Subscribe::new(vec![SubscribeTopic {
                topic_path: topic,
                qos: QoS::AtLeastOnce,
            }]))
            .await;

        info!(?subs, "Subscribed!");
        loop {
            let foobar = lock.read_subscriptions().await.unwrap();
            info!(?foobar, "got msg");
        }
    });

    let res = tokio::join!(t1);

    Ok(())
}

// async fn setting_task(
//     state: RwLock<ControllerState>,
//     sub_queue: mpsc::Sender<SubscribePacket>,
//     mut msg_queue: broadcast::Receiver<Arc<VariablePacket>>,
// ) {
//     let cf = TopicFilter::new(format!("{}/control", "asd")).unwrap();
//     sub_queue
//         .send(SubscribePacket::new(
//             0,
//             vec![(cf, QualityOfService::Level2)],
//         ))
//         .await
//         .expect("Should always be able to subscribe");
//     loop {
//         if let Ok(arc) = msg_queue.recv().await {
//             match *arc {
//                 VariablePacket::PublishPacket(ref pkg) => {
//                     /* if cf.matches(pkg.topic_name()) {
//                         let payload = pkg.payload();
//                         //json?
//                     } */
//                 }
//                 _ => {}
//             }
//         }
//     }
// }

async fn pid_task() {
    let mut state = ControllerState::new();
    let mut int = time::interval(Duration::from_secs(60 * 5));

    loop {
        int.tick().await;
        let res = state.pid.next_control_output(state.input);

        //tokio::sync::watch::
    }
}
