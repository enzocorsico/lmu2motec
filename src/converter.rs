use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::lmu::LmuDatabase;
use crate::motec::{LdMetadata, LdWriter, validate_ld_file, write_lap_markers};
use crate::telemetry::{EventSampling, build_lap_session};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExportMode {
    #[default]
    PerLap,
    Combined,
    SingleLap(u16),
}

#[derive(Clone, Copy, Debug)]
pub struct ConvertOptions {
    pub export_mode: ExportMode,
    pub include_partial: bool,
    pub event_frequency: u16,
    pub validate: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            export_mode: ExportMode::PerLap,
            include_partial: false,
            event_frequency: 100,
            validate: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConversionSummary {
    pub generated_count: usize,
    pub exported_lap_count: usize,
    pub cancelled: bool,
}

pub fn convert_file_with_progress(
    input: &Path,
    output: &Path,
    options: ConvertOptions,
    mut on_file_started: impl FnMut(&Path),
    mut cancellation_requested: impl FnMut() -> bool,
) -> Result<ConversionSummary> {
    if options.event_frequency == 0 {
        bail!("event frequency must be greater than zero");
    }

    let database = LmuDatabase::open(input)?;
    let overview = database.overview()?;
    let venue_length_mm = database.estimate_venue_length_mm(&overview)?;
    let event_sampling = EventSampling {
        frequency: options.event_frequency,
    };
    let requested_lap = match options.export_mode {
        ExportMode::SingleLap(number) => Some(number),
        ExportMode::PerLap | ExportMode::Combined => None,
    };
    let laps: Vec<_> = overview
        .laps
        .iter()
        .filter(|lap| requested_lap.is_none_or(|number| lap.number == number))
        .filter(|lap| lap.complete || options.include_partial)
        .cloned()
        .collect();

    if laps.is_empty() {
        if let Some(number) = requested_lap {
            bail!("lap {number} was not found, or it is partial");
        }
        bail!("no complete laps found");
    }

    let output_is_file = output
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ld"));
    if output_is_file {
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    } else {
        std::fs::create_dir_all(output)
            .with_context(|| format!("failed to create {}", output.display()))?;
    }

    let exported_lap_count = laps.len();

    if options.export_mode == ExportMode::Combined {
        if cancellation_requested() {
            return Ok(ConversionSummary {
                generated_count: 0,
                exported_lap_count: 0,
                cancelled: true,
            });
        }
        let destination = if output_is_file {
            output.to_path_buf()
        } else {
            output.join(combined_output_name(&overview.metadata.track_name()))
        };
        on_file_started(&destination);
        let first = laps.first().expect("laps were checked above");
        let last = laps.last().expect("laps were checked above");
        let combined_lap = crate::lmu::Lap {
            number: first.number,
            start_ts: first.start_ts,
            end_ts: last.end_ts,
            complete: last.complete,
        };
        let session = build_lap_session(&database, &overview, &combined_lap, event_sampling)?;
        let mut metadata = LdMetadata::from_lmu_laps(&overview.metadata, first.number, last.number);
        metadata.venue_length_mm = venue_length_mm;
        write_ld_safely(&destination, &metadata, &session.channels, options.validate)?;
        write_lap_markers(&destination, &laps)?;
        return Ok(ConversionSummary {
            generated_count: 1,
            exported_lap_count,
            cancelled: false,
        });
    }

    let mut laps = laps.into_iter().peekable();
    let mut generated_count = 0usize;
    let mut cancelled = false;
    while let Some(lap) = laps.next() {
        if cancellation_requested() {
            cancelled = true;
            break;
        }

        let destination = if output_is_file {
            output.to_path_buf()
        } else {
            output.join(output_name(&overview.metadata.track_name(), lap.number))
        };
        on_file_started(&destination);
        let session = build_lap_session(&database, &overview, &lap, event_sampling)?;
        let mut metadata = LdMetadata::from_lmu(&overview.metadata, lap.number);
        metadata.venue_length_mm = venue_length_mm;
        write_ld_safely(&destination, &metadata, &session.channels, options.validate)?;
        generated_count += 1;

        if laps.peek().is_some() && cancellation_requested() {
            cancelled = true;
            break;
        }
    }

    Ok(ConversionSummary {
        generated_count,
        exported_lap_count: generated_count,
        cancelled,
    })
}

fn write_ld_safely(
    destination: &Path,
    metadata: &LdMetadata,
    channels: &[crate::telemetry::TelemetryChannel],
    validate: bool,
) -> Result<()> {
    let temporary = temporary_path(destination);
    if temporary.exists() {
        std::fs::remove_file(&temporary)
            .with_context(|| format!("failed to remove stale {}", temporary.display()))?;
    }

    let result = (|| {
        LdWriter::write(&temporary, metadata, channels)?;
        if validate {
            validate_ld_file(&temporary)?;
        }
        if destination.exists() {
            std::fs::remove_file(destination)
                .with_context(|| format!("failed to replace {}", destination.display()))?;
        }
        std::fs::rename(&temporary, destination).with_context(|| {
            format!(
                "failed to move {} to {}",
                temporary.display(),
                destination.display()
            )
        })
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn temporary_path(destination: &Path) -> PathBuf {
    let mut name = OsString::from(destination.as_os_str());
    name.push(".tmp");
    PathBuf::from(name)
}

pub fn output_name(track_name: &str, lap_number: u16) -> String {
    format!("{}_lap_{lap_number}.ld", output_stem(track_name))
}

pub fn combined_output_name(track_name: &str) -> String {
    format!("{}_all_laps.ld", output_stem(track_name))
}

fn output_stem(track_name: &str) -> String {
    track_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn export_names_distinguish_per_lap_and_combined_files() {
        assert_eq!(output_name("Le Mans / 24h", 7), "Le_Mans___24h_lap_7.ld");
        assert_eq!(
            combined_output_name("Le Mans / 24h"),
            "Le_Mans___24h_all_laps.ld"
        );
    }

    #[test]
    fn failed_ld_write_preserves_existing_destination() {
        let directory = tempdir().unwrap();
        let destination = directory.path().join("existing.ld");
        std::fs::write(&destination, b"existing data").unwrap();
        let metadata = LdMetadata {
            timestamp: Utc::now(),
            driver: String::new(),
            vehicle: String::new(),
            venue: String::new(),
            venue_length_mm: None,
            short_comment: String::new(),
            event_name: String::new(),
            event_session: String::new(),
            event_comment: String::new(),
            vehicle_id: String::new(),
            vehicle_type: String::new(),
        };

        let result = write_ld_safely(&destination, &metadata, &[], true);

        assert!(result.is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing data");
        assert!(!temporary_path(&destination).exists());
    }
}
