use actix::prelude::*;
use mqtt_async_client::client::{Publish, ReadResult, Subscribe};
use serde::{self, Deserialize, Serialize};
use std::sync::Arc;

use crate::controller::ControllerSettings;

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct PidTick;

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct InputTemp(pub f32);

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct PidOutput(pub pid::ControlOutput<f32>);

#[derive(Serialize, Deserialize)]
#[serde(remote = "pid::ControlOutput::<f32>")]
pub struct ControlOutputDef {
    pub p: f32,
    pub i: f32,
    pub d: f32,
    pub output: f32,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ControlOutputHelper(#[serde(with = "ControlOutputDef")] pub pid::ControlOutput<f32>);

#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct MqttMessage(pub Arc<ReadResult>);

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct MqttPublish(pub Publish);

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct MqttSubscribe {
    pub sub: Subscribe,
    pub addr: Recipient<MqttMessage>,
}

#[derive(Debug)]
pub enum MqttTaskActions {
    Subscribe(MqttSubscribe),
    Publish(MqttPublish),
}

#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct MqttConfig(pub Arc<ControllerSettings>);
