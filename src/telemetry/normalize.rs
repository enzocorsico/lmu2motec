use anyhow::{Context, Result};

use crate::lmu::{ChannelDefinition, ChannelKind, Lap, LmuDatabase, LmuOverview, ValueColumn};

use super::EventSampling;
use super::model::{LapSession, TelemetryChannel};

pub fn build_lap_session(
    database: &LmuDatabase,
    overview: &LmuOverview,
    lap: &Lap,
    event_sampling: EventSampling,
) -> Result<LapSession> {
    let mut channels = Vec::new();

    for definition in overview
        .sampled_channels
        .iter()
        .chain(overview.event_channels.iter())
    {
        for column in &definition.value_columns {
            let channel = match definition.kind {
                ChannelKind::Sampled => {
                    build_sampled_channel(database, overview, lap, definition, column)?
                }
                ChannelKind::Event => build_event_channel(
                    database,
                    lap,
                    definition,
                    column,
                    event_sampling.frequency,
                )?,
            };
            if !channel.values.is_empty() {
                channels.push(channel);
            }
        }
    }

    Ok(LapSession { channels })
}

fn build_sampled_channel(
    database: &LmuDatabase,
    overview: &LmuOverview,
    lap: &Lap,
    definition: &ChannelDefinition,
    column: &ValueColumn,
) -> Result<TelemetryChannel> {
    let source = database.read_sampled_values(definition, column)?;
    let step_signal = is_discrete_type(&column.sql_type);
    let sample_count = sample_count(lap, definition.frequency);
    let mut values = Vec::with_capacity(sample_count);

    for index in 0..sample_count {
        let timestamp = lap.start_ts + index as f64 / f64::from(definition.frequency);
        let source_position =
            (timestamp - overview.recording_start) * f64::from(definition.frequency);
        values.push(interpolate_sample(&source, source_position, step_signal) as f32);
    }

    Ok(make_channel(
        definition,
        column,
        definition.frequency,
        values,
    ))
}

fn build_event_channel(
    database: &LmuDatabase,
    lap: &Lap,
    definition: &ChannelDefinition,
    column: &ValueColumn,
    frequency: u16,
) -> Result<TelemetryChannel> {
    let events = database
        .read_event_values(definition, column)
        .with_context(|| format!("failed to load event {}", definition.name))?;
    if events.is_empty() {
        return Ok(make_channel(definition, column, frequency, Vec::new()));
    }

    let mut cursor = 0usize;
    let mut current = events
        .iter()
        .take_while(|(timestamp, _)| *timestamp <= lap.start_ts)
        .last()
        .and_then(|(_, value)| *value)
        .unwrap_or(0.0);
    while cursor < events.len() && events[cursor].0 <= lap.start_ts {
        cursor += 1;
    }

    let count = sample_count(lap, frequency);
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let timestamp = lap.start_ts + index as f64 / f64::from(frequency);
        while cursor < events.len() && events[cursor].0 <= timestamp {
            if let Some(value) = events[cursor].1 {
                current = value;
            }
            cursor += 1;
        }
        values.push(current as f32);
    }

    Ok(make_channel(definition, column, frequency, values))
}

fn sample_count(lap: &Lap, frequency: u16) -> usize {
    ((lap.end_ts - lap.start_ts) * f64::from(frequency))
        .ceil()
        .max(0.0) as usize
}

fn interpolate_sample(source: &[Option<f64>], position: f64, step: bool) -> f64 {
    if source.is_empty() {
        return 0.0;
    }
    let left = position.floor().max(0.0) as usize;
    if step || left + 1 >= source.len() {
        return nearest_present(source, left);
    }
    let right = left + 1;
    let left_value = source[left].unwrap_or_else(|| nearest_present(source, left));
    let right_value = source[right].unwrap_or(left_value);
    left_value + (right_value - left_value) * position.fract()
}

fn nearest_present(source: &[Option<f64>], index: usize) -> f64 {
    source
        .get(index)
        .and_then(|value| *value)
        .or_else(|| {
            source[..index.min(source.len())]
                .iter()
                .rev()
                .find_map(|value| *value)
        })
        .unwrap_or(0.0)
}

fn is_discrete_type(sql_type: &str) -> bool {
    let upper = sql_type.to_ascii_uppercase();
    upper.contains("BOOL") || upper.contains("INT")
}

fn make_channel(
    definition: &ChannelDefinition,
    column: &ValueColumn,
    frequency: u16,
    values: Vec<f32>,
) -> TelemetryChannel {
    let name = output_channel_name(definition, column);
    TelemetryChannel {
        short_name: short_name(&name),
        name,
        unit: normalize_unit(&definition.unit),
        frequency,
        values,
    }
}

fn output_channel_name(definition: &ChannelDefinition, column: &ValueColumn) -> String {
    column.suffix.map_or_else(
        || definition.name.clone(),
        |suffix| format!("{} {}", definition.name, suffix),
    )
}

fn short_name(name: &str) -> String {
    let compact: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(8)
        .collect();
    if compact.len() >= 2 {
        compact.to_ascii_uppercase()
    } else {
        name.chars().take(8).collect()
    }
}

fn normalize_unit(unit: &str) -> String {
    match unit.trim() {
        "none" | "On/Off" => String::new(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_interpolation_works() {
        let values = vec![Some(0.0), Some(10.0)];
        assert_eq!(interpolate_sample(&values, 0.25, false), 2.5);
    }

    #[test]
    fn step_interpolation_works() {
        let values = vec![Some(2.0), Some(3.0)];
        assert_eq!(interpolate_sample(&values, 0.75, true), 2.0);
    }
}
