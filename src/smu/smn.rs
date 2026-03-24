//! SMN (System Management Network) register reading via PCI config space
//!
//! Reads temperature and voltage registers directly from AMD SMN.
//! Uses PCI config space registers 0x60/0x64 on the host bridge.
//! Requires root access. Works without the ryzen_smu kernel module.

use super::types::SmuError;

#[cfg(target_os = "linux")]
use std::os::unix::fs::FileExt;

// Temperature registers
const REPORTED_TEMP_CTRL: u32 = 0x00059800;
// CCD temperature base for Zen 5 (Granite Ridge)
const ZEN5_CCD_TEMP_BASE: u32 = 0x00059b08;
// CCD temperature base for older Zen (2/3/4)
const LEGACY_CCD_TEMP_BASE: u32 = 0x00059954;

// Temperature constants
const TEMP_ADJUST_MASK: u32 = 0x80000;
const CCD_TEMP_VALID_BIT: u32 = 1 << 11;
const CCD_TEMP_MASK: u32 = 0x7ff;
const TEMP_SCALE_FACTOR: i32 = 125; // millidegrees per LSB
const TEMP_OFFSET: i32 = 49000; // millidegree offset

// SVI telemetry registers (Zen 5 / Granite Ridge)
const ZEN5_SVI_CORE_ADDR: u32 = 0x00073010;
const ZEN5_SVI_SOC_ADDR: u32 = 0x00073014;

/// Read a 32-bit value from an SMN register via PCI config space
#[cfg(target_os = "linux")]
fn read_smn_register(pci_device: &str, address: u32) -> Result<u32, SmuError> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pci_device)
        .map_err(|e| SmuError::SmnError {
            address,
            reason: format!("Failed to open {}: {}", pci_device, e),
        })?;

    // Write SMN address to index register (0x60)
    let addr_bytes = address.to_le_bytes();
    file.write_at(&addr_bytes, 0x60)
        .map_err(|e| SmuError::SmnError {
            address,
            reason: format!("Failed to write SMN index: {}", e),
        })?;

    // Read result from data register (0x64)
    let mut buf = [0u8; 4];
    file.read_at(&mut buf, 0x64)
        .map_err(|e| SmuError::SmnError {
            address,
            reason: format!("Failed to read SMN data: {}", e),
        })?;

    Ok(u32::from_le_bytes(buf))
}

#[cfg(not(target_os = "linux"))]
fn read_smn_register(_pci_device: &str, address: u32) -> Result<u32, SmuError> {
    Err(SmuError::SmnError {
        address,
        reason: "SMN reading is only supported on Linux".to_string(),
    })
}

/// SMN register reader bound to a specific PCI host bridge
pub struct SmnReader {
    pci_device: String,
    is_zen5: bool,
}

impl SmnReader {
    /// Create a reader for the default host bridge (node 0)
    pub fn new(is_zen5: bool) -> Self {
        Self {
            pci_device: "/sys/bus/pci/devices/0000:00:00.0/config".to_string(),
            is_zen5,
        }
    }

    /// Create a reader for a specific PCI device path
    pub fn with_device(pci_device: String, is_zen5: bool) -> Self {
        Self {
            pci_device,
            is_zen5,
        }
    }

    /// Read a raw 32-bit SMN register at the given address
    pub fn read_register(&self, address: u32) -> Result<u32, SmuError> {
        read_smn_register(&self.pci_device, address)
    }

    /// Read Tctl (control temperature) in degrees Celsius
    pub fn read_tctl(&self) -> Result<f64, SmuError> {
        let raw = read_smn_register(&self.pci_device, REPORTED_TEMP_CTRL)?;

        let mut temp_millideg = ((raw >> 21) as i32) * TEMP_SCALE_FACTOR;
        if raw & TEMP_ADJUST_MASK != 0 {
            temp_millideg -= TEMP_OFFSET;
        }

        Ok(temp_millideg as f64 / 1000.0)
    }

    /// Read CCD temperature in degrees Celsius. Returns None if sensor is invalid.
    pub fn read_ccd_temp(&self, ccd_index: u32) -> Result<Option<f64>, SmuError> {
        let base = if self.is_zen5 {
            ZEN5_CCD_TEMP_BASE
        } else {
            LEGACY_CCD_TEMP_BASE
        };
        let addr = base + (ccd_index * 4);

        let raw = read_smn_register(&self.pci_device, addr)?;

        // Check validity bit
        if raw & CCD_TEMP_VALID_BIT == 0 {
            return Ok(None);
        }

        let temp_millideg = ((raw & CCD_TEMP_MASK) as i32) * TEMP_SCALE_FACTOR - TEMP_OFFSET;
        Ok(Some(temp_millideg as f64 / 1000.0))
    }

    /// Read all CCD temperatures. Probes up to max_ccds and stops at first invalid.
    pub fn read_all_ccd_temps(&self, max_ccds: u32) -> Result<Vec<Option<f64>>, SmuError> {
        let mut temps = Vec::new();
        for i in 0..max_ccds {
            match self.read_ccd_temp(i) {
                Ok(temp) => temps.push(temp),
                Err(_) => break,
            }
        }
        Ok(temps)
    }

