use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize, Clone, Copy)]
pub enum OperatingMode {
    ForceOff,
    ForceOn,
    Normal,
}

impl Default for OperatingMode {
    fn default() -> Self {
        OperatingMode::ForceOff
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ControllerSettings {
    pub kp: f32,
    pub ki: f32,
    pub kd: f32,
    pub p_limit: f32,
    pub i_limit: f32,
    pub d_limit: f32,
    pub output_limit: f32,
    pub setpoint: f32,
    pub mode: OperatingMode,
}

impl Default for ControllerSettings {
    fn default() -> Self {
        ControllerSettings {
            kp: 10.0,
            ki: 0.0,
            kd: 0.0,
            p_limit: 100.0,
            i_limit: 100.0,
            d_limit: 100.0,
            output_limit: 100.0,
            setpoint: 20.0,
            mode: OperatingMode::ForceOff,
        }
    }
}
