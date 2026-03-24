//! MSR (Model-Specific Register) reading for AMD RAPL power monitoring
//!
//! Reads energy counters from `/dev/cpu/N/msr` to compute power consumption.
//! Requires root access. Works without the ryzen_smu kernel module.

use super::types::SmuError;
use std::time::Instant;

#[cfg(target_os = "linux")]
use std::os::unix::fs::FileExt;

// AMD RAPL MSR addresses
const MSR_AMD_RAPL_POWER_UNIT: u64 = 0xc0010299;
const MSR_AMD_PKG_ENERGY_STATUS: u64 = 0xc001029b;
const MSR_AMD_PP0_ENERGY_STATUS: u64 = 0xc001029a;

// RAPL energy unit is in bits 8-12
const RAPL_ENERGY_UNIT_MASK: u64 = 0x1f00;
const RAPL_ENERGY_UNIT_SHIFT: u32 = 8;

/// Read a raw MSR value from a specific CPU
#[cfg(target_os = "linux")]
fn read_msr(cpu: u32, msr: u64) -> Result<u64, SmuError> {
    let path = format!("/dev/cpu/{}/msr", cpu);
    let file = std::fs::File::open(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            SmuError::MsrError {
                cpu,
                msr,
                reason: "Permission denied. Run with sudo.".to_string(),
            }
        } else if e.kind() == std::io::ErrorKind::NotFound {
            SmuError::MsrError {
                cpu,
                msr,
                reason: "MSR device not found. Load the 'msr' kernel module: sudo modprobe msr"
                    .to_string(),
            }
        } else {
            SmuError::MsrError {
                cpu,
                msr,
                reason: e.to_string(),
            }
        }
    })?;

    let mut buf = [0u8; 8];
    file.read_at(&mut buf, msr).map_err(|e| SmuError::MsrError {
        cpu,
        msr,
        reason: e.to_string(),
    })?;

    Ok(u64::from_le_bytes(buf))
}

#[cfg(not(target_os = "linux"))]
fn read_msr(_cpu: u32, _msr: u64) -> Result<u64, SmuError> {
    Err(SmuError::MsrError {
        cpu: _cpu,
        msr: _msr,
        reason: "MSR reading is only supported on Linux".to_string(),
    })
}

/// RAPL power reader that tracks energy counter state between reads
#[derive(Debug)]
pub struct RaplReader {
    /// Energy unit in microjoules per counter tick
    energy_unit_uj: f64,
    /// Previous package energy reading
    prev_pkg_energy: Option<u64>,
    /// Previous core energy reading
    prev_core_energy: Option<u64>,
    /// Timestamp of previous reading
    prev_time: Option<Instant>,
}

impl RaplReader {
    /// Create a new RAPL reader, reading the energy unit from CPU 0
    pub fn new() -> Result<Self, SmuError> {
        let raw = read_msr(0, MSR_AMD_RAPL_POWER_UNIT)?;
        let energy_unit_exp = ((raw & RAPL_ENERGY_UNIT_MASK) >> RAPL_UNIT_SHIFT) as u32;
        // Energy unit = 1 / 2^exp joules, convert to microjoules
        let energy_unit_uj = 1_000_000.0 / (1u64 << energy_unit_exp) as f64;

        Ok(Self {
            energy_unit_uj,
            prev_pkg_energy: None,
            prev_core_energy: None,
            prev_time: None,
        })
    }

    /// Get the energy unit in microjoules per tick
    pub fn energy_unit_uj(&self) -> f64 {
        self.energy_unit_uj
    }

