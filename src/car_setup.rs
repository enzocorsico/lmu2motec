use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::lmu::LmuMetadata;

// Ported from lmu_duckdb_carsetup_exporter.py by S.Victor, itself based on
// TinyPedal's GPL-3.0 garage exporter. CompoundSetting is intentionally omitted
// because LMU can expose a desynchronised value for it.
const SETTINGS: &[(&str, &str, &str)] = &[
    ("GENERAL", "Symmetric", "symmetric"),
    ("GENERAL", "CGHeightSetting", "VM_WEIGHT_VERTICAL"),
    ("GENERAL", "CGRightSetting", "VM_WEIGHT_LATERAL"),
    ("GENERAL", "CGRearSetting", "VM_WEIGHT_DISTRIB"),
    ("GENERAL", "WedgeSetting", "VM_WEIGHT_WEDGE"),
    (
        "GENERAL",
        "FrontTireCompoundSetting",
        "VM_FRONT_TIRE_COMPOUND",
    ),
    (
        "GENERAL",
        "RearTireCompoundSetting",
        "VM_REAR_TIRE_COMPOUND",
    ),
    ("GENERAL", "FuelSetting", "VM_FUEL_LEVEL"),
    ("GENERAL", "FuelCapacitySetting", "VM_FUEL_CAPACITY"),
    ("GENERAL", "VirtualEnergySetting", "VM_VIRTUAL_ENERGY"),
    ("GENERAL", "NumPitstopsSetting", "VM_NUM_PITSTOPS"),
    ("GENERAL", "Pitstop1Setting", "VM_PITSTOP_1"),
    ("GENERAL", "Pitstop2Setting", "VM_PITSTOP_2"),
    ("GENERAL", "Pitstop3Setting", "VM_PITSTOP_3"),
    ("LEFTFENDER", "FenderFlareSetting", "VM_LEFT_FENDER_FLARE"),
    ("RIGHTFENDER", "FenderFlareSetting", "VM_RIGHT_FENDER_FLARE"),
    ("FRONTWING", "FWSetting", "VM_FRONT_WING"),
    ("REARWING", "RWSetting", "VM_REAR_WING"),
    ("BODYAERO", "WaterRadiatorSetting", "VM_WATER_RADIATOR"),
    ("BODYAERO", "OilRadiatorSetting", "VM_OIL_RADIATOR"),
    ("BODYAERO", "BrakeDuctSetting", "VM_BRAKE_DUCTS"),
    ("BODYAERO", "BrakeDuctRearSetting", "VM_BRAKE_DUCTS_REAR"),
    (
        "SUSPENSION",
        "FrontWheelTrackSetting",
        "VM_FRONT_WHEEL_TRACK",
    ),
    ("SUSPENSION", "RearWheelTrackSetting", "VM_REAR_WHEEL_TRACK"),
    ("SUSPENSION", "FrontAntiSwaySetting", "VM_FRONT_ANTISWAY"),
    ("SUSPENSION", "RearAntiSwaySetting", "VM_REAR_ANTISWAY"),
    ("SUSPENSION", "FrontToeInSetting", "VM_FRONT_TOEIN"),
    ("SUSPENSION", "FrontToeOffsetSetting", "VM_FRONT_TOEOFFSET"),
    ("SUSPENSION", "RearToeInSetting", "VM_REAR_TOEIN"),
    ("SUSPENSION", "RearToeOffsetSetting", "VM_REAR_TOEOFFSET"),
    ("SUSPENSION", "LeftCasterSetting", "VM_LEFT_CASTER"),
    ("SUSPENSION", "RightCasterSetting", "VM_RIGHT_CASTER"),
    ("SUSPENSION", "LeftTrackBarSetting", "VM_LEFT_TRACK_BAR"),
    ("SUSPENSION", "RightTrackBarSetting", "VM_RIGHT_TRACK_BAR"),
    (
        "SUSPENSION",
        "Front3rdPackerSetting",
        "VM_FRONT_3RD_PACKERS",
    ),
    ("SUSPENSION", "Front3rdSpringSetting", "VM_FRONT_3RD_SPRING"),
    (
        "SUSPENSION",
        "Front3rdTenderSpringSetting",
        "VM_FRONT_3RD_TENDERSPRING",
    ),
    (
        "SUSPENSION",
        "Front3rdTenderTravelSetting",
        "VM_FRONT_3RD_TENDERSPRINGTRAVEL",
    ),
    (
        "SUSPENSION",
        "Front3rdSlowBumpSetting",
        "VM_FRONT_3RD_SLOWBUMP",
    ),
    (
        "SUSPENSION",
        "Front3rdFastBumpSetting",
        "VM_FRONT_3RD_FASTBUMP",
    ),
    (
        "SUSPENSION",
        "Front3rdSlowReboundSetting",
        "VM_FRONT_3RD_SLOWREBOUND",
    ),
    (
        "SUSPENSION",
        "Front3rdFastReboundSetting",
        "VM_FRONT_3RD_FASTREBOUND",
    ),
    ("SUSPENSION", "Rear3rdPackerSetting", "VM_REAR_3RD_PACKERS"),
    ("SUSPENSION", "Rear3rdSpringSetting", "VM_REAR_3RD_SPRING"),
    (
        "SUSPENSION",
        "Rear3rdTenderSpringSetting",
        "VM_REAR_3RD_TENDERSPRING",
    ),
    (
        "SUSPENSION",
        "Rear3rdTenderTravelSetting",
        "VM_REAR_3RD_TENDERSPRINGTRAVEL",
    ),
    (
        "SUSPENSION",
        "Rear3rdSlowBumpSetting",
        "VM_REAR_3RD_SLOWBUMP",
    ),
    (
        "SUSPENSION",
        "Rear3rdFastBumpSetting",
        "VM_REAR_3RD_FASTBUMP",
    ),
    (
        "SUSPENSION",
        "Rear3rdSlowReboundSetting",
        "VM_REAR_3RD_SLOWREBOUND",
    ),
    (
        "SUSPENSION",
        "Rear3rdFastReboundSetting",
        "VM_REAR_3RD_FASTREBOUND",
    ),
    ("SUSPENSION", "ChassisAdj00Setting", "VM_CHASSIS_ADJ_00"),
    ("SUSPENSION", "ChassisAdj01Setting", "VM_CHASSIS_ADJ_01"),
    ("SUSPENSION", "ChassisAdj02Setting", "VM_CHASSIS_ADJ_02"),
    ("SUSPENSION", "ChassisAdj03Setting", "VM_CHASSIS_ADJ_03"),
    ("SUSPENSION", "ChassisAdj04Setting", "VM_CHASSIS_ADJ_04"),
    ("SUSPENSION", "ChassisAdj05Setting", "VM_CHASSIS_ADJ_05"),
    ("SUSPENSION", "ChassisAdj06Setting", "VM_CHASSIS_ADJ_06"),
    ("SUSPENSION", "ChassisAdj07Setting", "VM_CHASSIS_ADJ_07"),
    ("SUSPENSION", "ChassisAdj08Setting", "VM_CHASSIS_ADJ_08"),
    ("SUSPENSION", "ChassisAdj09Setting", "VM_CHASSIS_ADJ_09"),
    ("SUSPENSION", "ChassisAdj10Setting", "VM_CHASSIS_ADJ_10"),
    ("SUSPENSION", "ChassisAdj11Setting", "VM_CHASSIS_ADJ_11"),
    ("CONTROLS", "SteerLockSetting", "VM_STEER_LOCK"),
    ("CONTROLS", "RearBrakeSetting", "VM_BRAKE_BALANCE"),
    ("CONTROLS", "BrakeMigrationSetting", "VM_BRAKE_MIGRATION"),
    ("CONTROLS", "BrakePressureSetting", "VM_BRAKE_PRESSURE"),
    (
        "CONTROLS",
        "HandfrontbrakePressSetting",
        "VM_HANDFRONTBRAKE_PRESSURE",
    ),
    ("CONTROLS", "HandbrakePressSetting", "VM_HANDBRAKE_PRESSURE"),
    ("CONTROLS", "TCSetting", "VM_TRACTION_CONTROL"),
    ("CONTROLS", "ABSSetting", "VM_ANTILOCK_BRAKES"),
    (
        "CONTROLS",
        "TractionControlMapSetting",
        "VM_TRACTIONCONTROLMAP",
    ),
    (
        "CONTROLS",
        "TCPowerCutMapSetting",
        "VM_TRACTIONCONTROLPOWERCUTMAP",
    ),
    (
        "CONTROLS",
        "TCSlipAngleMapSetting",
        "VM_TRACTIONCONTROLSLIPANGLEMAP",
    ),
    (
        "CONTROLS",
        "AntilockBrakeSystemMapSetting",
        "VM_ANTILOCKBRAKESYSTEMMAP",
    ),
    ("ENGINE", "RevLimitSetting", "VM_REV_LIMITER"),
    ("ENGINE", "EngineBoostSetting", "VM_ENGINE_BOOST"),
    ("ENGINE", "RegenerationMapSetting", "VM_REGEN_LEVEL"),
    ("ENGINE", "ElectricMotorMapSetting", "VM_ELECTRIC_MOTOR_MAP"),
    ("ENGINE", "Push2PassMapSetting", "VM_P2P_MAP"),
    ("ENGINE", "EngineMixtureSetting", "VM_ENGINE_MIXTURE"),
    ("ENGINE", "EngineBrakingMapSetting", "VM_ENGINE_BRAKEMAP"),
    ("DRIVELINE", "FinalDriveSetting", "VM_GEAR_FINAL"),
    ("DRIVELINE", "ReverseSetting", "VM_GEAR_REVERSE"),
    ("DRIVELINE", "Gear1Setting", "VM_GEAR_1"),
    ("DRIVELINE", "Gear2Setting", "VM_GEAR_2"),
    ("DRIVELINE", "Gear3Setting", "VM_GEAR_3"),
    ("DRIVELINE", "Gear4Setting", "VM_GEAR_4"),
    ("DRIVELINE", "Gear5Setting", "VM_GEAR_5"),
    ("DRIVELINE", "Gear6Setting", "VM_GEAR_6"),
    ("DRIVELINE", "Gear7Setting", "VM_GEAR_7"),
    ("DRIVELINE", "Gear8Setting", "VM_GEAR_8"),
    ("DRIVELINE", "Gear9Setting", "VM_GEAR_9"),
    ("DRIVELINE", "RatioSetSetting", "VM_RATIO_SET"),
    ("DRIVELINE", "DiffPumpSetting", "VM_DIFF_PUMP"),
    ("DRIVELINE", "DiffPowerSetting", "VM_DIFF_POWER"),
    ("DRIVELINE", "DiffCoastSetting", "VM_DIFF_COAST"),
    ("DRIVELINE", "DiffPreloadSetting", "VM_DIFF_PRELOAD"),
    ("DRIVELINE", "FrontDiffPumpSetting", "VM_FRONT_DIFF_PUMP"),
    ("DRIVELINE", "FrontDiffPowerSetting", "VM_FRONT_DIFF_POWER"),
    ("DRIVELINE", "FrontDiffCoastSetting", "VM_FRONT_DIFF_COAST"),
    (
        "DRIVELINE",
        "FrontDiffPreloadSetting",
        "VM_FRONT_DIFF_PRELOAD",
    ),
    ("DRIVELINE", "RearSplitSetting", "VM_TORQUE_SPLIT"),
    ("DRIVELINE", "GearAutoUpShiftSetting", "VM_GEAR_AUTOUPSHIFT"),
    (
        "DRIVELINE",
        "GearAutoDownShiftSetting",
        "VM_GEAR_AUTODOWNSHIFT",
    ),
    ("FRONTLEFT", "CamberSetting", "WM_CAMBER-W_FL"),
    ("FRONTLEFT", "PressureSetting", "WM_PRESSURE-W_FL"),
    ("FRONTLEFT", "PackerSetting", "WM_PACKERS-W_FL"),
    ("FRONTLEFT", "SpringSetting", "WM_SPRING-W_FL"),
    ("FRONTLEFT", "TenderSpringSetting", "WM_TENDERSPRING-W_FL"),
    (
        "FRONTLEFT",
        "TenderTravelSetting",
        "WM_TENDERSPRINGTRAVEL-W_FL",
    ),
    ("FRONTLEFT", "SpringRubberSetting", "WM_SRUBBER-W_FL"),
    ("FRONTLEFT", "RideHeightSetting", "WM_RIDEHEIGHT-W_FL"),
    ("FRONTLEFT", "SlowBumpSetting", "WM_SLOWBUMP-W_FL"),
    ("FRONTLEFT", "FastBumpSetting", "WM_FASTBUMP-W_FL"),
    ("FRONTLEFT", "SlowReboundSetting", "WM_SLOWREBOUND-W_FL"),
    ("FRONTLEFT", "FastReboundSetting", "WM_FASTREBOUND-W_FL"),
    ("FRONTLEFT", "BrakeDiscSetting", "WM_BRAKEDISC-W_FL"),
    ("FRONTLEFT", "BrakePadSetting", "WM_BRAKEPAD-W_FL"),
    ("FRONTRIGHT", "CamberSetting", "WM_CAMBER-W_FR"),
    ("FRONTRIGHT", "PressureSetting", "WM_PRESSURE-W_FR"),
    ("FRONTRIGHT", "PackerSetting", "WM_PACKERS-W_FR"),
    ("FRONTRIGHT", "SpringSetting", "WM_SPRING-W_FR"),
    ("FRONTRIGHT", "TenderSpringSetting", "WM_TENDERSPRING-W_FR"),
    (
        "FRONTRIGHT",
        "TenderTravelSetting",
        "WM_TENDERSPRINGTRAVEL-W_FR",
    ),
    ("FRONTRIGHT", "SpringRubberSetting", "WM_SRUBBER-W_FR"),
    ("FRONTRIGHT", "RideHeightSetting", "WM_RIDEHEIGHT-W_FR"),
    ("FRONTRIGHT", "SlowBumpSetting", "WM_SLOWBUMP-W_FR"),
    ("FRONTRIGHT", "FastBumpSetting", "WM_FASTBUMP-W_FR"),
    ("FRONTRIGHT", "SlowReboundSetting", "WM_SLOWREBOUND-W_FR"),
    ("FRONTRIGHT", "FastReboundSetting", "WM_FASTREBOUND-W_FR"),
    ("FRONTRIGHT", "BrakeDiscSetting", "WM_BRAKEDISC-W_FR"),
    ("FRONTRIGHT", "BrakePadSetting", "WM_BRAKEPAD-W_FR"),
    ("REARLEFT", "CamberSetting", "WM_CAMBER-W_RL"),
    ("REARLEFT", "PressureSetting", "WM_PRESSURE-W_RL"),
    ("REARLEFT", "PackerSetting", "WM_PACKERS-W_RL"),
    ("REARLEFT", "SpringSetting", "WM_SPRING-W_RL"),
    ("REARLEFT", "TenderSpringSetting", "WM_TENDERSPRING-W_RL"),
    (
        "REARLEFT",
        "TenderTravelSetting",
        "WM_TENDERSPRINGTRAVEL-W_RL",
    ),
    ("REARLEFT", "SpringRubberSetting", "WM_SRUBBER-W_RL"),
    ("REARLEFT", "RideHeightSetting", "WM_RIDEHEIGHT-W_RL"),
    ("REARLEFT", "SlowBumpSetting", "WM_SLOWBUMP-W_RL"),
    ("REARLEFT", "FastBumpSetting", "WM_FASTBUMP-W_RL"),
    ("REARLEFT", "SlowReboundSetting", "WM_SLOWREBOUND-W_RL"),
    ("REARLEFT", "FastReboundSetting", "WM_FASTREBOUND-W_RL"),
    ("REARLEFT", "BrakeDiscSetting", "WM_BRAKEDISC-W_RL"),
    ("REARLEFT", "BrakePadSetting", "WM_BRAKEPAD-W_RL"),
    ("REARRIGHT", "CamberSetting", "WM_CAMBER-W_RR"),
    ("REARRIGHT", "PressureSetting", "WM_PRESSURE-W_RR"),
    ("REARRIGHT", "PackerSetting", "WM_PACKERS-W_RR"),
    ("REARRIGHT", "SpringSetting", "WM_SPRING-W_RR"),
    ("REARRIGHT", "TenderSpringSetting", "WM_TENDERSPRING-W_RR"),
    (
        "REARRIGHT",
        "TenderTravelSetting",
        "WM_TENDERSPRINGTRAVEL-W_RR",
    ),
    ("REARRIGHT", "SpringRubberSetting", "WM_SRUBBER-W_RR"),
    ("REARRIGHT", "RideHeightSetting", "WM_RIDEHEIGHT-W_RR"),
    ("REARRIGHT", "SlowBumpSetting", "WM_SLOWBUMP-W_RR"),
    ("REARRIGHT", "FastBumpSetting", "WM_FASTBUMP-W_RR"),
    ("REARRIGHT", "SlowReboundSetting", "WM_SLOWREBOUND-W_RR"),
    ("REARRIGHT", "FastReboundSetting", "WM_FASTREBOUND-W_RR"),
    ("REARRIGHT", "BrakeDiscSetting", "WM_BRAKEDISC-W_RR"),
    ("REARRIGHT", "BrakePadSetting", "WM_BRAKEPAD-W_RR"),
];

