//! ryzen_smu kernel driver sysfs interface
//!
//! Reads CPU telemetry from the ryzen_smu kernel module's sysfs files.

use super::types::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::fs;
use std::io::Cursor;
use std::path::Path;

/// Check if the ryzen_smu driver is loaded and accessible
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

    // Try to read to verify permissions
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

/// Read complete SMU information (checks driver first)
pub fn read_info() -> Result<SmuInfo, SmuError> {
    check_driver()?;
    read_info_unchecked()
}

/// Read SMU information without checking driver (for internal use after check)
pub fn read_info_unchecked() -> Result<SmuInfo, SmuError> {
    let version_str = read_sysfs_string("version")?;
    let version: SmuVersion = version_str.parse().map_err(|e: String| SmuError::ParseError {
        path: format!("{}/version", SMU_DRV_PATH),
        reason: e,
    })?;

    let codename_str = read_sysfs_string("codename")?;
    let codename_val =
        codename_str
            .trim()
            .parse::<u32>()
            .map_err(|e| SmuError::ParseError {
                path: format!("{}/codename", SMU_DRV_PATH),
                reason: format!("Failed to parse codename: {}", e),
            })?;
    let codename = CpuCodename::from_u32(codename_val);

    let drv_version = read_sysfs_string("drv_version")?;
    let pm_table_version = read_sysfs_u64("pm_table_version")? as u32;
    let pm_table_size = read_sysfs_u64("pm_table_size")?;
    let mp1_if_version = read_sysfs_u32_optional("mp1_if_version");

    Ok(SmuInfo {
        version,
        codename,
        drv_version,
        pm_table_version,
        pm_table_size,
        mp1_if_version,
    })
}

/// Read PM table data from the driver
pub fn read_pm_table(force: bool) -> Result<PmTableData, SmuError> {
    check_driver()?;

    let info = read_info_unchecked()?;
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

    if !force && !is_version_supported(info.pm_table_version) {
        return Err(SmuError::UnsupportedPmTableVersion {
            version: info.pm_table_version,
        });
    }

    Ok(PmTableData {
        version: info.pm_table_version,
        data,
    })
}

