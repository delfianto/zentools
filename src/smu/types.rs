//! AMD Ryzen SMU type definitions

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use std::str::FromStr;
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

    #[error("MSR access failed for CPU {cpu}, MSR 0x{msr:X}: {reason}")]
    MsrError { cpu: u32, msr: u64, reason: String },

    #[error("SMN access failed for address 0x{address:X}: {reason}")]
    SmnError { address: u32, reason: String },
}

/// AMD CPU codename (matches ryzen_smu driver codename values)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuCodename {
    // Zen/Zen+ (Family 17h)
    Colfax,
    SummitRidge,
    PinnacleRidge,
    RavenRidge,
    RavenRidge2,
    Picasso,
    Dali,

    // Zen 2 (Family 17h)
    Matisse,
    CastlePeak,
    Renoir,
    Lucienne,

    // Zen 3 (Family 19h)
    Vermeer,
    Cezanne,
    Milan,
    Rembrandt,
    Vangogh,
    Chagall,

    // Zen 4 (Family 19h)
    Raphael,
    Phoenix,
    HawkPoint,
    StormPeak,

    // Zen 5 (Family 1Ah)
    StrixPoint,
    GraniteRidge,

    // Server
    Naples,

    // Unknown or unsupported
    Unknown(u32),
}

impl CpuCodename {
    /// Parse codename from ryzen_smu driver enumeration value
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => CpuCodename::Colfax,
            2 => CpuCodename::Renoir,
            3 => CpuCodename::Picasso,
            4 => CpuCodename::Matisse,
            5 | 6 => CpuCodename::CastlePeak,
            7 => CpuCodename::RavenRidge,
            8 => CpuCodename::RavenRidge2,
            9 => CpuCodename::SummitRidge,
            10 => CpuCodename::PinnacleRidge,
            11 => CpuCodename::Rembrandt,
            12 => CpuCodename::Vermeer,
            13 => CpuCodename::Vangogh,
            14 => CpuCodename::Cezanne,
            15 => CpuCodename::Milan,
            16 => CpuCodename::Dali,
            17 => CpuCodename::Lucienne,
            18 => CpuCodename::Naples,
            19 => CpuCodename::Chagall,
            20 => CpuCodename::Raphael,
            21 => CpuCodename::Phoenix,
            22 => CpuCodename::StrixPoint,
            23 => CpuCodename::GraniteRidge,
            24 => CpuCodename::HawkPoint,
            25 => CpuCodename::StormPeak,
            _ => CpuCodename::Unknown(value),
        }
    }

    /// Get human-readable name with generation
    pub fn as_str(&self) -> &'static str {
        match self {
            CpuCodename::Colfax => "Colfax (Zen)",
            CpuCodename::SummitRidge => "Summit Ridge (Zen)",
            CpuCodename::PinnacleRidge => "Pinnacle Ridge (Zen+)",
            CpuCodename::RavenRidge => "Raven Ridge (Zen)",
            CpuCodename::RavenRidge2 => "Raven Ridge 2 (Zen+)",
            CpuCodename::Picasso => "Picasso (Zen+)",
            CpuCodename::Dali => "Dali (Zen+)",
            CpuCodename::Matisse => "Matisse (Zen 2)",
            CpuCodename::CastlePeak => "Castle Peak (Zen 2)",
            CpuCodename::Renoir => "Renoir (Zen 2)",
            CpuCodename::Lucienne => "Lucienne (Zen 2)",
            CpuCodename::Vermeer => "Vermeer (Zen 3)",
            CpuCodename::Cezanne => "Cezanne (Zen 3)",
            CpuCodename::Milan => "Milan (Zen 3)",
            CpuCodename::Rembrandt => "Rembrandt (Zen 3+)",
            CpuCodename::Vangogh => "Vangogh (Zen 3)",
            CpuCodename::Chagall => "Chagall (Zen 3)",
            CpuCodename::Raphael => "Raphael (Zen 4)",
            CpuCodename::Phoenix => "Phoenix (Zen 4)",
            CpuCodename::HawkPoint => "Hawk Point (Zen 4)",
            CpuCodename::StormPeak => "Storm Peak (Zen 4)",
            CpuCodename::StrixPoint => "Strix Point (Zen 5)",
            CpuCodename::GraniteRidge => "Granite Ridge (Zen 5)",
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

    pub fn is_hedt(&self) -> bool {
        matches!(
            self,
            CpuCodename::CastlePeak | CpuCodename::Chagall | CpuCodename::StormPeak
        )
    }

    pub fn is_server(&self) -> bool {
        matches!(self, CpuCodename::Naples | CpuCodename::Milan)
    }

    pub fn is_zen5(&self) -> bool {
        matches!(
            self,
            CpuCodename::StrixPoint | CpuCodename::GraniteRidge
        )
    }

    /// Check if this CPU uses DDR5 (Zen 4 and newer)
    pub fn is_ddr5(&self) -> bool {
        matches!(
            self,
            CpuCodename::Raphael
                | CpuCodename::Phoenix
                | CpuCodename::HawkPoint
                | CpuCodename::StormPeak
                | CpuCodename::StrixPoint
                | CpuCodename::GraniteRidge
        )
    }
}