    /// Read package power in watts. Returns None on first call (needs two readings).
    pub fn read_package_power(&mut self) -> Result<Option<f64>, SmuError> {
        let now = Instant::now();
        let energy_raw = read_msr(0, MSR_AMD_PKG_ENERGY_STATUS)?;
        // Counter is 32-bit
        let energy = energy_raw as u32 as u64;

        let power = if let (Some(prev_energy), Some(prev_time)) =
            (self.prev_pkg_energy, self.prev_time)
        {
            let elapsed_ms = prev_time.elapsed().as_millis() as f64;
            if elapsed_ms > 0.0 {
                let delta = energy_delta_u32(prev_energy, energy);
                let energy_uj = delta as f64 * self.energy_unit_uj;
                // Power (W) = energy (uJ) / time (ms) / 1000
                Some(energy_uj / elapsed_ms / 1000.0)
            } else {
                None
            }
        } else {
            None
        };

        self.prev_pkg_energy = Some(energy);
        self.prev_time = Some(now);

        Ok(power)
    }

    /// Read core power in watts. Returns None on first call or if unavailable.
    /// Note: Core power is NOT available on Granite Ridge (Zen 5 desktop).
    pub fn read_core_power(&mut self) -> Result<Option<f64>, SmuError> {
        let _now = Instant::now();
        let energy_raw = match read_msr(0, MSR_AMD_PP0_ENERGY_STATUS) {
            Ok(v) => v,
            Err(_) => return Ok(None), // Core RAPL not available
        };
        let energy = energy_raw as u32 as u64;

        let power = if let (Some(prev_energy), Some(prev_time)) =
            (self.prev_core_energy, self.prev_time)
        {
            let elapsed_ms = prev_time.elapsed().as_millis() as f64;
            if elapsed_ms > 0.0 {
                let delta = energy_delta_u32(prev_energy, energy);
                let energy_uj = delta as f64 * self.energy_unit_uj;
                Some(energy_uj / elapsed_ms / 1000.0)
            } else {
                None
            }
        } else {
            None
        };

        self.prev_core_energy = Some(energy);
        // Don't update prev_time here — it's shared with package

        Ok(power)
    }

    /// Check if MSR-based RAPL is available on this system
    pub fn is_available() -> bool {
        read_msr(0, MSR_AMD_RAPL_POWER_UNIT).is_ok()
    }
}

// Workaround for a typo - use the correct constant name
const RAPL_UNIT_SHIFT: u32 = RAPL_ENERGY_UNIT_SHIFT;

