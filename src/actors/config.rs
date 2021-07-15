use std::{sync::Arc, time::Duration};

use crate::{controller::ControllerSettings, messages::*};
use actix::prelude::*;
use actix_broker::BrokerIssue;
use mqtt_async_client::client::{
    Publish,
    QoS::{self},
    Subscribe, SubscribeTopic,
};
use tracing::{info, instrument, trace};

#[derive(Debug)]
pub struct ConfigActor {
    config_topic: String,
    pub_addr: Recipient<MqttPublish>,
    sub_addr: Recipient<MqttSubscribe>,
    got_message: bool,
}

impl ConfigActor {
    pub fn new(
        config_topic: String,
        pub_addr: Recipient<MqttPublish>,
        sub_addr: Recipient<MqttSubscribe>,
    ) -> Self {
        Self {
            config_topic,
            pub_addr,
            sub_addr,
            got_message: false,
        }
    }

    #[instrument]
    fn on_timeout(&mut self, _ctx: &mut Context<Self>) {
        if !self.got_message {
            info!("Didn't get config message within timeout, publishing defautls");
            let mut p = Publish::new(
                self.config_topic.clone(),
                serde_json::to_vec(&ControllerSettings::default()).unwrap(),
            );
            p.set_retain(true);
            let _ = self.pub_addr.do_send(MqttPublish(p));
        }
    }
}

impl Actor for ConfigActor {
    type Context = Context<Self>;

    #[instrument]
    fn started(&mut self, ctx: &mut Self::Context) {
        let _req = self.sub_addr.do_send(MqttSubscribe {
            sub: Subscribe::new(vec![SubscribeTopic {
                topic_path: self.config_topic.clone(),
                qos: QoS::AtLeastOnce,
            }]),
            addr: ctx.address().recipient(),
        });

        ctx.run_later(Duration::from_secs(5), Self::on_timeout);
    }
}

impl Handler<MqttMessage> for ConfigActor {
    type Result = ();

    #[instrument(skip(msg, _ctx))]
    fn handle(&mut self, msg: MqttMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.got_message = true;

        trace!("Handling config mqtt message");
        if let Ok(msg) = serde_json::from_slice::<ControllerSettings>(msg.0.payload()) {
            info!(?msg, "Received config msg");
            self.issue_system_async(MqttConfig(Arc::new(msg)));
        }
    }
}