/// SMU firmware version
#[derive(Debug, Clone)]
pub struct SmuVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl FromStr for SmuVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let version_part = s.strip_prefix("SMU v").unwrap_or(s);

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid SMU version format: {}", s));
        }

        Ok(Self {
            major: parts[0]
                .parse()
                .map_err(|_| format!("Invalid major version: {}", parts[0]))?,
            minor: parts[1]
                .parse()
                .map_err(|_| format!("Invalid minor version: {}", parts[1]))?,
            patch: parts[2]
                .parse()
                .map_err(|_| format!("Invalid patch version: {}", parts[2]))?,
        })
    }
}

impl std::fmt::Display for SmuVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SMU v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Complete SMU information from the kernel driver
#[derive(Debug, Clone)]
pub struct SmuInfo {
    pub version: SmuVersion,
    pub codename: CpuCodename,
    pub drv_version: String,
    pub pm_table_version: u32,
    pub pm_table_size: u64,
    pub mp1_if_version: Option<u32>,
}

/// Raw PM table data
#[derive(Debug, Clone)]
pub struct PmTableData {
    pub version: u32,
    pub data: Vec<u8>,
}

impl PmTableData {
    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn read_f32(&self, offset: usize) -> Option<f32> {
        if offset + 4 > self.data.len() {
            return None;
        }
        let mut cursor = Cursor::new(&self.data[offset..offset + 4]);
        cursor.read_f32::<LittleEndian>().ok()
    }

    pub fn read_u32(&self, offset: usize) -> Option<u32> {
        if offset + 4 > self.data.len() {
            return None;
        }
        let mut cursor = Cursor::new(&self.data[offset..offset + 4]);
        cursor.read_u32::<LittleEndian>().ok()
    }

    pub fn read_bytes(&self, offset: usize, len: usize) -> Option<&[u8]> {
        if offset + len > self.data.len() {
            return None;
        }
        Some(&self.data[offset..offset + len])
    }
}

/// Which data source provided metrics
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetricsSource {
    /// Data from PM table via ryzen_smu driver
    PmTable,
    /// Data from direct register reads (MSR + SMN)
    DirectRegisters,
    /// Combination of PM table and direct reads
    Hybrid,
}

impl std::fmt::Display for MetricsSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricsSource::PmTable => write!(f, "PM Table (ryzen_smu)"),
            MetricsSource::DirectRegisters => write!(f, "Direct Registers (MSR/SMN)"),
            MetricsSource::Hybrid => write!(f, "Hybrid (PM Table + Direct)"),
        }
    }
}

/// Per-core metrics
#[derive(Debug, Clone, Default)]
pub struct CoreMetrics {
    pub core_id: u32,
    pub power_w: Option<f64>,
    pub frequency_mhz: Option<f64>,
    pub activity_pct: Option<f64>,
    pub sleep_pct: Option<f64>,
    pub voltage_v: Option<f64>,
    pub temp_c: Option<f64>,
    pub c0_pct: Option<f64>,
    pub cc1_pct: Option<f64>,
    pub cc6_pct: Option<f64>,
}

