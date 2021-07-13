use std::{
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        HashMap,
    },
    error::Error,
    fmt::Debug,
    future::Future,
    sync::Arc,
    time::Duration,
};

use clap::{crate_version, AppSettings, Clap};
use mqtt_async_client::client::{Client, Publish, QoS, ReadResult, Subscribe, SubscribeTopic};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, trace, warn, Level};
use tracing_subscriber::EnvFilter;

use pid::Pid;
use tokio::{
    sync::{broadcast, mpsc, oneshot, RwLock},
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

#[derive(Debug)]
struct SubHandler {
    client: Client,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    topic_map: HashMap<String, broadcast::Sender<Arc<ReadResult>>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InputMessage {
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum State {
    Off,
    On,
}

#[derive(Debug, Serialize, Deserialize)]
struct OutputMessage {
    state: bool,
}

impl SubHandler {
    pub fn new(client: Client) -> SubHandler {
        SubHandler {
            client,
            tasks: Default::default(),
            topic_map: Default::default(),
        }
    }

    #[instrument]
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let res = self.client.read_subscriptions().await;
            info!(?res);
            let res = res?;
            if let Some(sender) = self.topic_map.get(res.topic()) {
                sender.send(Arc::new(res))?;
            }
        }
    }

    #[instrument]
    pub async fn wait(&mut self) {
        for t in &mut self.tasks {
            let _ = tokio::join!(t);
        }
    }

    #[instrument(skip(callback))]
    pub async fn subscribe<FN, FU>(
        &mut self,
        topic: String,
        callback: FN,
    ) -> Result<(), Box<dyn Error>>
    where
        FN: FnOnce(broadcast::Receiver<Arc<ReadResult>>) -> FU,
        FU: Future<Output = ()> + Send + 'static,
    {
        let receiver = match self.topic_map.entry(topic.clone()) {
            Occupied(ref ent) => ent.get().subscribe(),
            Vacant(ent) => {
                let (sender, recevier) = broadcast::channel(10);
                ent.insert(sender);
                trace!(?topic, "Subscribing...");
                let res = self
                    .client
                    .subscribe(Subscribe::new(vec![SubscribeTopic {
                        topic_path: topic.clone(),
                        qos: QoS::AtMostOnce,
                    }]))
                    .await?;
                res.any_failures()?;
                info!(?topic, ?res, "Done subscribing");
                recevier
            }
        };

        self.tasks.push(tokio::spawn(callback(receiver)));

        Ok(())
    }
}

#[tokio::main]
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

    let mut split = args.server.split(':');
    let host = split.next().unwrap().to_owned();
    let port = split.next().unwrap().parse()?;
    let mut cc = rustls::ClientConfig::new();
    cc.root_store
        .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

    let mut client = Client::builder()
        .set_host(host)
        .set_port(port)
        .set_username(Some(args.username))
        .set_password(Some(args.password.as_bytes().to_vec()))
        .set_packet_buffer_len(1024)
        .set_automatic_connect(true)
        //.set_client_id(Some("yikes2".into()))
        //.set_tls_client_config(cc)
        .build()?;

    client.connect().await?;
    info!("connected!");

    let sh = RwLock::new(SubHandler::new(client));

    {
        let topic = args.input_topic.clone();
        sh.write()
            .await
            .subscribe(topic, |mut queue| async move {
                trace!("Starting temperature topic callback");
                while let Ok(msg) = queue.recv().await {
                    let res = serde_json::from_slice::<InputMessage>(msg.payload());
                    info!(?res, "Got temp");
                }
            })
            .await?;
    }
    {
        let (s, r) = oneshot::channel();
        let topic = format!("{}/foobar", args.topic_prefix);
        {
            sh.write()
                .await
                .subscribe(topic.clone(), |mut queue| async move {
                    trace!("Starting controller topic callback");
                    let first = queue.recv().await;
                    trace!(?first, "Got first controller topic");
                    if let Ok(msg) = first {
                        let _ = s.send(msg);
                    }
                    while let Ok(msg) = queue.recv().await {
                        info!(?msg, "Got message");
                    }
                    trace!("Finished controller topic callback");
                })
                .await?;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;

        let pid = match time::timeout(Duration::from_secs(5), r).await {
            Ok(Ok(res)) => serde_json::from_slice::<Pid<f32>>(res.payload()),
            _ => {
                let pid = Pid::new(1.0f32, 0.0, 0.0, 100.0, 30.0, 30.0, 100.0, 20.0);
                info!(
                    ?topic,
                    ?pid,
                    "Topic is empty, populating with default value"
                );
                let mut po = Publish::new(topic, serde_json::to_vec(&pid)?);
                po.set_retain(true);
                po.set_qos(QoS::AtLeastOnce);
                sh.write().await.client.publish(&po).await?;
                Ok(pid)
            }
        }?;
    }

    sh.write().await.run().await?;
    //sh.write().await.wait().await;

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
