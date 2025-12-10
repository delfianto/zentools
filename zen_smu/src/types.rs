//! AMD Ryzen System Management Unit (SMU) interface library
//!
//! This library provides access to AMD Ryzen CPU telemetry and control
//! through the ryzen_smu kernel driver.

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use thiserror::Error;

/// Path to the ryzen_smu driver sysfs interface
pub const SMU_DRV_PATH: &str = "/sys/kernel/ryzen_smu_drv";

/// Errors that can occur when interacting with the SMU
#[derive(Error, Debug)]
pub enum SmuError {
    #[error("SMU driver not found at {path}. Ensure ryzen_smu kernel module is loaded.")]
    DriverNotFound { path: String },

    #[error("Permission denied accessing {path}. Try running with sudo.")]
    PermissionDenied { path: String },

    #[error("Failed to read {path}: {source}")]
    ReadError {
        path: String,
        source: std::io::Error,
    },

    #[error("Failed to parse data from {path}: {reason}")]
    ParseError { path: String, reason: String },

    #[error("PM table version 0x{version:X} is not supported")]
    UnsupportedPmTableVersion { version: u32 },

    #[error("PM table too small: expected at least {expected} bytes, got {actual}")]
    PmTableTooSmall { expected: usize, actual: usize },

    #[error("Invalid codename value: {0}")]
    InvalidCodename(u32),
}

/// AMD CPU codename (matches ryzen_smu driver codename values)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuCodename {
    // Zen/Zen+ (Family 17h models 00h-0Fh, 10h-1Fh)
    Colfax,        // Zen  - Colfax
    SummitRidge,   // Zen  - Ryzen 1000 desktop
    PinnacleRidge, // Zen+ - Ryzen 2000 desktop
    RavenRidge,    // Zen  - Ryzen 2000 APU
    RavenRidge2,   // Zen+ - Raven Ridge 2
    Picasso,       // Zen+ - Ryzen 3000 APU
    Dali,          // Zen+ - Athlon 3000

    // Zen 2 (Family 17h models 30h-3Fh, 60h-6Fh, 70h-7Fh)
    Matisse,    // Zen 2 - Ryzen 3000 desktop
    CastlePeak, // Zen 2 - Ryzen 3000 Threadripper
    Renoir,     // Zen 2 - Ryzen 4000 mobile
    Lucienne,   // Zen 2 - Ryzen 5000 mobile, 7nm refresh

    // Zen 3 (Family 19h models 00h-0Fh, 20h-2Fh, 40h-4Fh, 50h-5Fh)
    Vermeer,   // Zen 3  - Ryzen 5000 desktop
    Cezanne,   // Zen 3  - Ryzen 5000 mobile
    Milan,     // Zen 3  - EPYC 7003
    Rembrandt, // Zen 3+ - Ryzen 6000 mobile, 6nm
    Vangogh,   // Zen 3  - Steam Deck APU
    Chagall,   // Zen 3  - Ryzen 5000 Threadripper Pro

    // Zen 4 (Family 19h models 60h-6Fh, 70h-7Fh, A0h-AFh)
    Raphael,   // Zen 4 - Ryzen 7000 desktop
    Phoenix,   // Zen 4 - Ryzen 7040 mobile
    HawkPoint, // Zen 4 - Ryzen 8040 mobile, refresh
    StormPeak, // Zen 4 - Ryzen 7000 Threadripper

    // Zen 5 (Family 1Ah models 00h-0Fh, 20h-2Fh, 40h-4Fh)
    StrixPoint,   // Zen 5 - Ryzen AI 300 mobile
    GraniteRidge, // Zen 5 - Ryzen 9000 desktop

    // Server
    Naples, // EPYC 7001 (Naples)

    // Unknown or unsupported codename
    Unknown(u32),
}

