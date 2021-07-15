use crate::messages::*;
use actix::prelude::*;
use actix_broker::BrokerSubscribe;
use std::time::Duration;
use tracing::instrument;

type Pid = pid::Pid<f32>;

#[derive(Debug)]
pub struct PidActor {
    output: Recipient<PidOutput>,
    interval: Duration,
    pid: Pid,
    input: f32,
    interval_handle: Option<SpawnHandle>,
}

impl PidActor {
    pub fn new(output: Recipient<PidOutput>, interval: Duration) -> Self {
        Self {
            output,
            interval,
            pid: Pid::new(1.0f32, 0.0, 0.0, 100.0, 30.0, 30.0, 100.0, 20.0),
            input: f32::NAN,
            interval_handle: None,
        }
    }

    #[instrument(skip(self, _ctx))]
    fn tick(&mut self, _ctx: &mut Context<Self>) {
        if self.input.is_nan() {
            return;
        }

        //info!(?self.pid, ?self.input, "pid tick, ptr={:p}", self);
        let out = self.pid.next_control_output(self.input);
        let _ = self.output.do_send(PidOutput(out));
    }
}

impl Actor for PidActor {
    type Context = Context<Self>;
    #[instrument]
    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_system_async::<MqttConfig>(ctx);
        self.interval_handle = Some(ctx.run_interval(self.interval, Self::tick));
    }
}

impl Handler<InputTemp> for PidActor {
    type Result = ();

    #[instrument(skip(msg, _ctx))]
    fn handle(&mut self, msg: InputTemp, _ctx: &mut Self::Context) -> Self::Result {
        self.input = msg.0;
    }
}

impl Handler<MqttConfig> for PidActor {
    type Result = ();

    #[instrument(skip(self, msg, _ctx))]
    fn handle(&mut self, msg: MqttConfig, _ctx: &mut Self::Context) -> Self::Result {
        let msg = msg.0;
        self.pid.kp = msg.kp;
        self.pid.ki = msg.ki;
        self.pid.kd = msg.kd;
        self.pid.p_limit = msg.p_limit;
        self.pid.i_limit = msg.i_limit;
        self.pid.d_limit = msg.d_limit;
        self.pid.output_limit = msg.output_limit;
        self.pid.setpoint = msg.setpoint;
    }
}
