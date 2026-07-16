use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDateTime, Utc};

use crate::lmu::LmuMetadata;
use crate::telemetry::TelemetryChannel;

use super::format::*;

#[derive(Clone, Debug)]
pub struct LdMetadata {
    pub timestamp: DateTime<Utc>,
    pub driver: String,
    pub vehicle: String,
    pub venue: String,
    pub short_comment: String,
    pub event_name: String,
    pub event_session: String,
    pub event_comment: String,
    pub vehicle_id: String,
    pub vehicle_type: String,
}

impl LdMetadata {
    pub fn from_lmu(metadata: &LmuMetadata, lap_number: u16) -> Self {
        Self {
            timestamp: parse_recording_time(metadata.recording_time()).unwrap_or_else(Utc::now),
            driver: metadata.driver_name(),
            vehicle: metadata.car_name(),
            venue: metadata.track_name(),
            short_comment: format!("LMU lap {lap_number} converted by lmu2motec"),
            event_name: metadata.track_name(),
            event_session: metadata.session_type(),
            event_comment: format!(
                "{} - {} - LMU lap {}",
                metadata.track_layout(),
                metadata.car_name(),
                lap_number
            ),
            vehicle_id: metadata.car_name(),
            vehicle_type: metadata.car_class(),
        }
    }
}

pub struct LdWriter;

impl LdWriter {
    pub fn write(
        destination: &Path,
        metadata: &LdMetadata,
        channels: &[TelemetryChannel],
    ) -> Result<()> {
        if channels.is_empty() {
            bail!("cannot write an LD file without channels");
        }
        let channel_count = u32::try_from(channels.len()).context("too many channels")?;
        let event_pointer = HEADER_SIZE;
        let venue_pointer = event_pointer + EVENT_SIZE;
        let vehicle_pointer = venue_pointer + VENUE_SIZE;
        let channels_meta_pointer = vehicle_pointer + VEHICLE_SIZE;
        let channels_data_pointer = channels_meta_pointer
            .checked_add(
                CHANNEL_META_SIZE
                    .checked_mul(channel_count)
                    .context("channel metadata size overflow")?,
            )
            .context("channel data pointer overflow")?;

        let file = File::create(destination)
            .with_context(|| format!("failed to create {}", destination.display()))?;
        let mut writer = BufWriter::new(file);

        write_header(
            &mut writer,
            metadata,
            channel_count,
            event_pointer,
            channels_meta_pointer,
            channels_data_pointer,
        )?;
        write_event(&mut writer, metadata, venue_pointer)?;
        write_venue(&mut writer, metadata, vehicle_pointer)?;
        write_vehicle(&mut writer, metadata)?;

        let mut data_pointer = channels_data_pointer;
        for (index, channel) in channels.iter().enumerate() {
            let index = u32::try_from(index)?;
            let meta_pointer = channels_meta_pointer + index * CHANNEL_META_SIZE;
            let previous = if index == 0 {
                0
            } else {
                meta_pointer - CHANNEL_META_SIZE
            };
            let next = if index + 1 == channel_count {
                0
            } else {
                meta_pointer + CHANNEL_META_SIZE
            };
            write_channel_metadata(&mut writer, channel, index, previous, next, data_pointer)?;
            data_pointer = data_pointer
                .checked_add(
                    u32::try_from(channel.values.len())?
                        .checked_mul(4)
                        .context("channel data length overflow")?,
                )
                .context("file pointer overflow")?;
        }

        for channel in channels {
            for value in &channel.values {
                writer.write_all(&value.to_le_bytes())?;
            }
        }
        writer.flush()?;
        Ok(())
    }
}

fn write_header(
    writer: &mut (impl Write + Seek),
    metadata: &LdMetadata,
    channel_count: u32,
    event_pointer: u32,
    channels_meta_pointer: u32,
    channels_data_pointer: u32,
) -> Result<()> {
    writer.seek(SeekFrom::Start(0))?;
    write_u32(writer, 0x40)?;
    write_zeros(writer, 4)?;
    write_u32(writer, channels_meta_pointer)?;
    write_u32(writer, channels_data_pointer)?;
    write_zeros(writer, 20)?;
    write_u32(writer, event_pointer)?;
    write_zeros(writer, 24)?;
    write_u16(writer, 1)?;
    write_u16(writer, 0x4240)?;
    write_u16(writer, 0x000f)?;
    write_u32(writer, 0x1f44)?;
    write_fixed(writer, "ADL", 8)?;
    write_u16(writer, 420)?;
    write_u16(writer, 0xadb0)?;
    write_u32(writer, channel_count)?;
    write_zeros(writer, 4)?;
    write_fixed(
        writer,
        &metadata.timestamp.format("%d/%m/%Y").to_string(),
        16,
    )?;
    write_zeros(writer, 16)?;
    write_fixed(
        writer,
        &metadata.timestamp.format("%H:%M:%S").to_string(),
        16,
    )?;
    write_zeros(writer, 16)?;
    write_fixed(writer, &metadata.driver, 64)?;
    write_fixed(writer, &metadata.vehicle, 64)?;
    write_zeros(writer, 64)?;
    write_fixed(writer, &metadata.venue, 64)?;
    write_zeros(writer, 64)?;
    write_zeros(writer, 1024)?;
    write_u32(writer, 0x000c_81a4)?;
    write_zeros(writer, 66)?;
    write_fixed(writer, &metadata.short_comment, 64)?;
    write_zeros(writer, 126)?;
    Ok(())
}