/// Calculate energy counter delta handling 32-bit wraparound
fn energy_delta_u32(prev: u64, current: u64) -> u64 {
    if current >= prev {
        current - prev
    } else {
        // 32-bit counter wrapped around
        (0x1_0000_0000u64 - prev) + current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // energy_delta_u32
    // =========================================================================

    #[test]
    fn test_energy_delta_no_wrap() {
        assert_eq!(energy_delta_u32(100, 200), 100);
        assert_eq!(energy_delta_u32(0, 1000), 1000);
    }

    #[test]
    fn test_energy_delta_same_value() {
        assert_eq!(energy_delta_u32(500, 500), 0);
        assert_eq!(energy_delta_u32(0, 0), 0);
        assert_eq!(energy_delta_u32(0xFFFF_FFFF, 0xFFFF_FFFF), 0);
    }

    #[test]
    fn test_energy_delta_single_tick() {
        assert_eq!(energy_delta_u32(0, 1), 1);
        assert_eq!(energy_delta_u32(999, 1000), 1);
    }

    #[test]
    fn test_energy_delta_large_no_wrap() {
        assert_eq!(energy_delta_u32(0, 0xFFFF_FFFF), 0xFFFF_FFFF);
        assert_eq!(energy_delta_u32(1, 0xFFFF_FFFF), 0xFFFF_FFFE);
    }

    #[test]
    fn test_energy_delta_wrap_basic() {
        // Counter wraps from near-max to near-zero
        assert_eq!(
            energy_delta_u32(0xFFFF_FFFE, 2),
            4 // 2 ticks to wrap + 2 after
        );
        assert_eq!(
            energy_delta_u32(0xFFFF_FFFF, 0),
            1 // exactly one tick wrap
        );
    }

    #[test]
    fn test_energy_delta_wrap_midpoint() {
        // Wrap from 0x80000000 down (prev > current but both large)
        assert_eq!(
            energy_delta_u32(0xFFFF_FF00, 0x100),
            0x200 // 0x100 to wrap + 0x100 after
        );
    }

    #[test]
    fn test_energy_delta_wrap_max_to_zero() {
        assert_eq!(energy_delta_u32(0xFFFF_FFFF, 0), 1);
    }

    #[test]
    fn test_energy_delta_wrap_max_to_max_minus_1() {
        // This is "almost full loop": prev=0xFFFFFFFE, current=0xFFFFFFFD
        // Since current < prev, this is a wrap
        // delta = (0x100000000 - 0xFFFFFFFE) + 0xFFFFFFFD = 2 + 0xFFFFFFFD = 0xFFFFFFFF
        assert_eq!(
            energy_delta_u32(0xFFFF_FFFE, 0xFFFF_FFFD),
            0xFFFF_FFFF
        );
    }

    // =========================================================================
    // MSR Constants
    // =========================================================================

    #[test]
    fn test_msr_constants() {
        // AMD RAPL MSR addresses (documented in AMD PPR)
        assert_eq!(MSR_AMD_RAPL_POWER_UNIT, 0xc0010299);
        assert_eq!(MSR_AMD_PKG_ENERGY_STATUS, 0xc001029b);
        assert_eq!(MSR_AMD_PP0_ENERGY_STATUS, 0xc001029a);
    }

    #[test]
    fn test_rapl_mask_shift() {
        // Energy unit is bits 12:8 (5 bits)
        assert_eq!(RAPL_ENERGY_UNIT_MASK, 0x1f00);
        assert_eq!(RAPL_ENERGY_UNIT_SHIFT, 8);

        // Verify mask extracts correct bits
        let test_val: u64 = 0x0000_0E00; // bits 11:9 set = 0b01110 = 14
        let extracted = (test_val & RAPL_ENERGY_UNIT_MASK) >> RAPL_ENERGY_UNIT_SHIFT;
        assert_eq!(extracted, 14);
    }

    #[test]
    fn test_rapl_energy_unit_calculation() {
        // Typical AMD value: energy_unit = 16 (means 1/2^16 J = ~15.26 uJ per tick)
        let raw: u64 = 16 << RAPL_ENERGY_UNIT_SHIFT;
        let exp = ((raw & RAPL_ENERGY_UNIT_MASK) >> RAPL_UNIT_SHIFT) as u32;
        assert_eq!(exp, 16);
        let energy_unit_uj = 1_000_000.0 / (1u64 << exp) as f64;
        assert!((energy_unit_uj - 15.2587890625).abs() < 0.001);
    }

    #[test]
    fn test_rapl_energy_unit_edge_cases() {
        // energy_unit = 0 -> 1/2^0 = 1J = 1000000 uJ
        let energy_unit_uj_0 = 1_000_000.0 / (1u64 << 0) as f64;
        assert!((energy_unit_uj_0 - 1_000_000.0).abs() < 0.01);

        // energy_unit = 31 (max 5-bit) -> very small unit
        let energy_unit_uj_31 = 1_000_000.0 / (1u64 << 31) as f64;
        assert!(energy_unit_uj_31 > 0.0);
        assert!(energy_unit_uj_31 < 0.001);
    }

    // =========================================================================
    // read_msr — platform behavior
    // =========================================================================

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn test_read_msr_non_linux() {
        let result = read_msr(0, MSR_AMD_RAPL_POWER_UNIT);
        assert!(result.is_err());
        match result.unwrap_err() {
            SmuError::MsrError { reason, .. } => {
                assert!(reason.contains("only supported on Linux"));
            }
            e => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_rapl_reader_is_available_non_linux() {
        // On macOS this should return false
        #[cfg(not(target_os = "linux"))]
        assert!(!RaplReader::is_available());
    }
}