/// Unified CPU metrics from all available sources
#[derive(Debug, Clone)]
pub struct CpuMetrics {
    pub source: MetricsSource,

    // Temperature (from SMN or PM table)
    pub tctl_temp_c: Option<f64>,
    pub ccd_temps_c: Vec<Option<f64>>,

    // Power (from RAPL or PM table)
    pub package_power_w: Option<f64>,
    pub core_power_w: Option<f64>,
    pub soc_power_w: Option<f64>,

    // Voltage (from SMN SVI or PM table)
    pub core_voltage_v: Option<f64>,
    pub soc_voltage_v: Option<f64>,
    pub peak_voltage_v: Option<f64>,

    // PBO limits (PM table only)
    pub ppt_limit_w: Option<f64>,
    pub ppt_current_w: Option<f64>,
    pub tdc_limit_a: Option<f64>,
    pub tdc_current_a: Option<f64>,
    pub edc_limit_a: Option<f64>,
    pub edc_current_a: Option<f64>,
    pub tjmax_c: Option<f64>,

    // Clocks (PM table only)
    pub fclk_mhz: Option<f64>,
    pub fclk_avg_mhz: Option<f64>,
    pub uclk_mhz: Option<f64>,
    pub mclk_mhz: Option<f64>,

    // Additional voltages
    pub vddp_v: Option<f64>,
    pub vddg_v: Option<f64>,

    // Derived
    pub peak_core_freq_mhz: Option<f64>,
    pub avg_core_voltage_v: Option<f64>,
    pub soc_temp_c: Option<f64>,

    // Per-core data (PM table only)
    pub per_core: Vec<CoreMetrics>,
}

