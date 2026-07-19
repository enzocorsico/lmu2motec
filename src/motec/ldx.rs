use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::lmu::Lap;

pub fn write_ldx(
    ld_path: &Path,
    laps: &[Lap],
    event: &str,
    venue: &str,
    venue_length_mm: Option<u32>,
) -> Result<()> {
    let destination = ld_path.with_extension("ldx");
    let temporary = temporary_path(&destination);
    let contents = ldx_xml(laps, event, venue, venue_length_mm);

    if temporary.exists() {
        std::fs::remove_file(&temporary)
            .with_context(|| format!("failed to remove stale {}", temporary.display()))?;
    }

    let result = (|| {
        std::fs::write(&temporary, contents)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        if destination.exists() {
            std::fs::remove_file(&destination)
                .with_context(|| format!("failed to replace {}", destination.display()))?;
        }
        std::fs::rename(&temporary, &destination).with_context(|| {
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

fn ldx_xml(laps: &[Lap], event: &str, venue: &str, venue_length_mm: Option<u32>) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <LDXFile Version=\"1.6\" Locale=\"English\">\n\
         <Layers>\n\
         <Layer>\n\
         <MarkerBlock>\n\
         <MarkerGroup Name=\"Beacons\" Index=\"3\">\n",
    );

    if let Some(first) = laps.first() {
        for (index, lap) in laps.iter().skip(1).enumerate() {
            let elapsed_microseconds = ((lap.start_ts - first.start_ts) * 1_000_000.0)
                .round()
                .max(0.0) as u64;
            writeln!(
                xml,
                "<Marker Version=\"100\" ClassName=\"BCN\" Name=\"Manual.{}\" Flags=\"77\" Time=\"{elapsed_microseconds}.000000\"/>",
                index + 1
            )
            .expect("writing to a string cannot fail");
        }
    }

    write!(
        xml,
        "</MarkerGroup>\n\
         </MarkerBlock>\n\
         <RangeBlock/>\n\
         </Layer>\n\
         <Details>\n\
         <String Id=\"Event\" Value=\"{}\"/>\n\
         <String Id=\"Venue\" Value=\"{}\"/>\n",
        escape_xml_attribute(event),
        escape_xml_attribute(venue)
    )
    .expect("writing to a string cannot fail");
    if let Some(length) = venue_length_mm {
        writeln!(
            xml,
            "<Numeric Id=\"Venue Length\" Value=\"{}\" Unit=\"m\" DPS=\"1\"/>",
            metres_text(length)
        )
        .expect("writing to a string cannot fail");
    }
    write!(
        xml,
        "<String Id=\"Total Laps\" Value=\"{}\"/>\n\
         </Details>\n\
         </Layers>\n\
         </LDXFile>\n",
        laps.len()
    )
    .expect("writing to a string cannot fail");
    xml
}

fn metres_text(millimetres: u32) -> String {
    let metres = millimetres / 1_000;
    let remainder = millimetres % 1_000;
    if remainder == 0 {
        metres.to_string()
    } else {
        format!("{metres}.{remainder:03}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn escape_xml_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn temporary_path(destination: &Path) -> PathBuf {
    destination.with_extension("ldx.tmp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_session_creates_beacons_between_laps() {
        let laps = vec![
            lap(1, 10.0, 110.0),
            lap(2, 110.0, 230.5),
            lap(3, 230.5, 340.0),
        ];

        let xml = ldx_xml(
            &laps,
            "Algarve International Circuit",
            "Algarve International Circuit",
            Some(4_653_000),
        );

        assert!(xml.contains("Name=\"Manual.1\" Flags=\"77\" Time=\"100000000.000000\""));
        assert!(xml.contains("Name=\"Manual.2\" Flags=\"77\" Time=\"220500000.000000\""));
        assert!(xml.contains("Id=\"Total Laps\" Value=\"3\""));
        assert!(xml.contains("<String Id=\"Event\" Value=\"Algarve International Circuit\"/>"));
        assert!(xml.contains("<String Id=\"Venue\" Value=\"Algarve International Circuit\"/>"));
        assert!(xml.contains("<Numeric Id=\"Venue Length\" Value=\"4653\" Unit=\"m\" DPS=\"1\"/>"));
        assert_eq!(xml.matches("ClassName=\"BCN\"").count(), 2);
    }

    #[test]
    fn details_escape_xml_and_keep_sub_metre_precision() {
        let xml = ldx_xml(
            &[lap(1, 0.0, 1.0)],
            "A & B",
            "Test <Short>",
            Some(4_653_125),
        );

        assert!(xml.contains("Value=\"A &amp; B\""));
        assert!(xml.contains("Value=\"Test &lt;Short&gt;\""));
        assert!(xml.contains("Value=\"4653.125\" Unit=\"m\" DPS=\"1\""));
    }

    fn lap(number: u16, start_ts: f64, end_ts: f64) -> Lap {
        Lap {
            number,
            start_ts,
            end_ts,
            complete: true,
        }
    }
}
