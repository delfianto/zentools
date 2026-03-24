//! AMD Energy Performance Preference (EPP) management
//!
//! Reads and writes AMD EPP settings through the Linux sysfs interface.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur when working with EPP settings
#[derive(Error, Debug)]
pub enum EppError {
    #[error("No CPU energy preference files found")]
    NoCpusFound,

    #[error("Permission denied accessing {path}: {source}")]
    PermissionDenied {
        path: String,
        source: io::Error,
    },

    #[error("Failed to access {path}: {source}")]
    IoError {
        path: String,
        source: io::Error,
    },

    #[error("Invalid CPU number in path: {0}")]
    InvalidCpuNumber(String),

    #[error("Invalid EPP value: {0}")]
    InvalidEppValue(String),
}

/// AMD EPP profile values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EppProfile {
    /// Prioritizes performance above power saving
    Performance,
    /// Balance leaning towards performance (default on many systems)
    BalancePerformance,
    /// Balance leaning towards power saving
    BalancePower,
    /// Strongly prioritizes power saving
    Power,
}

impl FromStr for EppProfile {
    type Err = EppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "performance" => Ok(EppProfile::Performance),
            "balance_performance" => Ok(EppProfile::BalancePerformance),
            "balance_power" => Ok(EppProfile::BalancePower),
            "power" => Ok(EppProfile::Power),
            other => Err(EppError::InvalidEppValue(other.to_string())),
        }
    }
}

impl EppProfile {
    /// Convert profile to kernel-recognized string
    pub fn as_str(&self) -> &'static str {
        match self {
            EppProfile::Performance => "performance",
            EppProfile::BalancePerformance => "balance_performance",
            EppProfile::BalancePower => "balance_power",
            EppProfile::Power => "power",
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            EppProfile::Performance => {
                "Prioritizes performance above power saving. CPU reaches higher clock speeds aggressively."
            }
            EppProfile::BalancePerformance => {
                "Aims for a balance but leans towards performance. This is the default value in many systems."
            }
            EppProfile::BalancePower => {
                "Aims for a balance but leans towards power saving. More conservative clock speed increases."
            }
            EppProfile::Power => {
                "Strongly prioritizes power saving. Favors lower frequencies, may limit peak performance."
            }
        }
    }

    /// Convert from profile level (0-3)
    pub fn from_level(level: u8) -> Option<Self> {
        match level {
            0 => Some(EppProfile::Performance),
            1 => Some(EppProfile::BalancePerformance),
            2 => Some(EppProfile::BalancePower),
            3 => Some(EppProfile::Power),
            _ => None,
        }
    }

    /// Get all available profiles
    pub fn all() -> &'static [EppProfile] {
        &[
            EppProfile::Performance,
            EppProfile::BalancePerformance,
            EppProfile::BalancePower,
            EppProfile::Power,
        ]
    }
}

/// Information about a CPU's EPP setting
#[derive(Debug, Clone)]
pub struct CpuEppInfo {
    pub cpu_num: u32,
    pub profile: EppProfile,
    pub path: PathBuf,
}

/// Manager for AMD EPP settings
pub struct EppManager {
    epp_paths: Vec<PathBuf>,
}

impl EppManager {
    /// Initialize the manager by discovering all CPU EPP paths
    pub fn new() -> Result<Self, EppError> {
        let mut epp_paths = Vec::new();
        let cpu_dir = PathBuf::from("/sys/devices/system/cpu/");

        let entries = fs::read_dir(&cpu_dir).map_err(|e| EppError::IoError {
            path: cpu_dir.display().to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| EppError::IoError {
                path: cpu_dir.display().to_string(),
                source: e,
            })?;

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(dir_name) = path.file_name().and_then(|s| s.to_str())
                && let Some(cpu_num_str) = dir_name.strip_prefix("cpu")
                && cpu_num_str.parse::<u32>().is_ok()
            {
                let epp_path = path.join("cpufreq/energy_performance_preference");
                if epp_path.exists() {
                    epp_paths.push(epp_path);
                }
            }
        }

        epp_paths.sort();

        if epp_paths.is_empty() {
            return Err(EppError::NoCpusFound);
        }

        Ok(Self { epp_paths })
    }

