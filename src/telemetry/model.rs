#[derive(Clone, Debug)]
pub struct TelemetryChannel {
    pub name: String,
    pub short_name: String,
    pub unit: String,
    pub frequency: u16,
    pub values: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct LapSession {
    pub channels: Vec<TelemetryChannel>,
}

#[derive(Clone, Copy, Debug)]
pub struct EventSampling {
    pub frequency: u16,
}
