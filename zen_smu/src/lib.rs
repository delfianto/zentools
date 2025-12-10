//! AMD Ryzen SMU interface library
//!
//! Provides access to AMD Ryzen CPU telemetry through the ryzen_smu kernel driver.

mod types;

pub use types::*;

use byteorder::{LittleEndian, ReadBytesExt};
use std::fs;
use std::io::Cursor;
use std::path::Path;

/// SMU Manager for reading CPU telemetry
pub struct SmuManager;

impl SmuManager {
    /// Check if the ryzen_smu driver is loaded
    pub fn check_driver() -> Result<(), SmuError> {
        let smu_path = Path::new(SMU_DRV_PATH);

        if !smu_path.exists() {
            return Err(SmuError::DriverNotFound {
                path: SMU_DRV_PATH.to_string(),
            });
        }

        let pm_table_path = smu_path.join("pm_table");
        if !pm_table_path.exists() {
            return Err(SmuError::DriverNotFound {
                path: pm_table_path.display().to_string(),
            });
        }

        // Try to read to check permissions
        fs::read(&pm_table_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                SmuError::PermissionDenied {
                    path: pm_table_path.display().to_string(),
                }
            } else {
                SmuError::ReadError {
                    path: pm_table_path.display().to_string(),
                    source: e,
                }
            }
        })?;

        Ok(())
    }

    /// Read complete SMU information
    pub fn read_info() -> Result<SmuInfo, SmuError> {
        Self::check_driver()?;

        // Read version
        let version_str = Self::read_sysfs_string("version")?;
        let version = SmuVersion::from_str(&version_str).ok_or_else(|| SmuError::ParseError {
            path: format!("{}/version", SMU_DRV_PATH),
            reason: format!("Invalid version format: {}", version_str),
        })?;

        // Read codename
        let codename_str = Self::read_sysfs_string("codename")?;
        let codename_val =
            codename_str
                .trim()
                .parse::<u32>()
                .map_err(|e| SmuError::ParseError {
                    path: format!("{}/codename", SMU_DRV_PATH),
                    reason: format!("Failed to parse codename: {}", e),
                })?;
        let codename = CpuCodename::from_u32(codename_val);

        // Read driver version
        let drv_version = Self::read_sysfs_string("drv_version")?;

        // Read PM table version (binary, little-endian)
        let pm_table_version = Self::read_sysfs_u64("pm_table_version")? as u32;

        // Read PM table size (binary, little-endian)
        let pm_table_size = Self::read_sysfs_u64("pm_table_size")?;

        // Read MP1 IF version (optional)
        let mp1_if_version = Self::read_sysfs_u32_optional("mp1_if_version");

        Ok(SmuInfo {
            version,
            codename,
            drv_version,
            pm_table_version,
            pm_table_size,
            mp1_if_version,
        })
    }

    /// Read PM table data
    pub fn read_pm_table(force: bool) -> Result<PmTableData, SmuError> {
        Self::check_driver()?;

        let info = Self::read_info()?;
        let pm_table_path = format!("{}/pm_table", SMU_DRV_PATH);

        let data = fs::read(&pm_table_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                SmuError::PermissionDenied {
                    path: pm_table_path.clone(),
                }
            } else {
                SmuError::ReadError {
                    path: pm_table_path.clone(),
                    source: e,
                }
            }
        })?;

        // Version check (can be bypassed with force flag)
        if !force && !Self::is_version_supported(info.pm_table_version) {
            return Err(SmuError::UnsupportedPmTableVersion {
                version: info.pm_table_version,
            });
        }

        Ok(PmTableData {
            version: info.pm_table_version,
            data,
        })
    }

    /// Parse basic metrics from PM table
    pub fn parse_basic_metrics(pm_table: &PmTableData) -> Result<BasicMetrics, SmuError> {
        // Minimum size check
        if pm_table.data.len() < 256 {
            return Err(SmuError::PmTableTooSmall {
                expected: 256,
                actual: pm_table.data.len(),
            });
        }

        Ok(BasicMetrics {
            table_version: pm_table.version,
            table_size: pm_table.data.len(),
        })
    }

    /// Check if PM table version is known/supported
    fn is_version_supported(version: u32) -> bool {
        // Known versions (this list should be expanded)
        matches!(version, 0x240903 | 0x240802 | 0x240803)
    }

    /// Read a string from sysfs
    fn read_sysfs_string(filename: &str) -> Result<String, SmuError> {
        let path = format!("{}/{}", SMU_DRV_PATH, filename);
        fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .map_err(|e| SmuError::ReadError {
                path: path.clone(),
                source: e,
            })
    }

    /// Read a u64 from sysfs (binary, little-endian)
    fn read_sysfs_u64(filename: &str) -> Result<u64, SmuError> {
        let path = format!("{}/{}", SMU_DRV_PATH, filename);
        let bytes = fs::read(&path).map_err(|e| SmuError::ReadError {
            path: path.clone(),
            source: e,
        })?;

        let value = if bytes.len() >= 8 {
            let mut cursor = Cursor::new(&bytes[..8]);
            cursor
                .read_u64::<LittleEndian>()
                .map_err(|e| SmuError::ParseError {
                    path: path.clone(),
                    reason: format!("Failed to parse u64: {}", e),
                })?
        } else if bytes.len() >= 4 {
            let mut cursor = Cursor::new(&bytes[..4]);
            cursor
                .read_u32::<LittleEndian>()
                .map_err(|e| SmuError::ParseError {
                    path: path.clone(),
                    reason: format!("Failed to parse u32: {}", e),
                })? as u64
        } else {
            return Err(SmuError::ParseError {
                path: path.clone(),
                reason: format!("File too small: {} bytes", bytes.len()),
            });
        };

        Ok(value)
    }

    /// Read a u32 from sysfs (optional, returns None if file doesn't exist)
    fn read_sysfs_u32_optional(filename: &str) -> Option<u32> {
        let path = format!("{}/{}", SMU_DRV_PATH, filename);
        let bytes = fs::read(&path).ok()?;

        if bytes.len() >= 4 {
            let mut cursor = Cursor::new(&bytes[..4]);
            cursor.read_u32::<LittleEndian>().ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smu_version_parsing() {
        // Test with "SMU v" prefix
        let version = SmuVersion::from_str("SMU v98.82.0").unwrap();
        assert_eq!(version.major, 98);
        assert_eq!(version.minor, 82);
        assert_eq!(version.patch, 0);
        assert_eq!(version.to_string(), "SMU v98.82.0");

        // Test without prefix (raw format from sysfs)
        let version2 = SmuVersion::from_str("98.82.0").unwrap();
        assert_eq!(version2.major, 98);
        assert_eq!(version2.minor, 82);
        assert_eq!(version2.patch, 0);
        assert_eq!(version2.to_string(), "SMU v98.82.0");

        // Test with whitespace
        let version3 = SmuVersion::from_str("  98.82.0  ").unwrap();
        assert_eq!(version3.major, 98);
    }

    #[test]
    fn test_codename_conversion() {
        // Test Zen 5 - Granite Ridge (9950X)
        assert!(matches!(
            CpuCodename::from_u32(23),
            CpuCodename::GraniteRidge
        ));

        // Test Zen 3 - Vermeer
        assert!(matches!(CpuCodename::from_u32(12), CpuCodename::Vermeer));

        // Test Zen 4 - Raphael
        assert!(matches!(CpuCodename::from_u32(20), CpuCodename::Raphael));

        // Test Zen 2 - Matisse
        assert!(matches!(CpuCodename::from_u32(4), CpuCodename::Matisse));

        // Test Zen 5 - Strix Point
        assert!(matches!(CpuCodename::from_u32(22), CpuCodename::StrixPoint));

        // Test unknown
        assert!(matches!(
            CpuCodename::from_u32(999),
            CpuCodename::Unknown(999)
        ));
    }

    #[test]
    fn test_codename_properties() {
        // Test desktop
        assert!(CpuCodename::GraniteRidge.is_desktop());
        assert!(CpuCodename::Raphael.is_desktop());
        assert!(CpuCodename::Vermeer.is_desktop());

        // Test mobile
        assert!(CpuCodename::StrixPoint.is_mobile());
        assert!(CpuCodename::Phoenix.is_mobile());
        assert!(CpuCodename::Renoir.is_mobile());

        // Test HEDT
        assert!(CpuCodename::StormPeak.is_hedt());
        assert!(CpuCodename::CastlePeak.is_hedt());

        // Test generation
        assert_eq!(CpuCodename::GraniteRidge.generation(), "Zen 5");
        assert_eq!(CpuCodename::Raphael.generation(), "Zen 4");
        assert_eq!(CpuCodename::Vermeer.generation(), "Zen 3");
        assert_eq!(CpuCodename::Matisse.generation(), "Zen 2");
    }

    #[test]
    fn test_pm_table_data_reads() {
        let data = vec![
            0x00, 0x00, 0x80, 0x3F, // f32: 1.0
            0x00, 0x00, 0x00, 0x40, // f32: 2.0
            0x0A, 0x00, 0x00, 0x00, // u32: 10
        ];

        let pm_table = PmTableData {
            version: 0x240903,
            data,
        };

        assert_eq!(pm_table.read_f32(0), Some(1.0));
        assert_eq!(pm_table.read_f32(4), Some(2.0));
        assert_eq!(pm_table.read_u32(8), Some(10));
        assert_eq!(pm_table.read_f32(100), None); // Out of bounds
    }
}