pub fn export(metadata: &LmuMetadata, destination: &Path) -> Result<bool> {
    let setup = metadata.get("CarSetup");
    if setup.trim().is_empty() {
        return Ok(false);
    }

    let source: Value = serde_json::from_str(setup).context("invalid CarSetup JSON")?;
    let Some(source) = source.as_object().filter(|source| !source.is_empty()) else {
        return Ok(false);
    };

    let mut contents = String::new();
    writeln!(
        contents,
        "VehicleClassSetting=\"{}\"\r",
        metadata.car_class()
    )?;
    contents.push_str("UpgradeSetting=(0,0,0,0)\r\n\r\n");

    let mut current_section = "";
    for &(section, setting, source_key) in SETTINGS {
        if section != current_section {
            if !current_section.is_empty() {
                contents.push_str("\r\n");
            }
            writeln!(contents, "[{section}]\r")?;
            current_section = section;
        }
        if let Some(value) = setup_value(source.get(source_key)) {
            writeln!(contents, "{setting}={value}\r")?;
        }
    }
    contents.push_str("\r\n");

    write_safely(destination, contents.as_bytes())?;
    Ok(true)
}

fn setup_value(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Object(object) => object.get("value").and_then(number_value),
        Value::Bool(value) => Some(u8::from(*value).to_string()),
        _ => None,
    }
}

