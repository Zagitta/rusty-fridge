use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    str::from_utf8,
    time::{Duration, Instant},
};

use mqtt_async_client::client::{
    Publish,
    QoS::{self},
    Subscribe, SubscribeTopic,
};

use tracing::{debug, info, instrument, trace};

use crate::{messages::*, InputMode, OutputMode};

#[derive(Debug)]
pub struct OutputActor {
    output_topic: String,
    state_output_topic: String,
    output_threshold: f32,
    output_mode: OutputMode,
    pub_addr: Recipient<MqttPublish>,
    prev_output: Option<bool>,
    prev_time: Option<Instant>,
}

impl OutputActor {
    pub fn new(
        output_topic: String,
        state_output_topic: String,
        output_threshold: f32,
        output_mode: OutputMode,
        pub_addr: Recipient<MqttPublish>,
    ) -> Self {
        Self {
            output_topic,
            state_output_topic,
            output_threshold,
            output_mode,
            pub_addr,
            prev_output: None,
            prev_time: None,
        }
    }
}

impl Actor for OutputActor {
    type Context = Context<Self>;
}

impl Handler<PidOutput> for OutputActor {
    type Result = ();

    #[instrument(skip(self, msg, _ctx))]
    fn handle(&mut self, msg: PidOutput, _ctx: &mut Self::Context) -> Self::Result {
        debug!(?msg, "Handling pid output");
        let output = self.output_threshold < msg.0.output;
        let changed = self.prev_output.map_or(true, |b| b != output);
        let now = Instant::now();
        let dt = self
            .prev_time
            .map_or(Duration::from_secs(60 * 60), |t| now - t);

        if changed && dt >= Duration::from_secs(60 * 15) {
            info!("Setting output to: {}. Dt={:?}", output, dt);
            let _ = self.pub_addr.do_send(MqttPublish(Publish::new(
                self.output_topic.clone(),
                format!("{}", if output { "1" } else { "0" })
                    .as_bytes()
                    .to_vec(),
            )));

            self.prev_output = Some(output);
            self.prev_time = Some(now);
        }
        let _ = self.pub_addr.do_send(MqttPublish(Publish::new(
            self.state_output_topic.clone(),
            serde_json::to_vec(&ControlOutputHelper(msg.0)).unwrap(),
        )));
        debug!(?output, "Pid tick");
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct InputMessage {
    temperature: f32,
}

#[derive(Debug)]
pub struct InputActor {
    input_topic: String,
    input_mode: InputMode,
    sub_addr: Recipient<MqttSubscribe>,
    pub_addr: Recipient<InputTemp>,
}

impl InputActor {
    pub fn new(
        input_topic: String,
        input_mode: InputMode,
        sub_addr: Recipient<MqttSubscribe>,
        pub_addr: Recipient<InputTemp>,
    ) -> Self {
        Self {
            input_topic,
            input_mode,
            sub_addr,
            pub_addr,
        }
    }
}

impl Actor for InputActor {
    type Context = Context<Self>;

    #[instrument]
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Started input actor");
        let _req = self.sub_addr.do_send(MqttSubscribe {
            sub: Subscribe::new(vec![SubscribeTopic {
                topic_path: self.input_topic.clone(),
                qos: QoS::AtLeastOnce,
            }]),
            addr: ctx.address().recipient(),
        });

        info!("Sent mqtt sub request");
    }
}

impl Handler<MqttMessage> for InputActor {
    type Result = ();

    #[instrument(skip(self, msg, _ctx))]
    fn handle(&mut self, msg: MqttMessage, _ctx: &mut Self::Context) -> Self::Result {
        trace!("Handling mqtt message");
        let temp = match self.input_mode {
            InputMode::Raw => from_utf8(msg.0.payload()).map_or(None, |s| {
                //skip the first char if it's a quoted string
                let s = if s.starts_with('"') { &s[1..] } else { s };
                s.parse::<f32>().ok()
            }),
            InputMode::Json => serde_json::from_slice::<InputMessage>(msg.0.payload())
                .map(|im| im.temperature)
                .ok(),
        };
        if let Some(temp) = temp {
            info!(?msg, "Received tempereature msg");
            let _ = self.pub_addr.do_send(InputTemp(temp));
        }
    }
}
