//! Strict decoding of the audited AOD getter protocol. This module sends no WMI requests.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
use thiserror::Error;

pub const AUDITED_HASH: &str = "219659fefb95ff23dded68aac16ff6af35ffa2863baf4999172250d7788d2f96";
pub const AUDITED_BOARD: &str = "MPG X870E CARBON WIFI (MS-7E49)";
pub const AUDITED_BIOS: &str = "1.AA3";
const OBJECTS: &str = include_str!("../tools/aod-probe/objects.json");

#[derive(Debug, Error)]
pub enum AodError {
    #[error("AOD capture: {0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Deserialize)]
struct Object {
    id: u32,
    method: u32,
    length: usize,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AodSetting {
    pub id: u32,
    pub name: String,
    pub firmware_name: String,
    pub raw: u32,
    pub display: String,
    pub interpretation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AodCapture {
    pub source: String,
    pub metadata_verified: bool,
    pub settings: Vec<AodSetting>,
}

impl AodCapture {
    /// Raw captures may be imported without metadata, but never presented as current telemetry.
    pub fn from_path(path: &Path) -> Result<Self, AodError> {
        let path = if path.is_dir() {
            path.join("aod-getters.txt")
        } else {
            path.to_owned()
        };
        if fs::metadata(&path)?.len() > 1024 * 1024 {
            return Err(AodError::Invalid("file exceeds 1 MiB".into()));
        }
        let mut capture = Self::parse(&fs::read_to_string(&path)?)?;
        capture.source = format!("Saved capture · {}", path.display());
        let metadata = path.with_file_name("metadata.json");
        if metadata.exists() {
            if fs::metadata(&metadata)?.len() > 65536 {
                return Err(AodError::Invalid("metadata exceeds 64 KiB".into()));
            }
            let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(metadata)?)?;
            for (key, expected) in [
                ("board", AUDITED_BOARD),
                ("bios", AUDITED_BIOS),
                ("table_sha256", AUDITED_HASH),
            ] {
                if value.get(key).and_then(|v| v.as_str()) != Some(expected) {
                    return Err(AodError::Invalid(format!(
                        "unsupported {key} in capture metadata"
                    )));
                }
            }
            capture.metadata_verified = true;
        }
        Ok(capture)
    }

    pub fn parse(text: &str) -> Result<Self, AodError> {
        let objects: Vec<Object> = serde_json::from_str(OBJECTS)?;
        let definitions: BTreeMap<_, _> = objects.iter().map(|o| (o.id, o)).collect();
        let mut records = BTreeMap::new();
        for line in text.lines() {
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.len() != 9 {
                return Err(AodError::Invalid("expected nine columns per record".into()));
            }
            let hex = |s: &str| {
                u32::from_str_radix(s, 16)
                    .map_err(|_| AodError::Invalid(format!("invalid word {s}")))
            };
            let id = hex(parts[0])?;
            let method: u32 = parts[1]
                .parse()
                .map_err(|_| AodError::Invalid("invalid method".into()))?;
            let length: usize = parts[3]
                .parse()
                .map_err(|_| AodError::Invalid("invalid length".into()))?;
            let object = definitions
                .get(&id)
                .ok_or_else(|| AodError::Invalid(format!("unknown object {id:08X}")))?;
            let words: Vec<u32> = parts[4..]
                .iter()
                .map(|s| hex(s))
                .collect::<Result<_, _>>()?;
            if !matches!(method, 4 | 6)
                || method != object.method
                || hex(parts[2])? != 0
                || length != object.length
                || words[0] != id
            {
                return Err(AodError::Invalid(format!(
                    "invalid getter response {id:08X}"
                )));
            }
            let (name, display, interpretation) = describe(id, words[1], &object.name);
            if records
                .insert(
                    id,
                    AodSetting {
                        id,
                        name,
                        display,
                        interpretation,
                        firmware_name: object.name.clone(),
                        raw: words[1],
                    },
                )
                .is_some()
            {
                return Err(AodError::Invalid(format!("duplicate object {id:08X}")));
            }
        }
        if records.len() != definitions.len() {
            return Err(AodError::Invalid("capture is missing AOD objects".into()));
        }
        Ok(Self {
            source: "Saved AOD getter capture (metadata not supplied)".into(),
            metadata_verified: false,
            settings: records.into_values().collect(),
        })
    }
}

fn describe(id: u32, value: u32, firmware_name: &str) -> (String, String, String) {
    let (name, kind) = match id {
        0x20051 => ("ProcODT pull-up P0", "odt"),
        0x20052 => ("ProcODT pull-down P0", "odt"),
        0x20053 => ("DRAM drive pull-up P0", "drive"),
        0x20054 => ("DRAM drive pull-down P0", "drive"),
        0x2003e => ("RTT nominal write", "rtt"),
        0x2003f => ("RTT nominal read", "rtt"),
        0x20023 => ("RTT write", "rtt"),
        0x20024 => ("RTT park", "rtt"),
        0x20040 => ("RTT park DQS", "rtt"),
        0x30001 => ("VDDIO setting", "mv"),
        0x30007 => ("DIMM VDDQ setting", "mv"),
        0x30003 => ("VPP setting", "mv"),
        0x30004 => ("APU VDDIO setting", "mv"),
        0x2004d => ("Nitro RX", "nitro"),
        0x2004e => ("Nitro TX", "nitro"),
        0x2004f => ("Nitro control", "nitro"),
        _ => (firmware_name, "raw"),
    };
    let unknown = || format!("Unknown code {value}");
    let display = match kind {
        "odt" => match value {
            0 => "Hi-Z".into(),
            1..=7 => format!(
                "{} Ω",
                ["480", "240", "160", "120", "96", "80", "68.6"][(value - 1) as usize]
            ),
            12..=15 => format!("{} Ω", ["60", "53.3", "48", "43.6"][(value - 12) as usize]),
            28..=31 => format!("{} Ω", ["40", "36.9", "34.3", "32"][(value - 28) as usize]),
            60..=63 => format!(
                "{} Ω",
                ["30", "28.2", "26.7", "25.3"][(value - 60) as usize]
            ),
            _ => unknown(),
        },
        "drive" => match value {
            0 => "34 Ω".into(),
            1 => "40 Ω".into(),
            2 => "48 Ω".into(),
            _ => unknown(),
        },
        "rtt" => match value {
            0 => "Off (encoded setting)".into(),
            1..=7 => format!("RZQ/{value} ({:.1} Ω nominal)", 240.0 / value as f64),
            _ => unknown(),
        },
        "nitro" if value > 3 => unknown(),
        "mv" if value == 0 => "Auto / unspecified (raw 0)".into(),
        "mv" => format!("{:.3} V configured", value as f64 / 1000.0),
        _ => value.to_string(),
    };
    let interpretation = match kind {
        "odt" | "drive" | "rtt" => "Provisional electrical encoding · compare with BIOS",
        "mv" => "Firmware setting · not a voltage measurement",
        "nitro" => "Firmware setting · encoding matched UMC in the audited capture",
        _ => "Raw firmware setting · zero may mean Auto / no override",
    };
    (name.into(), display, interpretation.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!("../tools/aod-probe/fixtures/ms7e49-1aa3.txt");
    #[test]
    fn validates_fixture_and_electrical_encodings() {
        let capture = AodCapture::parse(FIXTURE).unwrap();
        assert_eq!(capture.settings.len(), 81);
        let setting = |id| capture.settings.iter().find(|s| s.id == id).unwrap();
        assert_eq!(setting(0x20051).display, "40 Ω");
        assert_eq!(setting(0x30001).display, "1.350 V configured");
        assert!(setting(0x20051).interpretation.contains("Provisional"));
        assert!(!capture.metadata_verified);
    }
    #[test]
    fn rejects_partial_duplicate_setter_and_failed_responses() {
        assert!(
            AodCapture::parse(&FIXTURE.lines().skip(1).collect::<Vec<_>>().join("\n")).is_err()
        );
        assert!(
            AodCapture::parse(&format!("{FIXTURE}\n{}", FIXTURE.lines().next().unwrap())).is_err()
        );
        assert!(AodCapture::parse(&FIXTURE.replacen(" 6 ", " 5 ", 1)).is_err());
        assert!(AodCapture::parse(&FIXTURE.replacen(" 00000000 20 ", " 00000001 20 ", 1)).is_err());
        assert!(AodCapture::parse(&FIXTURE.replacen(" 20 ", " 412 ", 1)).is_err());
    }
    #[test]
    fn retains_unknown_and_auto_codes() {
        assert_eq!(describe(0x20051, 999, "").1, "Unknown code 999");
        assert!(describe(0x30001, 0, "").1.contains("Auto"));
        assert!(describe(0x999, 0, "Override").2.contains("no override"));
    }
}