impl Default for CpuMetrics {
    fn default() -> Self {
        Self {
            source: MetricsSource::DirectRegisters,
            tctl_temp_c: None,
            ccd_temps_c: Vec::new(),
            package_power_w: None,
            core_power_w: None,
            soc_power_w: None,
            core_voltage_v: None,
            soc_voltage_v: None,
            peak_voltage_v: None,
            ppt_limit_w: None,
            ppt_current_w: None,
            tdc_limit_a: None,
            tdc_current_a: None,
            edc_limit_a: None,
            edc_current_a: None,
            tjmax_c: None,
            fclk_mhz: None,
            fclk_avg_mhz: None,
            uclk_mhz: None,
            mclk_mhz: None,
            vddp_v: None,
            vddg_v: None,
            peak_core_freq_mhz: None,
            avg_core_voltage_v: None,
            soc_temp_c: None,
            per_core: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // CpuCodename — from_u32 exhaustive
    // =========================================================================

    #[test]
    fn test_codename_from_u32_all_known() {
        let known = [
            (1, "Colfax"),
            (2, "Renoir"),
            (3, "Picasso"),
            (4, "Matisse"),
            (5, "Castle Peak"),
            (6, "Castle Peak"),
            (7, "Raven Ridge"),
            (8, "Raven Ridge 2"),
            (9, "Summit Ridge"),
            (10, "Pinnacle Ridge"),
            (11, "Rembrandt"),
            (12, "Vermeer"),
            (13, "Vangogh"),
            (14, "Cezanne"),
            (15, "Milan"),
            (16, "Dali"),
            (17, "Lucienne"),
            (18, "Naples"),
            (19, "Chagall"),
            (20, "Raphael"),
            (21, "Phoenix"),
            (22, "Strix Point"),
            (23, "Granite Ridge"),
            (24, "Hawk Point"),
            (25, "Storm Peak"),
        ];

        for (val, expected_name) in &known {
            let codename = CpuCodename::from_u32(*val);
            assert_eq!(
                codename.name(),
                *expected_name,
                "from_u32({}) should be {}",
                val,
                expected_name
            );
        }
    }

    #[test]
    fn test_codename_from_u32_unknown() {
        assert!(matches!(CpuCodename::from_u32(0), CpuCodename::Unknown(0)));
        assert!(matches!(CpuCodename::from_u32(26), CpuCodename::Unknown(26)));
        assert!(matches!(CpuCodename::from_u32(100), CpuCodename::Unknown(100)));
        assert!(matches!(CpuCodename::from_u32(u32::MAX), CpuCodename::Unknown(u32::MAX)));
    }

    #[test]
    fn test_codename_unknown_preserves_value() {
        if let CpuCodename::Unknown(v) = CpuCodename::from_u32(42) {
            assert_eq!(v, 42);
        } else {
            panic!("expected Unknown(42)");
        }
    }

    // =========================================================================
    // CpuCodename — as_str / name / generation
    // =========================================================================

    #[test]
    fn test_codename_as_str_non_empty() {
        for val in 0..=30 {
            let cn = CpuCodename::from_u32(val);
            assert!(!cn.as_str().is_empty(), "as_str empty for {}", val);
            assert!(!cn.name().is_empty(), "name empty for {}", val);
            assert!(!cn.generation().is_empty(), "generation empty for {}", val);
        }
    }

    #[test]
    fn test_codename_as_str_contains_generation() {
        let cn = CpuCodename::GraniteRidge;
        assert!(cn.as_str().contains("Zen 5"));
        assert!(cn.as_str().contains("Granite Ridge"));

        let cn = CpuCodename::Raphael;
        assert!(cn.as_str().contains("Zen 4"));

        let cn = CpuCodename::Vermeer;
        assert!(cn.as_str().contains("Zen 3"));

        let cn = CpuCodename::Matisse;
        assert!(cn.as_str().contains("Zen 2"));
    }

    #[test]
    fn test_codename_generation_values() {
        // Zen
        assert_eq!(CpuCodename::Colfax.generation(), "Zen");
        assert_eq!(CpuCodename::SummitRidge.generation(), "Zen");
        assert_eq!(CpuCodename::RavenRidge.generation(), "Zen");
        assert_eq!(CpuCodename::Naples.generation(), "Zen");

        // Zen+
        assert_eq!(CpuCodename::PinnacleRidge.generation(), "Zen+");
        assert_eq!(CpuCodename::RavenRidge2.generation(), "Zen+");
        assert_eq!(CpuCodename::Picasso.generation(), "Zen+");
        assert_eq!(CpuCodename::Dali.generation(), "Zen+");

        // Zen 2
        assert_eq!(CpuCodename::Matisse.generation(), "Zen 2");
        assert_eq!(CpuCodename::CastlePeak.generation(), "Zen 2");
        assert_eq!(CpuCodename::Renoir.generation(), "Zen 2");
        assert_eq!(CpuCodename::Lucienne.generation(), "Zen 2");

        // Zen 3
        assert_eq!(CpuCodename::Vermeer.generation(), "Zen 3");
        assert_eq!(CpuCodename::Cezanne.generation(), "Zen 3");
        assert_eq!(CpuCodename::Milan.generation(), "Zen 3");
        assert_eq!(CpuCodename::Vangogh.generation(), "Zen 3");
        assert_eq!(CpuCodename::Chagall.generation(), "Zen 3");

        // Zen 3+
        assert_eq!(CpuCodename::Rembrandt.generation(), "Zen 3+");

        // Zen 4
        assert_eq!(CpuCodename::Raphael.generation(), "Zen 4");
        assert_eq!(CpuCodename::Phoenix.generation(), "Zen 4");
        assert_eq!(CpuCodename::HawkPoint.generation(), "Zen 4");
        assert_eq!(CpuCodename::StormPeak.generation(), "Zen 4");

        // Zen 5
        assert_eq!(CpuCodename::StrixPoint.generation(), "Zen 5");
        assert_eq!(CpuCodename::GraniteRidge.generation(), "Zen 5");

        // Unknown
        assert_eq!(CpuCodename::Unknown(0).generation(), "Unknown");
    }

    // =========================================================================
    // CpuCodename — classification
    // =========================================================================

    #[test]
    fn test_codename_is_desktop() {
        let desktops = [
            CpuCodename::SummitRidge,
            CpuCodename::PinnacleRidge,
            CpuCodename::Matisse,
            CpuCodename::Vermeer,
            CpuCodename::Raphael,
            CpuCodename::GraniteRidge,
        ];
        for cn in &desktops {
            assert!(cn.is_desktop(), "{:?} should be desktop", cn);
            assert!(!cn.is_mobile(), "{:?} should not be mobile", cn);
            assert!(!cn.is_hedt(), "{:?} should not be HEDT", cn);
        }
    }

    #[test]
    fn test_codename_is_mobile() {
        let mobiles = [
            CpuCodename::RavenRidge,
            CpuCodename::RavenRidge2,
            CpuCodename::Picasso,
            CpuCodename::Dali,
            CpuCodename::Renoir,
            CpuCodename::Lucienne,
            CpuCodename::Cezanne,
            CpuCodename::Rembrandt,
            CpuCodename::Vangogh,
            CpuCodename::Phoenix,
            CpuCodename::HawkPoint,
            CpuCodename::StrixPoint,
        ];
        for cn in &mobiles {
            assert!(cn.is_mobile(), "{:?} should be mobile", cn);
            assert!(!cn.is_desktop(), "{:?} should not be desktop", cn);
            assert!(!cn.is_hedt(), "{:?} should not be HEDT", cn);
        }
    }

    #[test]
    fn test_codename_is_hedt() {
        let hedts = [
            CpuCodename::CastlePeak,
            CpuCodename::Chagall,
            CpuCodename::StormPeak,
        ];
        for cn in &hedts {
            assert!(cn.is_hedt(), "{:?} should be HEDT", cn);
            assert!(!cn.is_desktop(), "{:?} should not be desktop", cn);
            assert!(!cn.is_mobile(), "{:?} should not be mobile", cn);
        }
    }

    #[test]
    fn test_codename_is_server() {
        assert!(CpuCodename::Naples.is_server());
        assert!(CpuCodename::Milan.is_server());
        assert!(!CpuCodename::Raphael.is_server());
        assert!(!CpuCodename::Unknown(0).is_server());
    }

    #[test]
    fn test_codename_is_zen5() {
        assert!(CpuCodename::GraniteRidge.is_zen5());
        assert!(CpuCodename::StrixPoint.is_zen5());
        assert!(!CpuCodename::Raphael.is_zen5());
        assert!(!CpuCodename::Vermeer.is_zen5());
        assert!(!CpuCodename::Unknown(0).is_zen5());
    }

    #[test]
    fn test_codename_unknown_classification() {
        let unk = CpuCodename::Unknown(99);
        assert!(!unk.is_desktop());
        assert!(!unk.is_mobile());
        assert!(!unk.is_hedt());
        assert!(!unk.is_server());
        assert!(!unk.is_zen5());
    }

    #[test]
    fn test_codename_clone_copy_eq() {
        let cn = CpuCodename::Raphael;
        let cn2 = cn; // Copy
        let cn3 = cn.clone();
        assert_eq!(cn, cn2);
        assert_eq!(cn, cn3);
        assert_ne!(cn, CpuCodename::Vermeer);
    }

    // =========================================================================
    // SmuVersion — FromStr
    // =========================================================================

    #[test]
    fn test_smu_version_parse_with_prefix() {
        let v: SmuVersion = "SMU v98.82.0".parse().unwrap();
        assert_eq!(v.major, 98);
        assert_eq!(v.minor, 82);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_smu_version_parse_without_prefix() {
        let v: SmuVersion = "76.54.3".parse().unwrap();
        assert_eq!(v.major, 76);
        assert_eq!(v.minor, 54);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_smu_version_parse_with_whitespace() {
        let v: SmuVersion = "  12.34.56  ".parse().unwrap();
        assert_eq!(v.major, 12);
        assert_eq!(v.minor, 34);
        assert_eq!(v.patch, 56);
    }

    #[test]
    fn test_smu_version_parse_zero() {
        let v: SmuVersion = "0.0.0".parse().unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_smu_version_parse_max() {
        let v: SmuVersion = "255.255.255".parse().unwrap();
        assert_eq!(v.major, 255);
        assert_eq!(v.minor, 255);
        assert_eq!(v.patch, 255);
    }

    #[test]
    fn test_smu_version_parse_invalid_empty() {
        assert!("".parse::<SmuVersion>().is_err());
    }

    #[test]
    fn test_smu_version_parse_invalid_too_few_parts() {
        assert!("1.2".parse::<SmuVersion>().is_err());
        assert!("1".parse::<SmuVersion>().is_err());
    }

    #[test]
    fn test_smu_version_parse_invalid_too_many_parts() {
        assert!("1.2.3.4".parse::<SmuVersion>().is_err());
    }

    #[test]
    fn test_smu_version_parse_invalid_non_numeric() {
        assert!("a.b.c".parse::<SmuVersion>().is_err());
        assert!("1.2.x".parse::<SmuVersion>().is_err());
    }

    #[test]
    fn test_smu_version_parse_invalid_overflow() {
        assert!("256.0.0".parse::<SmuVersion>().is_err()); // > u8::MAX
        assert!("0.999.0".parse::<SmuVersion>().is_err());
    }

    #[test]
    fn test_smu_version_parse_invalid_negative() {
        assert!("-1.0.0".parse::<SmuVersion>().is_err());
    }

    // =========================================================================
    // SmuVersion — Display
    // =========================================================================

    #[test]
    fn test_smu_version_display() {
        let v: SmuVersion = "98.82.0".parse().unwrap();
        assert_eq!(v.to_string(), "SMU v98.82.0");
    }

    #[test]
    fn test_smu_version_display_roundtrip() {
        let v: SmuVersion = "SMU v12.34.56".parse().unwrap();
        let s = v.to_string();
        let v2: SmuVersion = s.parse().unwrap();
        assert_eq!(v.major, v2.major);
        assert_eq!(v.minor, v2.minor);
        assert_eq!(v.patch, v2.patch);
    }

    // =========================================================================
    // PmTableData
    // =========================================================================

    #[test]
    fn test_pm_table_data_size() {
        let pt = PmTableData {
            version: 0,
            data: vec![0; 100],
        };
        assert_eq!(pt.size(), 100);
    }

    #[test]
    fn test_pm_table_data_size_empty() {
        let pt = PmTableData {
            version: 0,
            data: vec![],
        };
        assert_eq!(pt.size(), 0);
    }

    #[test]
    fn test_pm_table_read_f32_valid() {
        let data = 42.5f32.to_le_bytes().to_vec();
        let pt = PmTableData { version: 0, data };
        let v = pt.read_f32(0).unwrap();
        assert!((v - 42.5).abs() < 0.001);
    }

    #[test]
    fn test_pm_table_read_f32_negative() {
        let data = (-123.456f32).to_le_bytes().to_vec();
        let pt = PmTableData { version: 0, data };
        let v = pt.read_f32(0).unwrap();
        assert!((v - (-123.456)).abs() < 0.01);
    }

    #[test]
    fn test_pm_table_read_f32_out_of_bounds() {
        let pt = PmTableData {
            version: 0,
            data: vec![0; 3], // too small for f32
        };
        assert_eq!(pt.read_f32(0), None);
    }

    #[test]
    fn test_pm_table_read_f32_at_exact_end() {
        let data = 1.0f32.to_le_bytes().to_vec();
        let pt = PmTableData {
            version: 0,
            data: data.clone(),
        };
        assert!(pt.read_f32(0).is_some()); // exactly fits
        assert_eq!(pt.read_f32(1), None); // 1 byte past
    }

    #[test]
    fn test_pm_table_read_u32_valid() {
        let data = 0xDEADBEEFu32.to_le_bytes().to_vec();
        let pt = PmTableData { version: 0, data };
        assert_eq!(pt.read_u32(0), Some(0xDEADBEEF));
    }

    #[test]
    fn test_pm_table_read_u32_zero() {
        let data = 0u32.to_le_bytes().to_vec();
        let pt = PmTableData { version: 0, data };
        assert_eq!(pt.read_u32(0), Some(0));
    }

    #[test]
    fn test_pm_table_read_u32_max() {
        let data = u32::MAX.to_le_bytes().to_vec();
        let pt = PmTableData { version: 0, data };
        assert_eq!(pt.read_u32(0), Some(u32::MAX));
    }

    #[test]
    fn test_pm_table_read_u32_out_of_bounds() {
        let pt = PmTableData {
            version: 0,
            data: vec![0; 2],
        };
        assert_eq!(pt.read_u32(0), None);
    }

    #[test]
    fn test_pm_table_read_bytes_valid() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let pt = PmTableData { version: 0, data };
        assert_eq!(pt.read_bytes(2, 3), Some(&[3u8, 4, 5][..]));
    }

    #[test]
    fn test_pm_table_read_bytes_full() {
        let data = vec![10, 20, 30];
        let pt = PmTableData {
            version: 0,
            data: data.clone(),
        };
        assert_eq!(pt.read_bytes(0, 3), Some(&data[..]));
    }

    #[test]
    fn test_pm_table_read_bytes_out_of_bounds() {
        let data = vec![1, 2, 3];
        let pt = PmTableData { version: 0, data };
        assert_eq!(pt.read_bytes(0, 4), None);
        assert_eq!(pt.read_bytes(2, 2), None);
    }

    #[test]
    fn test_pm_table_read_bytes_zero_length() {
        let data = vec![1, 2, 3];
        let pt = PmTableData { version: 0, data };
        assert_eq!(pt.read_bytes(0, 0), Some(&[][..]));
    }

    #[test]
    fn test_pm_table_multiple_values() {
        let mut data = vec![0u8; 16];
        data[0..4].copy_from_slice(&100.0f32.to_le_bytes());
        data[4..8].copy_from_slice(&200.0f32.to_le_bytes());
        data[8..12].copy_from_slice(&42u32.to_le_bytes());
        data[12..16].copy_from_slice(&99u32.to_le_bytes());

        let pt = PmTableData { version: 0, data };
        assert!((pt.read_f32(0).unwrap() - 100.0).abs() < 0.01);
        assert!((pt.read_f32(4).unwrap() - 200.0).abs() < 0.01);
        assert_eq!(pt.read_u32(8), Some(42));
        assert_eq!(pt.read_u32(12), Some(99));
    }

    #[test]
    fn test_pm_table_nan_inf() {
        let mut data = vec![0u8; 8];
        data[0..4].copy_from_slice(&f32::NAN.to_le_bytes());
        data[4..8].copy_from_slice(&f32::INFINITY.to_le_bytes());

        let pt = PmTableData { version: 0, data };
        let nan_val = pt.read_f32(0).unwrap();
        assert!(nan_val.is_nan());
        let inf_val = pt.read_f32(4).unwrap();
        assert!(inf_val.is_infinite());
    }

    // =========================================================================
    // MetricsSource — Display
    // =========================================================================

    #[test]
    fn test_metrics_source_display() {
        assert!(MetricsSource::PmTable.to_string().contains("PM Table"));
        assert!(MetricsSource::DirectRegisters.to_string().contains("Direct"));
        assert!(MetricsSource::Hybrid.to_string().contains("Hybrid"));
    }

    #[test]
    fn test_metrics_source_eq() {
        assert_eq!(MetricsSource::PmTable, MetricsSource::PmTable);
        assert_ne!(MetricsSource::PmTable, MetricsSource::Hybrid);
    }

    // =========================================================================
    // CpuMetrics — Default
    // =========================================================================

    #[test]
    fn test_cpu_metrics_default() {
        let m = CpuMetrics::default();
        assert_eq!(m.source, MetricsSource::DirectRegisters);
        assert!(m.tctl_temp_c.is_none());
        assert!(m.ccd_temps_c.is_empty());
        assert!(m.package_power_w.is_none());
        assert!(m.core_power_w.is_none());
        assert!(m.soc_power_w.is_none());
        assert!(m.core_voltage_v.is_none());
        assert!(m.soc_voltage_v.is_none());
        assert!(m.peak_voltage_v.is_none());
        assert!(m.ppt_limit_w.is_none());
        assert!(m.ppt_current_w.is_none());
        assert!(m.tdc_limit_a.is_none());
        assert!(m.tdc_current_a.is_none());
        assert!(m.edc_limit_a.is_none());
        assert!(m.edc_current_a.is_none());
        assert!(m.tjmax_c.is_none());
        assert!(m.fclk_mhz.is_none());
        assert!(m.fclk_avg_mhz.is_none());
        assert!(m.uclk_mhz.is_none());
        assert!(m.mclk_mhz.is_none());
        assert!(m.vddp_v.is_none());
        assert!(m.vddg_v.is_none());
        assert!(m.peak_core_freq_mhz.is_none());
        assert!(m.avg_core_voltage_v.is_none());
        assert!(m.soc_temp_c.is_none());
        assert!(m.per_core.is_empty());
    }

    // =========================================================================
    // CoreMetrics — Default
    // =========================================================================

    #[test]
    fn test_core_metrics_default() {
        let cm = CoreMetrics::default();
        assert_eq!(cm.core_id, 0);
        assert!(cm.power_w.is_none());
        assert!(cm.frequency_mhz.is_none());
        assert!(cm.activity_pct.is_none());
        assert!(cm.sleep_pct.is_none());
        assert!(cm.voltage_v.is_none());
        assert!(cm.temp_c.is_none());
        assert!(cm.c0_pct.is_none());
        assert!(cm.cc1_pct.is_none());
        assert!(cm.cc6_pct.is_none());
    }

    // =========================================================================
    // SmuError — Display
    // =========================================================================

    #[test]
    fn test_smu_error_display_driver_not_found() {
        let err = SmuError::DriverNotFound { path: "/test".to_string() };
        let msg = format!("{}", err);
        assert!(msg.contains("/test"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_smu_error_display_permission_denied() {
        let err = SmuError::PermissionDenied { path: "/test".to_string() };
        let msg = format!("{}", err);
        assert!(msg.contains("Permission denied"));
        assert!(msg.contains("sudo"));
    }

    #[test]
    fn test_smu_error_display_unsupported_version() {
        let err = SmuError::UnsupportedPmTableVersion { version: 0xABCDEF };
        let msg = format!("{}", err);
        assert!(msg.contains("ABCDEF"));
    }

    #[test]
    fn test_smu_error_display_pm_table_too_small() {
        let err = SmuError::PmTableTooSmall { expected: 256, actual: 10 };
        let msg = format!("{}", err);
        assert!(msg.contains("256"));
        assert!(msg.contains("10"));
    }

    #[test]
    fn test_smu_error_display_msr() {
        let err = SmuError::MsrError { cpu: 5, msr: 0xC0010299, reason: "denied".to_string() };
        let msg = format!("{}", err);
        assert!(msg.contains("CPU 5"));
        assert!(msg.contains("C0010299"));
        assert!(msg.contains("denied"));
    }

    #[test]
    fn test_smu_error_display_smn() {
        let err = SmuError::SmnError { address: 0x59800, reason: "fail".to_string() };
        let msg = format!("{}", err);
        assert!(msg.contains("59800"));
        assert!(msg.contains("fail"));
    }

    #[test]
    fn test_smu_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<SmuError>();
        assert_sync::<SmuError>();
    }

    // =========================================================================
    // SMU_DRV_PATH constant
    // =========================================================================

    #[test]
    fn test_smu_drv_path() {
        assert_eq!(SMU_DRV_PATH, "/sys/kernel/ryzen_smu_drv");
        assert!(SMU_DRV_PATH.starts_with("/sys/"));
    }
}