    /// Get number of CPUs with EPP support
    pub fn cpu_count(&self) -> usize {
        self.epp_paths.len()
    }

    /// Apply EPP profile to all CPUs
    pub fn apply_profile(&self, profile: EppProfile) -> Result<(), EppError> {
        let profile_str = format!("{}\n", profile.as_str());

        for path in &self.epp_paths {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .open(path)
                .map_err(|e| {
                    if e.kind() == io::ErrorKind::PermissionDenied {
                        EppError::PermissionDenied {
                            path: path.display().to_string(),
                            source: e,
                        }
                    } else {
                        EppError::IoError {
                            path: path.display().to_string(),
                            source: e,
                        }
                    }
                })?;

            file.write_all(profile_str.as_bytes())
                .map_err(|e| EppError::IoError {
                    path: path.display().to_string(),
                    source: e,
                })?;

            file.flush().map_err(|e| EppError::IoError {
                path: path.display().to_string(),
                source: e,
            })?;
        }

        Ok(())
    }

    /// Read current EPP settings for all CPUs
    pub fn read_all(&self) -> Result<Vec<CpuEppInfo>, EppError> {
        let mut cpu_infos = Vec::new();

        for path in &self.epp_paths {
            let cpu_num = extract_cpu_number(path)?;

            let mut epp_value = String::new();
            fs::File::open(path)
                .map_err(|e| EppError::IoError {
                    path: path.display().to_string(),
                    source: e,
                })?
                .read_to_string(&mut epp_value)
                .map_err(|e| EppError::IoError {
                    path: path.display().to_string(),
                    source: e,
                })?;

            let profile: EppProfile = epp_value.parse()?;

            cpu_infos.push(CpuEppInfo {
                cpu_num,
                profile,
                path: path.clone(),
            });
        }

        cpu_infos.sort_by_key(|info| info.cpu_num);
        Ok(cpu_infos)
    }

    /// Read EPP setting for a specific CPU
    pub fn read_cpu(&self, cpu_num: u32) -> Result<EppProfile, EppError> {
        for path in &self.epp_paths {
            if extract_cpu_number(path)? == cpu_num {
                let mut epp_value = String::new();
                fs::File::open(path)
                    .map_err(|e| EppError::IoError {
                        path: path.display().to_string(),
                        source: e,
                    })?
                    .read_to_string(&mut epp_value)
                    .map_err(|e| EppError::IoError {
                        path: path.display().to_string(),
                        source: e,
                    })?;

                return epp_value.parse();
            }
        }

        Err(EppError::InvalidCpuNumber(cpu_num.to_string()))
    }
}