fn write_event(
    writer: &mut (impl Write + Seek),
    metadata: &LdMetadata,
    venue_pointer: u32,
) -> Result<()> {
    writer.seek(SeekFrom::Start(u64::from(HEADER_SIZE)))?;
    write_fixed(writer, &metadata.event_name, 64)?;
    write_fixed(writer, &metadata.event_session, 64)?;
    write_fixed(writer, &metadata.event_comment, 1024)?;
    write_u16(
        writer,
        u16::try_from(venue_pointer).context("venue pointer exceeds LD limit")?,
    )?;
    Ok(())
}

fn write_venue(
    writer: &mut (impl Write + Seek),
    metadata: &LdMetadata,
    vehicle_pointer: u32,
) -> Result<()> {
    writer.seek(SeekFrom::Start(u64::from(HEADER_SIZE + EVENT_SIZE)))?;
    write_fixed(writer, &metadata.venue, 64)?;
    write_zeros(writer, 1034)?;
    write_u16(
        writer,
        u16::try_from(vehicle_pointer).context("vehicle pointer exceeds LD limit")?,
    )?;
    Ok(())
}

fn write_vehicle(writer: &mut (impl Write + Seek), metadata: &LdMetadata) -> Result<()> {
    writer.seek(SeekFrom::Start(u64::from(
        HEADER_SIZE + EVENT_SIZE + VENUE_SIZE,
    )))?;
    write_fixed(writer, &metadata.vehicle_id, 64)?;
    write_zeros(writer, 128)?;
    write_u32(writer, 0)?;
    write_fixed(writer, &metadata.vehicle_type, 32)?;
    write_fixed(writer, "Le Mans Ultimate", 32)?;
    Ok(())
}

fn write_channel_metadata(
    writer: &mut (impl Write + Seek),
    channel: &TelemetryChannel,
    index: u32,
    previous: u32,
    next: u32,
    data_pointer: u32,
) -> Result<()> {
    let meta_pointer =
        HEADER_SIZE + EVENT_SIZE + VENUE_SIZE + VEHICLE_SIZE + index * CHANNEL_META_SIZE;
    writer.seek(SeekFrom::Start(u64::from(meta_pointer)))?;
    write_u32(writer, previous)?;
    write_u32(writer, next)?;
    write_u32(writer, data_pointer)?;
    write_u32(writer, u32::try_from(channel.values.len())?)?;
    write_u16(writer, 0x2ee1u16.wrapping_add(u16::try_from(index)?))?;
    write_u16(writer, DATA_TYPE_FLOAT)?;
    write_u16(writer, DATA_TYPE_FLOAT32_LENGTH)?;
    write_u16(writer, channel.frequency)?;
    write_i16(writer, 0)?;
    write_i16(writer, 1)?;
    write_i16(writer, 1)?;
    // Float channels are stored as their final engineering values. A non-zero
    // decimal exponent would make MoTeC scale the values a second time.
    write_i16(writer, 0)?;
    write_fixed(writer, &channel.name, 32)?;
    write_fixed(writer, &channel.short_name, 8)?;
    write_fixed(writer, &channel.unit, 12)?;
    write_zeros(writer, 40)?;
    Ok(())
}

fn parse_recording_time(value: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H_%M_%SZ")
        .ok()
        .map(|timestamp| timestamp.and_utc())
}

fn write_fixed(writer: &mut impl Write, value: &str, length: usize) -> Result<()> {
    let bytes: Vec<u8> = value
        .chars()
        .map(|character| {
            if character.is_ascii() {
                character as u8
            } else {
                b'?'
            }
        })
        .take(length)
        .collect();
    let used = bytes.len();
    writer.write_all(&bytes)?;
    write_zeros(writer, length - used)
}

fn write_zeros(writer: &mut impl Write, length: usize) -> Result<()> {
    const ZEROS: [u8; 1024] = [0; 1024];
    let mut remaining = length;
    while remaining > 0 {
        let count = remaining.min(ZEROS.len());
        writer.write_all(&ZEROS[..count])?;
        remaining -= count;
    }
    Ok(())
}

fn write_u16(writer: &mut impl Write, value: u16) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_i16(writer: &mut impl Write, value: i16) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u32(writer: &mut impl Write, value: u32) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::motec::validate_ld_file;

    #[test]
    fn parses_lmu_recording_timestamp() {
        let timestamp = parse_recording_time("2026-04-03T20_06_03Z").unwrap();
        assert_eq!(timestamp.format("%d/%m/%Y").to_string(), "03/04/2026");
    }

    #[test]
    fn generated_file_passes_structural_validation() {
        let directory = tempdir().unwrap();
        let destination = directory.path().join("test.ld");
        let metadata = LdMetadata {
            timestamp: Utc::now(),
            driver: "Driver".to_owned(),
            vehicle: "Car".to_owned(),
            venue: "Track".to_owned(),
            short_comment: "Test".to_owned(),
            event_name: "Event".to_owned(),
            event_session: "Practice".to_owned(),
            event_comment: String::new(),
            vehicle_id: "Car".to_owned(),
            vehicle_type: "Prototype".to_owned(),
        };
        let channels = vec![
            TelemetryChannel {
                name: "Speed".to_owned(),
                short_name: "SPD".to_owned(),
                unit: "km/h".to_owned(),
                frequency: 10,
                values: vec![0.0, 10.0, 20.0],
            },
            TelemetryChannel {
                name: "RPM".to_owned(),
                short_name: "RPM".to_owned(),
                unit: "rpm".to_owned(),
                frequency: 20,
                values: vec![1000.0, 2000.0, 3000.0, 4000.0],
            },
        ];

        LdWriter::write(&destination, &metadata, &channels).unwrap();
        validate_ld_file(&destination).unwrap();
    }
}