impl CpuCodename {
    /// Parse codename from driver value
    ///
    /// These values must match the exact codename enumeration used by the
    /// ryzen_smu kernel driver. See the driver's README for the mapping table.
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => CpuCodename::Colfax,         // 0x01
            2 => CpuCodename::Renoir,         // 0x02
            3 => CpuCodename::Picasso,        // 0x03
            4 => CpuCodename::Matisse,        // 0x04
            5 => CpuCodename::CastlePeak,     // 0x05 (legacy "Threadripper")
            6 => CpuCodename::CastlePeak,     // 0x06
            7 => CpuCodename::RavenRidge,     // 0x07
            8 => CpuCodename::RavenRidge2,    // 0x08
            9 => CpuCodename::SummitRidge,    // 0x09
            10 => CpuCodename::PinnacleRidge, // 0x0A
            11 => CpuCodename::Rembrandt,     // 0x0B
            12 => CpuCodename::Vermeer,       // 0x0C
            13 => CpuCodename::Vangogh,       // 0x0D
            14 => CpuCodename::Cezanne,       // 0x0E
            15 => CpuCodename::Milan,         // 0x0F
            16 => CpuCodename::Dali,          // 0x10
            17 => CpuCodename::Lucienne,      // 0x11
            18 => CpuCodename::Naples,        // 0x12
            19 => CpuCodename::Chagall,       // 0x13
            20 => CpuCodename::Raphael,       // 0x14 - Zen 4 desktop
            21 => CpuCodename::Phoenix,       // 0x15 - Zen 4 mobile
            22 => CpuCodename::StrixPoint,    // 0x16 - Zen 5 mobile
            23 => CpuCodename::GraniteRidge,  // 0x17 - Zen 5 desktop (9950X!)
            24 => CpuCodename::HawkPoint,     // 0x18 - Zen 4 mobile refresh
            25 => CpuCodename::StormPeak,     // 0x19 - Threadripper 7000
            _ => CpuCodename::Unknown(value),
        }
    }

    /// Get human-readable name with generation
    pub fn as_str(&self) -> &'static str {
        match self {
            // Zen/Zen+
            CpuCodename::Colfax => "Colfax (Zen)",
            CpuCodename::SummitRidge => "Summit Ridge (Zen)",
            CpuCodename::PinnacleRidge => "Pinnacle Ridge (Zen+)",
            CpuCodename::RavenRidge => "Raven Ridge (Zen)",
            CpuCodename::RavenRidge2 => "Raven Ridge 2 (Zen+)",
            CpuCodename::Picasso => "Picasso (Zen+)",
            CpuCodename::Dali => "Dali (Zen+)",

            // Zen 2
            CpuCodename::Matisse => "Matisse (Zen 2)",
            CpuCodename::CastlePeak => "Castle Peak (Zen 2)",
            CpuCodename::Renoir => "Renoir (Zen 2)",
            CpuCodename::Lucienne => "Lucienne (Zen 2)",

            // Zen 3
            CpuCodename::Vermeer => "Vermeer (Zen 3)",
            CpuCodename::Cezanne => "Cezanne (Zen 3)",
            CpuCodename::Milan => "Milan (Zen 3)",
            CpuCodename::Rembrandt => "Rembrandt (Zen 3+)",
            CpuCodename::Vangogh => "Vangogh (Zen 3)",
            CpuCodename::Chagall => "Chagall (Zen 3)",

            // Zen 4
            CpuCodename::Raphael => "Raphael (Zen 4)",
            CpuCodename::Phoenix => "Phoenix (Zen 4)",
            CpuCodename::HawkPoint => "Hawk Point (Zen 4)",
            CpuCodename::StormPeak => "Storm Peak (Zen 4)",

            // Zen 5
            CpuCodename::StrixPoint => "Strix Point (Zen 5)",
            CpuCodename::GraniteRidge => "Granite Ridge (Zen 5)",

            // Server
            CpuCodename::Naples => "Naples (Zen)",

            CpuCodename::Unknown(_) => "Unknown",
        }
    }

    /// Get short codename without generation
    pub fn name(&self) -> &'static str {
        match self {
            CpuCodename::Colfax => "Colfax",
            CpuCodename::SummitRidge => "Summit Ridge",
            CpuCodename::PinnacleRidge => "Pinnacle Ridge",
            CpuCodename::RavenRidge => "Raven Ridge",
            CpuCodename::RavenRidge2 => "Raven Ridge 2",
            CpuCodename::Picasso => "Picasso",
            CpuCodename::Dali => "Dali",
            CpuCodename::Matisse => "Matisse",
            CpuCodename::CastlePeak => "Castle Peak",
            CpuCodename::Renoir => "Renoir",
            CpuCodename::Lucienne => "Lucienne",
            CpuCodename::Vermeer => "Vermeer",
            CpuCodename::Cezanne => "Cezanne",
            CpuCodename::Milan => "Milan",
            CpuCodename::Rembrandt => "Rembrandt",
            CpuCodename::Vangogh => "Vangogh",
            CpuCodename::Chagall => "Chagall",
            CpuCodename::Raphael => "Raphael",
            CpuCodename::Phoenix => "Phoenix",
            CpuCodename::HawkPoint => "Hawk Point",
            CpuCodename::StormPeak => "Storm Peak",
            CpuCodename::StrixPoint => "Strix Point",
            CpuCodename::GraniteRidge => "Granite Ridge",
            CpuCodename::Naples => "Naples",
            CpuCodename::Unknown(_) => "Unknown",
        }
    }

    /// Get Zen generation
    pub fn generation(&self) -> &'static str {
        match self {
            CpuCodename::Colfax
            | CpuCodename::SummitRidge
            | CpuCodename::RavenRidge
            | CpuCodename::Naples => "Zen",

            CpuCodename::PinnacleRidge
            | CpuCodename::RavenRidge2
            | CpuCodename::Picasso
            | CpuCodename::Dali => "Zen+",

            CpuCodename::Matisse
            | CpuCodename::CastlePeak
            | CpuCodename::Renoir
            | CpuCodename::Lucienne => "Zen 2",

            CpuCodename::Vermeer
            | CpuCodename::Cezanne
            | CpuCodename::Milan
            | CpuCodename::Vangogh
            | CpuCodename::Chagall => "Zen 3",

            CpuCodename::Rembrandt => "Zen 3+",

            CpuCodename::Raphael
            | CpuCodename::Phoenix
            | CpuCodename::HawkPoint
            | CpuCodename::StormPeak => "Zen 4",

            CpuCodename::StrixPoint | CpuCodename::GraniteRidge => "Zen 5",

            CpuCodename::Unknown(_) => "Unknown",
        }
    }

    /// Check if this is a desktop processor
    pub fn is_desktop(&self) -> bool {
        matches!(
            self,
            CpuCodename::SummitRidge
                | CpuCodename::PinnacleRidge
                | CpuCodename::Matisse
                | CpuCodename::Vermeer
                | CpuCodename::Raphael
                | CpuCodename::GraniteRidge
        )
    }

    /// Check if this is a mobile processor
    pub fn is_mobile(&self) -> bool {
        matches!(
            self,
            CpuCodename::RavenRidge
                | CpuCodename::RavenRidge2
                | CpuCodename::Picasso
                | CpuCodename::Dali
                | CpuCodename::Renoir
                | CpuCodename::Lucienne
                | CpuCodename::Cezanne
                | CpuCodename::Rembrandt
                | CpuCodename::Vangogh
                | CpuCodename::Phoenix
                | CpuCodename::HawkPoint
                | CpuCodename::StrixPoint
        )
    }

    /// Check if this is a workstation/HEDT processor
    pub fn is_hedt(&self) -> bool {
        matches!(
            self,
            CpuCodename::CastlePeak | CpuCodename::Chagall | CpuCodename::StormPeak
        )
    }

    /// Check if this is a server processor
    pub fn is_server(&self) -> bool {
        matches!(self, CpuCodename::Naples | CpuCodename::Milan)
    }
}

