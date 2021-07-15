use crate::messages::*;
use actix::prelude::*;
use mqtt_async_client::client::{Client, ReadResult};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    sync::mpsc::{self, Receiver},
    task::JoinHandle,
};
use tracing::{debug, error, info, instrument, trace};

#[derive(Debug)]
pub struct MqttActor {
    handle: JoinHandle<()>,
    queue: mpsc::Sender<MqttTaskActions>,
}

impl MqttActor {
    pub fn new(client: Client) -> Self {
        let (s, r) = mpsc::channel(100);
        Self {
            handle: tokio::spawn(Self::client_task(client, r)),
            queue: s,
        }
    }

    async fn handle_mqtt_task(
        client: &mut Client,
        topic_subs: &mut HashMap<String, Vec<Recipient<MqttMessage>>>,
        task: Option<MqttTaskActions>,
    ) {
        match task {
            Some(MqttTaskActions::Subscribe(msg)) => {
                for t in msg.sub.topics() {
                    match topic_subs.get_mut(&t.topic_path) {
                        Some(addrs) => addrs.push(msg.addr.clone()),
                        None => {
                            topic_subs.insert(t.topic_path.clone(), vec![msg.addr.clone()]);
                        }
                    }
                }

                match client.subscribe(msg.sub).await {
                    Ok(res) => info!(?res, "Subscribed"),
                    Err(e) => error!(?e, "Failed to subscribe"),
                }
            }
            Some(MqttTaskActions::Publish(msg)) => {
                debug!(
                    "Publishing mqtt message: {:?}",
                    std::str::from_utf8(msg.0.payload())
                );
                let res = client.publish(&msg.0).await;
                debug!(?res, "Published");
            }
            None => {}
        }
    }
    async fn handle_subscription_task(
        topic_subs: &mut HashMap<String, Vec<Recipient<MqttMessage>>>,
        res: Result<ReadResult, mqtt_async_client::Error>,
    ) {
        if let Ok(msg) = res {
            if let Some(addr) = topic_subs.get(msg.topic()) {
                let p = MqttMessage(Arc::new(msg));
                for a in addr {
                    let res = a.do_send(p.clone());
                    trace!(?res, "Sent msg");
                }
            }
        }
    }

    #[instrument(skip(client, receiver))]
    async fn client_task(mut client: Client, mut receiver: Receiver<MqttTaskActions>) {
        let mut topic_subs = HashMap::<String, Vec<Recipient<MqttMessage>>>::new();

        let _ = client.connect().await;
        loop {
            let _ = tokio::select! {
                val = receiver.recv() => Self::handle_mqtt_task(&mut client, &mut topic_subs, val).await,
                res = client.read_subscriptions() => Self::handle_subscription_task(&mut topic_subs, res).await,
            };
        }
    }
}

impl Actor for MqttActor {
    type Context = Context<Self>;
}

impl Handler<MqttPublish> for MqttActor {
    type Result = ();

    #[instrument]
    fn handle(&mut self, msg: MqttPublish, ctx: &mut Self::Context) -> Self::Result {
        let q = self.queue.clone();
        async move {
            let _ = q.send(MqttTaskActions::Publish(msg)).await;
        }
        .into_actor(self)
        .wait(ctx);
    }
}

impl Handler<MqttSubscribe> for MqttActor {
    type Result = ();

    #[instrument]
    fn handle(&mut self, msg: MqttSubscribe, ctx: &mut Self::Context) -> Self::Result {
        let q = self.queue.clone();
        async move {
            let _ = q.send(MqttTaskActions::Subscribe(msg)).await;
        }
        .into_actor(self)
        .wait(ctx);
    }
}