/// Extract CPU number from sysfs EPP path
fn extract_cpu_number(path: &Path) -> Result<u32, EppError> {
    path.parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("cpu"))
        .and_then(|num_str| num_str.parse().ok())
        .ok_or_else(|| EppError::InvalidCpuNumber(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // =========================================================================
    // EppProfile::as_str
    // =========================================================================

    #[test]
    fn test_epp_profile_as_str() {
        assert_eq!(EppProfile::Performance.as_str(), "performance");
        assert_eq!(
            EppProfile::BalancePerformance.as_str(),
            "balance_performance"
        );
        assert_eq!(EppProfile::BalancePower.as_str(), "balance_power");
        assert_eq!(EppProfile::Power.as_str(), "power");
    }

    #[test]
    fn test_epp_profile_as_str_unique() {
        let profiles = EppProfile::all();
        let mut strings: Vec<&str> = profiles.iter().map(|p| p.as_str()).collect();
        let original_len = strings.len();
        strings.sort();
        strings.dedup();
        assert_eq!(strings.len(), original_len, "all profile strings must be unique");
    }

    // =========================================================================
    // EppProfile::description
    // =========================================================================

    #[test]
    fn test_epp_profile_description_non_empty() {
        for profile in EppProfile::all() {
            let desc = profile.description();
            assert!(!desc.is_empty(), "{:?} description is empty", profile);
            assert!(desc.len() > 20, "{:?} description too short", profile);
        }
    }

    #[test]
    fn test_epp_profile_description_unique() {
        let descriptions: Vec<&str> = EppProfile::all().iter().map(|p| p.description()).collect();
        for (i, a) in descriptions.iter().enumerate() {
            for (j, b) in descriptions.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "descriptions should be unique");
                }
            }
        }
    }

    // =========================================================================
    // FromStr (parse)
    // =========================================================================

    #[test]
    fn test_epp_profile_parse_all_valid() {
        assert!(matches!(
            "performance".parse::<EppProfile>(),
            Ok(EppProfile::Performance)
        ));
        assert!(matches!(
            "balance_performance".parse::<EppProfile>(),
            Ok(EppProfile::BalancePerformance)
        ));
        assert!(matches!(
            "balance_power".parse::<EppProfile>(),
            Ok(EppProfile::BalancePower)
        ));
        assert!(matches!(
            "power".parse::<EppProfile>(),
            Ok(EppProfile::Power)
        ));
    }

    #[test]
    fn test_epp_profile_parse_with_whitespace() {
        // The kernel may return values with trailing newlines
        assert!(matches!(
            "performance\n".parse::<EppProfile>(),
            Ok(EppProfile::Performance)
        ));
        assert!(matches!(
            "  balance_power  ".parse::<EppProfile>(),
            Ok(EppProfile::BalancePower)
        ));
        assert!(matches!(
            "\tpower\t\n".parse::<EppProfile>(),
            Ok(EppProfile::Power)
        ));
    }

    #[test]
    fn test_epp_profile_parse_invalid() {
        assert!("invalid".parse::<EppProfile>().is_err());
        assert!("".parse::<EppProfile>().is_err());
        assert!("Performance".parse::<EppProfile>().is_err()); // case-sensitive
        assert!("PERFORMANCE".parse::<EppProfile>().is_err());
        assert!("balance-performance".parse::<EppProfile>().is_err()); // hyphen vs underscore
        assert!("balance performance".parse::<EppProfile>().is_err()); // space
    }

    #[test]
    fn test_epp_profile_parse_error_contains_value() {
        let err = "bogus_value".parse::<EppProfile>().unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("bogus_value"), "error should contain the invalid value: {}", msg);
    }

    // =========================================================================
    // Roundtrip: as_str -> parse
    // =========================================================================

    #[test]
    fn test_epp_profile_roundtrip() {
        for profile in EppProfile::all() {
            let s = profile.as_str();
            let parsed: EppProfile = s.parse().unwrap();
            assert_eq!(*profile, parsed, "roundtrip failed for {:?}", profile);
        }
    }

    // =========================================================================
    // from_level
    // =========================================================================

    #[test]
    fn test_epp_profile_from_level_valid() {
        assert_eq!(EppProfile::from_level(0), Some(EppProfile::Performance));
        assert_eq!(
            EppProfile::from_level(1),
            Some(EppProfile::BalancePerformance)
        );
        assert_eq!(EppProfile::from_level(2), Some(EppProfile::BalancePower));
        assert_eq!(EppProfile::from_level(3), Some(EppProfile::Power));
    }

    #[test]
    fn test_epp_profile_from_level_invalid() {
        assert_eq!(EppProfile::from_level(4), None);
        assert_eq!(EppProfile::from_level(5), None);
        assert_eq!(EppProfile::from_level(100), None);
        assert_eq!(EppProfile::from_level(255), None);
    }

    // =========================================================================
    // all()
    // =========================================================================

    #[test]
    fn test_epp_all_profiles() {
        let profiles = EppProfile::all();
        assert_eq!(profiles.len(), 4);
        assert!(profiles.contains(&EppProfile::Performance));
        assert!(profiles.contains(&EppProfile::BalancePerformance));
        assert!(profiles.contains(&EppProfile::BalancePower));
        assert!(profiles.contains(&EppProfile::Power));
    }

    #[test]
    fn test_epp_all_profiles_matches_from_level() {
        // Every level 0-3 should produce a profile that's in all()
        let all = EppProfile::all();
        for level in 0..=3u8 {
            let profile = EppProfile::from_level(level).unwrap();
            assert!(all.contains(&profile), "level {} not in all()", level);
        }
    }

    // =========================================================================
    // Clone, Copy, PartialEq, Eq, Debug
    // =========================================================================

    #[test]
    fn test_epp_profile_clone_copy() {
        let p = EppProfile::Performance;
        let p2 = p; // Copy
        let p3 = p.clone(); // Clone
        assert_eq!(p, p2);
        assert_eq!(p, p3);
    }

    #[test]
    fn test_epp_profile_equality() {
        assert_eq!(EppProfile::Performance, EppProfile::Performance);
        assert_ne!(EppProfile::Performance, EppProfile::Power);
        assert_ne!(EppProfile::BalancePerformance, EppProfile::BalancePower);
    }

    #[test]
    fn test_epp_profile_debug() {
        let debug = format!("{:?}", EppProfile::BalancePerformance);
        assert_eq!(debug, "BalancePerformance");
    }

    // =========================================================================
    // extract_cpu_number
    // =========================================================================

    #[test]
    fn test_extract_cpu_number_valid() {
        let path = PathBuf::from("/sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference");
        assert_eq!(extract_cpu_number(&path).unwrap(), 0);

        let path = PathBuf::from("/sys/devices/system/cpu/cpu15/cpufreq/energy_performance_preference");
        assert_eq!(extract_cpu_number(&path).unwrap(), 15);

        let path = PathBuf::from("/sys/devices/system/cpu/cpu127/cpufreq/energy_performance_preference");
        assert_eq!(extract_cpu_number(&path).unwrap(), 127);
    }

    #[test]
    fn test_extract_cpu_number_invalid_no_cpu_prefix() {
        let path = PathBuf::from("/sys/devices/system/cpu/notcpu0/cpufreq/energy_performance_preference");
        assert!(extract_cpu_number(&path).is_err());
    }

    #[test]
    fn test_extract_cpu_number_invalid_no_number() {
        let path = PathBuf::from("/sys/devices/system/cpu/cpuXX/cpufreq/energy_performance_preference");
        assert!(extract_cpu_number(&path).is_err());
    }

    #[test]
    fn test_extract_cpu_number_invalid_too_short_path() {
        let path = PathBuf::from("/cpu0");
        assert!(extract_cpu_number(&path).is_err());

        let path = PathBuf::from("cpu0/cpufreq");
        assert!(extract_cpu_number(&path).is_err());
    }

    #[test]
    fn test_extract_cpu_number_invalid_empty() {
        let path = PathBuf::from("");
        assert!(extract_cpu_number(&path).is_err());
    }

    // =========================================================================
    // CpuEppInfo
    // =========================================================================

    #[test]
    fn test_cpu_epp_info_struct() {
        let info = CpuEppInfo {
            cpu_num: 42,
            profile: EppProfile::Power,
            path: PathBuf::from("/sys/test"),
        };
        assert_eq!(info.cpu_num, 42);
        assert_eq!(info.profile, EppProfile::Power);

        // Clone
        let info2 = info.clone();
        assert_eq!(info2.cpu_num, 42);
    }

    // =========================================================================
    // EppError formatting
    // =========================================================================

    #[test]
    fn test_epp_error_display() {
        let err = EppError::NoCpusFound;
        assert!(format!("{}", err).contains("No CPU"));

        let err = EppError::InvalidEppValue("bogus".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("bogus"));
        assert!(msg.contains("Invalid EPP"));

        let err = EppError::InvalidCpuNumber("999".to_string());
        assert!(format!("{}", err).contains("999"));
    }

    #[test]
    fn test_epp_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        // EppError contains io::Error which is Send+Sync
        // This won't compile if EppError isn't Send+Sync
        assert_send::<EppError>();
        assert_sync::<EppError>();
    }
}