/// SMU firmware version
#[derive(Debug, Clone)]
pub struct SmuVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl SmuVersion {
    /// Parse version from string like "SMU v98.82.0" or "98.82.0"
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.trim();

        // Handle both "SMU v98.82.0" and "98.82.0" formats
        let version_part = s.strip_prefix("SMU v").unwrap_or(s);

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }
}

impl std::fmt::Display for SmuVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SMU v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Complete SMU information
#[derive(Debug, Clone)]
pub struct SmuInfo {
    /// SMU firmware version
    pub version: SmuVersion,
    /// CPU codename
    pub codename: CpuCodename,
    /// Driver version
    pub drv_version: String,
    /// PM table version
    pub pm_table_version: u32,
    /// PM table size in bytes
    pub pm_table_size: u64,
    /// MP1 interface version
    pub mp1_if_version: Option<u32>,
}

/// Raw PM table data
#[derive(Debug, Clone)]
pub struct PmTableData {
    /// PM table version
    pub version: u32,
    /// Raw binary data
    pub data: Vec<u8>,
}

impl PmTableData {
    /// Get size in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Read a f32 value at byte offset
    pub fn read_f32(&self, offset: usize) -> Option<f32> {
        if offset + 4 > self.data.len() {
            return None;
        }
        let mut cursor = Cursor::new(&self.data[offset..offset + 4]);
        cursor.read_f32::<LittleEndian>().ok()
    }

    /// Read a u32 value at byte offset
    pub fn read_u32(&self, offset: usize) -> Option<u32> {
        if offset + 4 > self.data.len() {
            return None;
        }
        let mut cursor = Cursor::new(&self.data[offset..offset + 4]);
        cursor.read_u32::<LittleEndian>().ok()
    }

    /// Get raw bytes at offset
    pub fn read_bytes(&self, offset: usize, len: usize) -> Option<&[u8]> {
        if offset + len > self.data.len() {
            return None;
        }
        Some(&self.data[offset..offset + len])
    }
}

/// Basic metrics that can be read from PM table (version-agnostic)
#[derive(Debug, Clone)]
pub struct BasicMetrics {
    pub table_version: u32,
    pub table_size: usize,
}