/// Check if PM table version is known/supported
fn is_version_supported(version: u32) -> bool {
    matches!(
        version,
        // Zen 2/3
        0x240903 | 0x240802 | 0x240803 |
        // Zen 4 (Raphael)
        0x480804 | 0x480805 | 0x480904 |
        // Zen 5 (Granite Ridge)
        0x620105 | 0x620205 | 0x621101 | 0x621102 | 0x621201 | 0x621202
    )
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

    if bytes.len() >= 8 {
        let mut cursor = Cursor::new(&bytes[..8]);
        cursor
            .read_u64::<LittleEndian>()
            .map_err(|e| SmuError::ParseError {
                path,
                reason: format!("Failed to parse u64: {}", e),
            })
    } else if bytes.len() >= 4 {
        let mut cursor = Cursor::new(&bytes[..4]);
        cursor
            .read_u32::<LittleEndian>()
            .map(|v| v as u64)
            .map_err(|e| SmuError::ParseError {
                path,
                reason: format!("Failed to parse u32: {}", e),
            })
    } else {
        Err(SmuError::ParseError {
            path,
            reason: format!("File too small: {} bytes", bytes.len()),
        })
    }
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

/// List all files in the driver sysfs directory (for debug)
pub fn list_sysfs_files() -> Result<Vec<(String, SysfsFileInfo)>, SmuError> {
    let smu_path = Path::new(SMU_DRV_PATH);

    if !smu_path.exists() {
        return Err(SmuError::DriverNotFound {
            path: SMU_DRV_PATH.to_string(),
        });
    }

    let entries = fs::read_dir(smu_path).map_err(|e| SmuError::ReadError {
        path: SMU_DRV_PATH.to_string(),
        source: e,
    })?;

    let mut files: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    files.sort_by_key(|e| e.file_name());

    let mut result = Vec::new();
    for entry in files {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if !path.is_file() {
            continue;
        }

        let info = match fs::read(&path) {
            Ok(data) => {
                if let Ok(text) = String::from_utf8(data.clone()) {
                    let trimmed = text.trim().to_string();
                    if trimmed.len() < 100 && !trimmed.contains('\0') {
                        SysfsFileInfo::Text(trimmed)
                    } else {
                        SysfsFileInfo::Binary(data)
                    }
                } else {
                    SysfsFileInfo::Binary(data)
                }
            }
            Err(e) => SysfsFileInfo::Error(e.to_string()),
        };

        result.push((name, info));
    }

    Ok(result)
}

/// Information about a sysfs file
#[derive(Debug)]
pub enum SysfsFileInfo {
    Text(String),
    Binary(Vec<u8>),
    Error(String),
}

/// Decode a known binary sysfs value into a human-readable string
pub fn decode_binary_value(name: &str, data: &[u8]) -> Option<String> {
    match name {
        "pm_table_version" if data.len() >= 4 => {
            let mut c = Cursor::new(&data[..4]);
            let v = c.read_u32::<LittleEndian>().ok()?;
            Some(format!("0x{:06X}", v))
        }
        "pm_table_size" => {
            if data.len() >= 8 {
                let mut c = Cursor::new(&data[..8]);
                let v = c.read_u64::<LittleEndian>().ok()?;
                Some(format!("{} bytes", v))
            } else if data.len() >= 4 {
                let mut c = Cursor::new(&data[..4]);
                let v = c.read_u32::<LittleEndian>().ok()?;
                Some(format!("{} bytes", v))
            } else {
                None
            }
        }
        "mp1_if_version" if data.len() >= 4 => {
            let mut c = Cursor::new(&data[..4]);
            let v = c.read_u32::<LittleEndian>().ok()?;
            Some(format!("{}", v))
        }
        // SMU command response codes
        "rsmu_cmd" | "mp1_smu_cmd" | "hsmp_smu_cmd" if data.len() >= 4 => {
            let mut c = Cursor::new(&data[..4]);
            let v = c.read_u32::<LittleEndian>().ok()?;
            let status = match v {
                0x00 => "Failed",
                0x01 => "OK",
                0x02 => "UnknownCmd",
                0x03 => "CmdRejectedPrereq",
                0x04 => "CmdRejectedBusy",
                0xFE => "CommandTimeout",
                0xFF => "CmdCompletedPartially",
                _ => return Some(format!("0x{:02X} (unknown)", v)),
            };
            Some(format!("0x{:02X} ({})", v, status))
        }
        "smn" if data.len() >= 4 => {
            let mut c = Cursor::new(&data[..4]);
            let v = c.read_u32::<LittleEndian>().ok()?;
            if v == 0xFFFFFFFF {
                Some("0xFFFFFFFF (no address set)".to_string())
            } else {
                Some(format!("0x{:08X}", v))
            }
        }
        "smu_args" if data.len() >= 4 => {
            let mut args = Vec::new();
            let count = data.len() / 4;
            for i in 0..count {
                let off = i * 4;
                if off + 4 <= data.len() {
                    let mut c = Cursor::new(&data[off..off + 4]);
                    if let Ok(v) = c.read_u32::<LittleEndian>() {
                        args.push(format!("0x{:08X}", v));
                    }
                }
            }
            Some(format!("[{}]", args.join(", ")))
        }
        _ => None,
    }
}

/// Read CPU model name from /proc/cpuinfo (Linux only)
pub fn read_cpu_model() -> Option<String> {
    let content = fs::read_to_string("/proc/cpuinfo").ok()?;
    for line in content.lines() {
        if line.starts_with("model name") {
            return line.split(':').nth(1).map(|s| s.trim().to_string());
        }
    }
    None
}

/// Read CPU topology info from /proc/cpuinfo
pub fn read_cpu_topology() -> Option<CpuTopology> {
    let content = fs::read_to_string("/proc/cpuinfo").ok()?;
    let mut physical_ids = std::collections::HashSet::new();
    let mut core_ids = std::collections::HashSet::new();
    let mut logical_count = 0u32;

    for line in content.lines() {
        if line.starts_with("processor") {
            logical_count += 1;
        } else if line.starts_with("physical id")
            && let Some(val) = line.split(':').nth(1)
        {
            physical_ids.insert(val.trim().to_string());
        } else if line.starts_with("core id")
            && let Some(val) = line.split(':').nth(1)
        {
            core_ids.insert(val.trim().to_string());
        }
    }

    if logical_count == 0 {
        return None;
    }

    let physical_cores = core_ids.len() as u32;
    let smt = logical_count > physical_cores;

    Some(CpuTopology {
        logical_cpus: logical_count,
        physical_cores,
        sockets: physical_ids.len().max(1) as u32,
        smt,
    })
}

/// CPU topology information
#[derive(Debug, Clone)]
pub struct CpuTopology {
    pub logical_cpus: u32,
    pub physical_cores: u32,
    pub sockets: u32,
    pub smt: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // is_version_supported
    // =========================================================================

    #[test]
    fn test_version_supported_zen2() {
        assert!(is_version_supported(0x240903));
        assert!(is_version_supported(0x240802));
        assert!(is_version_supported(0x240803));
    }

    #[test]
    fn test_version_supported_zen4() {
        assert!(is_version_supported(0x480804));
        assert!(is_version_supported(0x480805));
        assert!(is_version_supported(0x480904));
    }

    #[test]
    fn test_version_supported_zen5() {
        assert!(is_version_supported(0x620105));
        assert!(is_version_supported(0x620205));
        assert!(is_version_supported(0x621101));
        assert!(is_version_supported(0x621102));
        assert!(is_version_supported(0x621201));
        assert!(is_version_supported(0x621202));
    }

    #[test]
    fn test_version_not_supported() {
        assert!(!is_version_supported(0x000000));
        assert!(!is_version_supported(0x999999));
        assert!(!is_version_supported(0x240901));
        assert!(!is_version_supported(0x480800)); // close but not valid
        assert!(!is_version_supported(0x620100));
        assert!(!is_version_supported(u32::MAX));
    }

    // =========================================================================
    // SysfsFileInfo enum
    // =========================================================================

    #[test]
    fn test_sysfs_file_info_text() {
        let info = SysfsFileInfo::Text("hello".to_string());
        match info {
            SysfsFileInfo::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_sysfs_file_info_binary() {
        let info = SysfsFileInfo::Binary(vec![1, 2, 3]);
        match info {
            SysfsFileInfo::Binary(data) => assert_eq!(data, vec![1, 2, 3]),
            _ => panic!("expected Binary"),
        }
    }

    #[test]
    fn test_sysfs_file_info_error() {
        let info = SysfsFileInfo::Error("permission denied".to_string());
        match info {
            SysfsFileInfo::Error(msg) => assert!(msg.contains("permission")),
            _ => panic!("expected Error"),
        }
    }

    // =========================================================================
    // check_driver / read_info — no-hardware tests
    // =========================================================================

    #[test]
    fn test_check_driver_not_found() {
        // On non-Linux or without ryzen_smu, the driver path won't exist
        // This test verifies we get the right error type
        if !std::path::Path::new(SMU_DRV_PATH).exists() {
            let err = check_driver().unwrap_err();
            match err {
                SmuError::DriverNotFound { path } => {
                    assert!(path.contains("ryzen_smu"));
                }
                _ => panic!("expected DriverNotFound, got: {:?}", err),
            }
        }
    }

    #[test]
    fn test_read_info_without_driver() {
        if !std::path::Path::new(SMU_DRV_PATH).exists() {
            assert!(read_info().is_err());
        }
    }

    #[test]
    fn test_read_pm_table_without_driver() {
        if !std::path::Path::new(SMU_DRV_PATH).exists() {
            assert!(read_pm_table(false).is_err());
            assert!(read_pm_table(true).is_err());
        }
    }

    #[test]
    fn test_list_sysfs_files_without_driver() {
        if !std::path::Path::new(SMU_DRV_PATH).exists() {
            let err = list_sysfs_files().unwrap_err();
            matches!(err, SmuError::DriverNotFound { .. });
        }
    }

    // =========================================================================
    // read_sysfs_* helpers
    // =========================================================================

    #[test]
    fn test_read_sysfs_string_nonexistent() {
        let err = read_sysfs_string("nonexistent_file_xyzzy").unwrap_err();
        match err {
            SmuError::ReadError { path, .. } => {
                assert!(path.contains("nonexistent_file_xyzzy"));
            }
            _ => panic!("expected ReadError"),
        }
    }

    #[test]
    fn test_read_sysfs_u64_nonexistent() {
        let err = read_sysfs_u64("nonexistent_file_xyzzy").unwrap_err();
        match err {
            SmuError::ReadError { path, .. } => {
                assert!(path.contains("nonexistent_file_xyzzy"));
            }
            _ => panic!("expected ReadError"),
        }
    }

    #[test]
    fn test_read_sysfs_u32_optional_nonexistent() {
        assert_eq!(read_sysfs_u32_optional("nonexistent_file_xyzzy"), None);
    }
}
