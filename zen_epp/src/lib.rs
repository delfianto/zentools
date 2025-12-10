//! AMD Energy Performance Preference (EPP) management library
//!
//! This library provides functionality to read and write AMD EPP settings
//! through the Linux sysfs interface.

use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
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

    /// Parse from string value
    pub fn from_str(s: &str) -> Result<Self, EppError> {
        match s.trim() {
            "performance" => Ok(EppProfile::Performance),
            "balance_performance" => Ok(EppProfile::BalancePerformance),
            "balance_power" => Ok(EppProfile::BalancePower),
            "power" => Ok(EppProfile::Power),
            other => Err(EppError::InvalidEppValue(other.to_string())),
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
    /// CPU number
    pub cpu_num: u32,
    /// Current EPP profile
    pub profile: EppProfile,
    /// Path to the EPP file
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

        let entries = fs::read_dir(&cpu_dir)
            .map_err(|e| EppError::IoError {
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

            if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
                if let Some(cpu_num_str) = dir_name.strip_prefix("cpu") {
                    if cpu_num_str.parse::<u32>().is_ok() {
                        let epp_path = path.join("cpufreq/energy_performance_preference");
                        if epp_path.exists() {
                            epp_paths.push(epp_path);
                        }
                    }
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

            file.flush()
                .map_err(|e| EppError::IoError {
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
            let cpu_num = self.extract_cpu_number(path)?;

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

            let profile = EppProfile::from_str(&epp_value)?;

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
            if self.extract_cpu_number(path)? == cpu_num {
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

                return EppProfile::from_str(&epp_value);
            }
        }

        Err(EppError::InvalidCpuNumber(cpu_num.to_string()))
    }

    /// Extract CPU number from path
    fn extract_cpu_number(&self, path: &PathBuf) -> Result<u32, EppError> {
        path.parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("cpu"))
            .and_then(|num_str| num_str.parse().ok())
            .ok_or_else(|| EppError::InvalidCpuNumber(path.display().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epp_profile_conversion() {
        assert_eq!(EppProfile::Performance.as_str(), "performance");
        assert_eq!(
            EppProfile::BalancePerformance.as_str(),
            "balance_performance"
        );
        assert_eq!(EppProfile::BalancePower.as_str(), "balance_power");
        assert_eq!(EppProfile::Power.as_str(), "power");
    }

    #[test]
    fn test_epp_profile_from_str() {
        assert!(matches!(
            EppProfile::from_str("performance"),
            Ok(EppProfile::Performance)
        ));
        assert!(matches!(
            EppProfile::from_str("balance_performance"),
            Ok(EppProfile::BalancePerformance)
        ));
        assert!(matches!(
            EppProfile::from_str("balance_power"),
            Ok(EppProfile::BalancePower)
        ));
        assert!(matches!(
            EppProfile::from_str("power"),
            Ok(EppProfile::Power)
        ));
        assert!(EppProfile::from_str("invalid").is_err());
    }

    #[test]
    fn test_epp_profile_from_level() {
        assert_eq!(EppProfile::from_level(0), Some(EppProfile::Performance));
        assert_eq!(
            EppProfile::from_level(1),
            Some(EppProfile::BalancePerformance)
        );
        assert_eq!(EppProfile::from_level(2), Some(EppProfile::BalancePower));
        assert_eq!(EppProfile::from_level(3), Some(EppProfile::Power));
        assert_eq!(EppProfile::from_level(4), None);
    }

    #[test]
    fn test_epp_all_profiles() {
        let profiles = EppProfile::all();
        assert_eq!(profiles.len(), 4);
        assert!(profiles.contains(&EppProfile::Performance));
        assert!(profiles.contains(&EppProfile::BalancePerformance));
        assert!(profiles.contains(&EppProfile::BalancePower));
        assert!(profiles.contains(&EppProfile::Power));
    }
}
