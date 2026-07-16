use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct LmuMetadata {
    values: BTreeMap<String, String>,
}

impl LmuMetadata {
    pub fn new(values: BTreeMap<String, String>) -> Self {
        Self { values }
    }

    pub fn get(&self, key: &str) -> &str {
        self.values.get(key).map(String::as_str).unwrap_or("")
    }

    pub fn track_name(&self) -> String {
        fallback(self.get("TrackName"), "Unknown Track")
    }

    pub fn track_layout(&self) -> String {
        fallback(self.get("TrackLayout"), "Unknown Layout")
    }

    pub fn car_name(&self) -> String {
        fallback(self.get("CarName"), "Unknown Car")
    }

    pub fn car_class(&self) -> String {
        fallback(self.get("CarClass"), "Unknown")
    }

    pub fn driver_name(&self) -> String {
        fallback(self.get("DriverName"), "Unknown Driver")
    }

    pub fn session_type(&self) -> String {
        fallback(self.get("SessionType"), "Session")
    }

    pub fn recording_time(&self) -> &str {
        self.get("RecordingTime")
    }
}

fn fallback(value: &str, default: &str) -> String {
    if value.trim().is_empty() {
        default.to_owned()
    } else {
        value.to_owned()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelKind {
    Sampled,
    Event,
}

#[derive(Clone, Debug)]
pub struct ValueColumn {
    pub name: String,
    pub sql_type: String,
    pub suffix: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct ChannelDefinition {
    pub name: String,
    pub frequency: u16,
    pub unit: String,
    pub kind: ChannelKind,
    pub value_columns: Vec<ValueColumn>,
}

#[derive(Clone, Debug)]
pub struct Lap {
    pub number: u16,
    pub start_ts: f64,
    pub end_ts: f64,
    pub complete: bool,
}

#[derive(Clone, Debug)]
pub struct LmuOverview {
    pub metadata: LmuMetadata,
    pub recording_start: f64,
    pub sampled_channels: Vec<ChannelDefinition>,
    pub event_channels: Vec<ChannelDefinition>,
    pub laps: Vec<Lap>,
}
