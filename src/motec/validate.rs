use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::format::*;

pub fn validate_ld_file(path: &Path) -> Result<()> {
    let file = File::open(path)
        .with_context(|| format!("failed to open generated LD {}", path.display()))?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    let marker = read_u32(&mut reader)?;
    if marker != 0x40 {
        bail!("invalid LD marker: {marker:#x}");
    }
    reader.seek(SeekFrom::Start(8))?;
    let channels_meta_pointer = read_u32(&mut reader)?;
    let channels_data_pointer = read_u32(&mut reader)?;
    reader.seek(SeekFrom::Start(36))?;
    let event_pointer = read_u32(&mut reader)?;
    reader.seek(SeekFrom::Start(86))?;
    let channel_count = read_u32(&mut reader)?;

    if event_pointer != HEADER_SIZE {
        bail!("unexpected event pointer: {event_pointer}");
    }
    let expected_meta = HEADER_SIZE + EVENT_SIZE + VENUE_SIZE + VEHICLE_SIZE;
    if channels_meta_pointer != expected_meta {
        bail!(
            "channel metadata pointer mismatch: expected {expected_meta}, got {channels_meta_pointer}"
        );
    }
    let expected_data = expected_meta + channel_count * CHANNEL_META_SIZE;
    if channels_data_pointer != expected_data {
        bail!(
            "channel data pointer mismatch: expected {expected_data}, got {channels_data_pointer}"
        );
    }

    let mut expected_data_pointer = channels_data_pointer;
    for index in 0..channel_count {
        let meta_pointer = channels_meta_pointer + index * CHANNEL_META_SIZE;
        reader.seek(SeekFrom::Start(u64::from(meta_pointer)))?;
        let previous = read_u32(&mut reader)?;
        let next = read_u32(&mut reader)?;
        let data_pointer = read_u32(&mut reader)?;
        let data_length = read_u32(&mut reader)?;

        let expected_previous = if index == 0 {
            0
        } else {
            meta_pointer - CHANNEL_META_SIZE
        };
        let expected_next = if index + 1 == channel_count {
            0
        } else {
            meta_pointer + CHANNEL_META_SIZE
        };
        if previous != expected_previous || next != expected_next {
            bail!("broken channel metadata links at channel {index}");
        }
        if data_pointer != expected_data_pointer {
            bail!("unexpected data pointer at channel {index}");
        }

        reader.seek(SeekFrom::Current(2))?;
        let data_type = read_u16(&mut reader)?;
        let data_type_length = read_u16(&mut reader)?;
        let frequency = read_u16(&mut reader)?;
        if data_type != DATA_TYPE_FLOAT || data_type_length != DATA_TYPE_FLOAT32_LENGTH {
            bail!("unsupported generated datatype at channel {index}");
        }
        if frequency == 0 {
            bail!("zero frequency at channel {index}");
        }

        expected_data_pointer = expected_data_pointer
            .checked_add(
                data_length
                    .checked_mul(u32::from(data_type_length))
                    .context("channel size overflow")?,
            )
            .context("file size overflow")?;
    }

    if u64::from(expected_data_pointer) != file_size {
        bail!(
            "LD file size mismatch: pointers end at {}, file has {} bytes",
            expected_data_pointer,
            file_size
        );
    }

    Ok(())
}

fn read_u16(reader: &mut impl Read) -> Result<u16> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(reader: &mut impl Read) -> Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