fn number_value(value: &Value) -> Option<String> {
    match value {
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn write_safely(destination: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temporary = temporary_path(destination);
    let result = (|| {
        std::fs::write(&temporary, contents)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn exports_svm_with_crlf_and_known_settings() {
        let metadata = LmuMetadata::new(BTreeMap::from([
            ("CarClass".to_owned(), "GT3".to_owned()),
            (
                "CarSetup".to_owned(),
                r#"{"symmetric":true,"VM_FUEL_LEVEL":{"value":42},"WM_PRESSURE-W_FL":{"value":7}}"#
                    .to_owned(),
            ),
        ]));
        let directory = tempdir().unwrap();
        let destination = directory.path().join("setup.svm");

        assert!(export(&metadata, &destination).unwrap());

        let contents = std::fs::read(&destination).unwrap();
        let text = String::from_utf8(contents.clone()).unwrap();
        assert!(text.contains("VehicleClassSetting=\"GT3\"\r\n"));
        assert!(text.contains("Symmetric=1\r\n"));
        assert!(text.contains("FuelSetting=42\r\n"));
        assert!(text.contains("PressureSetting=7\r\n"));
        assert!(!contents.windows(2).any(|pair| pair == b"\r\r"));
    }

    #[test]
    fn missing_setup_does_not_create_a_file() {
        let directory = tempdir().unwrap();
        let destination = directory.path().join("setup.svm");

        assert!(!export(&LmuMetadata::default(), &destination).unwrap());
        assert!(!destination.exists());
    }
}