    /// Read SVI core voltage (experimental on Zen 5 — uses SVI3 registers)
    pub fn read_core_voltage(&self) -> Result<Option<f64>, SmuError> {
        if !self.is_zen5 {
            return Ok(None); // Only Zen 5 addresses are currently mapped
        }
        let raw = read_smn_register(&self.pci_device, ZEN5_SVI_CORE_ADDR)?;
        Ok(decode_svi_voltage(raw))
    }

    /// Read SVI SoC voltage (experimental on Zen 5 — uses SVI3 registers)
    pub fn read_soc_voltage(&self) -> Result<Option<f64>, SmuError> {
        if !self.is_zen5 {
            return Ok(None);
        }
        let raw = read_smn_register(&self.pci_device, ZEN5_SVI_SOC_ADDR)?;
        Ok(decode_svi_voltage(raw))
    }

    /// Check if SMN access is available on this system
    pub fn is_available(&self) -> bool {
        read_smn_register(&self.pci_device, REPORTED_TEMP_CTRL).is_ok()
    }
}

/// Decode SVI2/SVI3 voltage telemetry register value
/// Uses the Zen 2 calculation formula: V = 1.55 - (vid * 0.00625)
/// where vid is bits 7:0. Returns None if value looks invalid.
fn decode_svi_voltage(raw: u32) -> Option<f64> {
    let vid = (raw & 0xFF) as f64;
    let voltage = 1.55 - (vid * 0.00625);

    // Sanity check: voltage should be between 0.2V and 2.0V
    if voltage > 0.2 && voltage < 2.0 {
        Some(voltage)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // decode_svi_voltage
    // =========================================================================

    #[test]
    fn test_decode_svi_voltage_vid_0() {
        // VID 0 = 1.55V (maximum voltage)
        assert_eq!(decode_svi_voltage(0x00), Some(1.55));
    }

    #[test]
    fn test_decode_svi_voltage_typical_load() {
        // VID 100 = 1.55 - 0.625 = 0.925V
        let v = decode_svi_voltage(100).unwrap();
        assert!((v - 0.925).abs() < 0.001);
    }

    #[test]
    fn test_decode_svi_voltage_typical_idle() {
        // VID 80 = 1.55 - 0.5 = 1.05V (typical idle)
        let v = decode_svi_voltage(80).unwrap();
        assert!((v - 1.05).abs() < 0.001);
    }

    #[test]
    fn test_decode_svi_voltage_low() {
        // VID 200 = 1.55 - 1.25 = 0.3V (very low)
        let v = decode_svi_voltage(200).unwrap();
        assert!((v - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_decode_svi_voltage_boundary_low() {
        // VID 216 = 1.55 - 1.35 = 0.2V (at threshold)
        // 0.2 is NOT > 0.2, so this should be None
        assert_eq!(decode_svi_voltage(216), None);
    }

    #[test]
    fn test_decode_svi_voltage_below_threshold() {
        // VID 248 = 1.55 - 1.55 = 0.0V
        assert_eq!(decode_svi_voltage(248), None);
        // VID 255 = 1.55 - 1.59375 = negative
        assert_eq!(decode_svi_voltage(255), None);
    }

    #[test]
    fn test_decode_svi_voltage_upper_bits_ignored() {
        // Upper bits should be masked off (only lower 8 bits used)
        let v_no_upper = decode_svi_voltage(100);
        let v_with_upper = decode_svi_voltage(0xFF00_0064); // upper bits set, VID = 100
        assert_eq!(v_no_upper, v_with_upper);
    }

    #[test]
    fn test_decode_svi_voltage_all_valid_range() {
        // VID 0-215 should produce valid voltages (0.2-1.55V range approximately)
        let mut valid_count = 0;
        for vid in 0u32..=255 {
            if let Some(v) = decode_svi_voltage(vid) {
                assert!(v > 0.2, "VID {} produced too-low voltage {}", vid, v);
                assert!(v < 2.0, "VID {} produced too-high voltage {}", vid, v);
                valid_count += 1;
            }
        }
        assert!(valid_count > 200, "most VID values should produce valid voltages");
    }

    // =========================================================================
    // Temperature register addresses
    // =========================================================================

    #[test]
    fn test_ccd_temp_base_addresses_zen5() {
        assert_eq!(ZEN5_CCD_TEMP_BASE, 0x00059b08);
        assert_eq!(ZEN5_CCD_TEMP_BASE + 0 * 4, 0x00059b08); // CCD0
        assert_eq!(ZEN5_CCD_TEMP_BASE + 1 * 4, 0x00059b0c); // CCD1
        assert_eq!(ZEN5_CCD_TEMP_BASE + 7 * 4, 0x00059b24); // CCD7 (max for EPYC)
    }

    #[test]
    fn test_ccd_temp_base_addresses_legacy() {
        assert_eq!(LEGACY_CCD_TEMP_BASE, 0x00059954);
        assert_eq!(LEGACY_CCD_TEMP_BASE + 0 * 4, 0x00059954);
        assert_eq!(LEGACY_CCD_TEMP_BASE + 1 * 4, 0x00059958);
    }

    #[test]
    fn test_reported_temp_ctrl_address() {
        assert_eq!(REPORTED_TEMP_CTRL, 0x00059800);
    }

    // =========================================================================
    // Temperature constants
    // =========================================================================

    #[test]
    fn test_temp_scale_factor() {
        // 125 millidegrees per LSB
        assert_eq!(TEMP_SCALE_FACTOR, 125);
    }

    #[test]
    fn test_temp_offset() {
        // 49000 millidegrees = 49.0 C offset
        assert_eq!(TEMP_OFFSET, 49000);
    }

    #[test]
    fn test_ccd_temp_valid_bit() {
        assert_eq!(CCD_TEMP_VALID_BIT, 0x800); // bit 11
    }

    #[test]
    fn test_ccd_temp_mask() {
        assert_eq!(CCD_TEMP_MASK, 0x7ff); // 11 bits
    }

    #[test]
    fn test_temp_adjust_mask() {
        assert_eq!(TEMP_ADJUST_MASK, 0x80000); // bit 19
    }

    // =========================================================================
    // SVI register addresses
    // =========================================================================

    #[test]
    fn test_svi_register_addresses() {
        assert_eq!(ZEN5_SVI_CORE_ADDR, 0x00073010);
        assert_eq!(ZEN5_SVI_SOC_ADDR, 0x00073014);
        // SoC should be 4 bytes after core
        assert_eq!(ZEN5_SVI_SOC_ADDR - ZEN5_SVI_CORE_ADDR, 4);
    }

    // =========================================================================
    // SmnReader construction
    // =========================================================================

    #[test]
    fn test_smn_reader_new_zen5() {
        let reader = SmnReader::new(true);
        assert!(reader.is_zen5);
        assert!(reader.pci_device.contains("0000:00:00.0"));
    }

    #[test]
    fn test_smn_reader_new_legacy() {
        let reader = SmnReader::new(false);
        assert!(!reader.is_zen5);
    }

    #[test]
    fn test_smn_reader_with_device() {
        let reader = SmnReader::with_device("/sys/custom/path".to_string(), true);
        assert_eq!(reader.pci_device, "/sys/custom/path");
        assert!(reader.is_zen5);
    }

    // =========================================================================
    // Temperature calculation verification
    // =========================================================================

    #[test]
    fn test_tctl_formula_example() {
        // Simulate Tctl reading: raw register value where temp = 65.0C
        // Formula: (raw >> 21) * 125; if (raw & 0x80000) -= 49000
        // For 65.0C: temp_millideg = 65000
        // With adjust: 65000 + 49000 = 114000; val = 114000 / 125 = 912; raw = 912 << 21
        // Without adjust: 65000 / 125 = 520; raw = 520 << 21

        // Case: with adjust bit set
        let val_with_adjust = 912u32;
        let raw = (val_with_adjust << 21) | TEMP_ADJUST_MASK;
        let mut temp_millideg = ((raw >> 21) as i32) * TEMP_SCALE_FACTOR;
        if raw & TEMP_ADJUST_MASK != 0 {
            temp_millideg -= TEMP_OFFSET;
        }
        let temp_c = temp_millideg as f64 / 1000.0;
        assert!((temp_c - 65.0).abs() < 0.2, "expected ~65C, got {}", temp_c);
    }

    #[test]
    fn test_ccd_temp_formula_example() {
        // CCD temp: (val & 0x7ff) * 125 - 49000 millidegrees
        // For 55.0C: 55000 + 49000 = 104000 / 125 = 832
        let raw = CCD_TEMP_VALID_BIT | 832;
        let temp_millideg = ((raw & CCD_TEMP_MASK) as i32) * TEMP_SCALE_FACTOR - TEMP_OFFSET;
        let temp_c = temp_millideg as f64 / 1000.0;
        assert!((temp_c - 55.0).abs() < 0.2, "expected ~55C, got {}", temp_c);
    }

    #[test]
    fn test_ccd_temp_invalid_when_valid_bit_not_set() {
        let raw = 832u32; // no valid bit
        assert_eq!(raw & CCD_TEMP_VALID_BIT, 0, "should be invalid");
    }

    #[test]
    fn test_ccd_temp_valid_bit_set() {
        let raw = CCD_TEMP_VALID_BIT | 500;
        assert_ne!(raw & CCD_TEMP_VALID_BIT, 0, "should be valid");
    }

    // =========================================================================
    // read_smn_register — platform behavior
    // =========================================================================

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn test_read_smn_register_non_linux() {
        let result = read_smn_register("/dev/null", 0x59800);
        assert!(result.is_err());
        match result.unwrap_err() {
            SmuError::SmnError { reason, .. } => {
                assert!(reason.contains("only supported on Linux"));
            }
            e => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_smn_reader_not_available_nonexistent_device() {
        let reader = SmnReader::with_device("/nonexistent/device".to_string(), false);
        assert!(!reader.is_available());
    }
}
