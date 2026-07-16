use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use duckdb::{AccessMode, Config, Connection};

use super::{ChannelDefinition, ChannelKind, Lap, LmuMetadata, LmuOverview, ValueColumn};

pub struct LmuDatabase {
    connection: Connection,
}

impl LmuDatabase {
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_file() {
            bail!("input file does not exist: {}", path.display());
        }
        let config = Config::default()
            .access_mode(AccessMode::ReadOnly)
            .context("failed to configure DuckDB read-only access")?;
        let connection = Connection::open_with_flags(path, config)
            .with_context(|| format!("failed to open DuckDB file {}", path.display()))?;
        Ok(Self { connection })
    }

    pub fn overview(&self) -> Result<LmuOverview> {
        let metadata = self.read_metadata()?;
        let sampled_channels = self.read_channel_catalog("channelsList", ChannelKind::Sampled)?;
        let event_channels = self.read_channel_catalog("eventsList", ChannelKind::Event)?;
        let (recording_start, recording_end) = self.recording_bounds()?;
        let laps = self.read_laps(recording_end)?;

        Ok(LmuOverview {
            metadata,
            recording_start,
            sampled_channels,
            event_channels,
            laps,
        })
    }

    pub fn read_sampled_values(
        &self,
        definition: &ChannelDefinition,
        column: &ValueColumn,
    ) -> Result<Vec<Option<f64>>> {
        let sql = format!(
            "SELECT TRY_CAST({} AS DOUBLE) FROM {}",
            quote_ident(&column.name),
            quote_ident(&definition.name)
        );
        self.read_optional_f64_column(&sql)
            .with_context(|| format!("failed to read channel {}.{}", definition.name, column.name))
    }

    pub fn read_event_values(
        &self,
        definition: &ChannelDefinition,
        column: &ValueColumn,
    ) -> Result<Vec<(f64, Option<f64>)>> {
        let sql = format!(
            "SELECT TRY_CAST(ts AS DOUBLE), TRY_CAST({} AS DOUBLE) FROM {} ORDER BY ts",
            quote_ident(&column.name),
            quote_ident(&definition.name)
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, f64>(0)?, row.get::<_, Option<f64>>(1)?))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn read_metadata(&self) -> Result<LmuMetadata> {
        let mut statement = self.connection.prepare(
            "SELECT CAST(key AS VARCHAR), CAST(value AS VARCHAR) FROM metadata ORDER BY key",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let values = rows
            .collect::<std::result::Result<BTreeMap<_, _>, _>>()
            .context("failed to read LMU metadata")?;
        Ok(LmuMetadata::new(values))
    }

    fn read_channel_catalog(
        &self,
        table: &str,
        kind: ChannelKind,
    ) -> Result<Vec<ChannelDefinition>> {
        let sql = match kind {
            ChannelKind::Sampled => format!(
                "SELECT CAST(channelName AS VARCHAR), CAST(frequency AS INTEGER), \
                 COALESCE(CAST(unit AS VARCHAR), '') FROM {} ORDER BY channelName",
                quote_ident(table)
            ),
            ChannelKind::Event => format!(
                "SELECT CAST(eventName AS VARCHAR), 0, \
                 COALESCE(CAST(unit AS VARCHAR), '') FROM {} ORDER BY eventName",
                quote_ident(table)
            ),
        };
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut definitions = Vec::new();
        for row in rows {
            let (name, frequency, unit) = row?;
            let frequency = match kind {
                ChannelKind::Sampled => {
                    let frequency = u16::try_from(frequency).with_context(|| {
                        format!("invalid frequency {frequency} for channel {name}")
                    })?;
                    if frequency == 0 {
                        bail!("zero frequency for sampled channel {name}");
                    }
                    frequency
                }
                ChannelKind::Event => 0,
            };
            let columns = self.table_columns(&name)?;
            let value_columns = build_value_columns(columns);
            if value_columns.is_empty() {
                continue;
            }
            definitions.push(ChannelDefinition {
                name,
                frequency,
                unit,
                kind,
                value_columns,
            });
        }
        Ok(definitions)
    }

    fn table_columns(&self, table: &str) -> Result<Vec<(String, String)>> {
        let sql = format!("PRAGMA table_info({})", quote_string(table));
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn recording_bounds(&self) -> Result<(f64, f64)> {
        self.connection
            .query_row(
                "SELECT MIN(TRY_CAST(value AS DOUBLE)), MAX(TRY_CAST(value AS DOUBLE)) \
                 FROM \"GPS Time\"",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .context("failed to determine recording bounds from GPS Time")
    }

    fn read_laps(&self, recording_end: f64) -> Result<Vec<Lap>> {
        let mut statement = self.connection.prepare(
            "SELECT TRY_CAST(ts AS DOUBLE), TRY_CAST(value AS INTEGER) FROM \"Lap\" ORDER BY ts",
        )?;
        let rows =
            statement.query_map([], |row| Ok((row.get::<_, f64>(0)?, row.get::<_, i32>(1)?)))?;
        let markers = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        let mut laps = Vec::with_capacity(markers.len());
        for (index, (start_ts, number)) in markers.iter().enumerate() {
            let next = markers.get(index + 1);
            let end_ts = next.map_or(recording_end, |row| row.0);
            let number = u16::try_from(*number)
                .with_context(|| format!("invalid lap number {number} at timestamp {start_ts}"))?;
            laps.push(Lap {
                number,
                start_ts: *start_ts,
                end_ts,
                complete: next.is_some(),
            });
        }
        Ok(laps)
    }

    fn read_optional_f64_column(&self, sql: &str) -> Result<Vec<Option<f64>>> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map([], |row| row.get::<_, Option<f64>>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn build_value_columns(columns: Vec<(String, String)>) -> Vec<ValueColumn> {
    let values: Vec<_> = columns
        .into_iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("ts"))
        .collect();
    let is_four_corner = values.len() == 4;
    values
        .into_iter()
        .enumerate()
        .map(|(index, (name, sql_type))| ValueColumn {
            name,
            sql_type,
            suffix: if is_four_corner {
                Some(["FL", "FR", "RL", "RR"][index])
            } else {
                None
            },
        })
        .collect()
}

pub fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn quote_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
