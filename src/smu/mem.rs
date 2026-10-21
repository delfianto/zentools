//! AMD UMC (Unified Memory Controller) memory timing reader
//!
//! Reads DDR4/DDR5 memory timings from SMN registers via PCI config space.
//! Replicates ZenTimings functionality on Linux.
//! Source: irusanov/ZenTimings + irusanov/ZenStates-Core

use super::smn::SmnReader;
use super::types::SmuError;
use std::collections::BTreeMap;

// =============================================================================
// UMC Register Addresses (relative to channel base)
// Channel base = channel_index << 20
// =============================================================================

// DIMM detection
const DDR4_DIMM0_ADDR: u32 = 0x50000;
const DDR4_DIMM1_ADDR: u32 = 0x50008;
const DDR5_DIMM0_ADDR: u32 = 0x50020;
const DDR5_DIMM1_ADDR: u32 = 0x50028;

// Channel enabled
const UMC_CH_ENABLED: u32 = 0x50DF0;

// Timing registers
const UMC_CFG: u32 = 0x50200;
const UMC_TIM0: u32 = 0x50204;
const UMC_TIM1: u32 = 0x50208;
const UMC_TIM2: u32 = 0x5020C;
const UMC_TIM3: u32 = 0x50210;
const UMC_TIM4: u32 = 0x50214;
const UMC_TIM5: u32 = 0x50218;
const UMC_TIM6: u32 = 0x50220;
const UMC_TIM7: u32 = 0x50224;
const UMC_TIM8: u32 = 0x50228;
const UMC_REFI: u32 = 0x50230;
const UMC_RFC: u32 = 0x50260;

// Power management
const UMC_PWR_CFG: u32 = 0x5012C;

/// Additional timing registers with layouts shared by DDR4 and DDR5.
const EXTRA_TIMING_REGS: &[u32] = &[0x5021C, 0x50234, 0x50238, 0x50250, 0x50254, 0x50258];

/// Values captured during decoding, keyed by channel-relative SMN address.
/// None indicates a failed read, rather than a hardware value of zero.
pub type MemRegisters = BTreeMap<u32, Option<u32>>;

/// Maximum channels to probe (2 for desktop, 4 for HEDT, 8 for server)
const MAX_CHANNELS: u32 = 8;

/// Memory type detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemType {
    DDR4,
    DDR5,
    Unknown,
}

impl std::fmt::Display for MemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemType::DDR4 => write!(f, "DDR4"),
            MemType::DDR5 => write!(f, "DDR5"),
            MemType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Memory timing parameters for one channel
#[derive(Debug, Clone, Default)]
pub struct MemTimings {
    // Config
    pub mem_type: Option<MemType>,
    pub ratio: u32,
    pub frequency_mhz: f64,
    pub cmd2t: bool,
    pub gdm: bool,
    /// Configured power-down enable; this is not current residency.
    pub power_down: Option<bool>,

    // Primary timings
    pub tcl: u32,
    pub trcdrd: u32,
    pub trcdwr: u32,
    pub trp: u32,
    pub tras: u32,
    pub trc: u32,

    // Secondary timings
    pub trrds: u32,
    pub trrdl: u32,
    pub tfaw: u32,
    pub twtrs: u32,
    pub twtrl: u32,
    pub twr: u32,
    pub tcwl: u32,
    pub trtp: u32,

    // Tertiary timings
    pub trdrdscl: u32,
    pub twrwrscl: u32,
    pub trdrdsc: u32,
    pub trdrdsd: u32,
    pub trdrddd: u32,
    pub twrwrsc: u32,
    pub twrwrsd: u32,
    pub twrwrdd: u32,
    pub trdwr: u32,
    pub twrrd: u32,

    // Refresh
    pub trefi: u32,
    pub trfc: Option<u32>,
    pub trfc2: Option<u32>,
    pub trfc4: Option<u32>,
    pub trfcsb: Option<u32>,

    // Additional subtimings (None if the register could not be read)
    pub trcpage: Option<u32>,
    pub tmod: Option<u32>,
    pub tmodpda: Option<u32>,
    pub tmrd: Option<u32>,
    pub tmrdpda: Option<u32>,
    pub txs: Option<u32>,
    pub tdllk: Option<u32>,
    pub tstag: Option<u32>,
    pub tcke: Option<u32>,
    pub tcpded: Option<u32>,
    pub txp: Option<u32>,
    pub tphywrd: Option<u32>,
    pub tphyrdl: Option<u32>,
    pub tphywrl: Option<u32>,
    pub trddataen: Option<u32>,
    /// DDR5 Nitro RX data / TX data / control-line settings.
    pub nitro: Option<[u32; 3]>,
}

/// Information about a memory channel
#[derive(Debug, Clone)]
pub struct MemChannel {
    pub channel_id: u32,
    pub dimm0_present: bool,
    pub dimm1_present: bool,
    pub timings: MemTimings,
    pub registers: MemRegisters,
}

/// Complete memory configuration
#[derive(Debug, Clone)]
pub struct MemConfig {
    pub mem_type: MemType,
    pub channels: Vec<MemChannel>,
}

/// Read memory configuration from UMC registers via SMN.
/// `mem_type` should be determined from the CPU generation (Zen 4/5 = DDR5, older = DDR4).
pub fn read_mem_config(smn: &SmnReader, mem_type: MemType) -> Result<MemConfig, SmuError> {
    let mut channels = Vec::new();

    for ch in 0..MAX_CHANNELS {
        let base = ch << 20;

        // Check if channel is enabled (bit 19: 0=enabled, 1=disabled)
        let ch_reg = match smn.read_register(base | UMC_CH_ENABLED) {
            Ok(v) => v,
            Err(_) => break,
        };
        if (ch_reg >> 19) & 1 != 0 {
            continue;
        }

        // Detect DIMM presence using the correct registers for the memory type
        let (dimm0, dimm1) = detect_dimms(smn, base, mem_type);

        if !dimm0 && !dimm1 {
            continue;
        }

        let (timings, registers) =
            read_channel_timings(|addr| smn.read_register(base | addr), mem_type)?;

        channels.push(MemChannel {
            channel_id: ch,
            dimm0_present: dimm0,
            dimm1_present: dimm1,
            timings,
            registers,
        });
    }

    Ok(MemConfig { mem_type, channels })
}

/// Detect DIMM presence for a channel.
/// Tries both DDR4 and DDR5 register addresses — on some Zen 5 platforms,
/// the DDR4 addresses (0x50000/0x50008) report presence even for DDR5 DIMMs.
fn detect_dimms(smn: &SmnReader, base: u32, _mem_type: MemType) -> (bool, bool) {
    // Try DDR4 addresses first (work on all platforms including Zen 5 DDR5)
    let d4_0 = smn.read_register(base | DDR4_DIMM0_ADDR).unwrap_or(0);
    let d4_1 = smn.read_register(base | DDR4_DIMM1_ADDR).unwrap_or(0);

    if (d4_0 & 1) != 0 || (d4_1 & 1) != 0 {
        return ((d4_0 & 1) != 0, (d4_1 & 1) != 0);
    }

    // Fallback: try DDR5 addresses
    let d5_0 = smn.read_register(base | DDR5_DIMM0_ADDR).unwrap_or(0);
    let d5_1 = smn.read_register(base | DDR5_DIMM1_ADDR).unwrap_or(0);

    ((d5_0 & 1) != 0, (d5_1 & 1) != 0)
}

/// Read all timing registers for a single channel
fn read_channel_timings(
    mut read: impl FnMut(u32) -> Result<u32, SmuError>,
    mem_type: MemType,
) -> Result<(MemTimings, MemRegisters), SmuError> {
    let mut registers = MemRegisters::new();
    let mut read = |addr| {
        let result = read(addr).and_then(|value| {
            if value == u32::MAX {
                Err(SmuError::SmnError {
                    address: addr,
                    reason: "UMC register returned all ones".into(),
                })
            } else {
                Ok(value)
            }
        });
        registers.insert(addr, result.as_ref().ok().copied());
        result
    };
    let cfg = read(UMC_CFG)?;
    let tim0 = read(UMC_TIM0)?;
    let tim1 = read(UMC_TIM1)?;
    let tim2 = read(UMC_TIM2)?;
    let tim3 = read(UMC_TIM3)?;
    let tim4 = read(UMC_TIM4)?;
    let tim5 = read(UMC_TIM5)?;
    let tim6 = read(UMC_TIM6)?;
    let tim7 = read(UMC_TIM7)?;
    let tim8 = read(UMC_TIM8)?;
    let refi = read(UMC_REFI)?;
    let pwr = read(UMC_PWR_CFG).ok();

    // Refresh timings are banked by DIMM/rank. Skip unpopulated defaults.
    // DDR5 uses 16-bit RFC/RFC2 fields, not DDR4's packed 11/11/10 layout.
    let rfc_count = if mem_type == MemType::DDR5 { 4 } else { 2 };
    let default_rfc = if mem_type == MemType::DDR5 {
        0x00C00138
    } else {
        0x21060138
    };
    let rfc_values: Vec<_> = (0..rfc_count)
        .filter_map(|i| read(UMC_RFC + i * 4).ok())
        .filter(|&v| v != 0)
        .collect();
    let rfc = rfc_values
        .iter()
        .copied()
        .find(|&v| v != default_rfc)
        // DDR4's default can also be a valid timing; preserve it if both match.
        .or_else(|| {
            (mem_type == MemType::DDR4)
                .then(|| rfc_values.first().copied())
                .flatten()
        });
    let trfcsb = if mem_type == MemType::DDR5 {
        let values: Vec<_> = (0..4).filter_map(|i| read(0x502C0 + i * 4).ok()).collect();
        values.into_iter().map(|v| v & 0x7FF).find(|&v| v != 0)
    } else {
        None
    };
    for &addr in EXTRA_TIMING_REGS {
        let _ = read(addr);
    }
    if mem_type == MemType::DDR5 {
        let _ = read(0x50284);
    }
    let field = |addr, start, width| {
        registers
            .get(&addr)
            .copied()
            .flatten()
            .map(|v| extract_bits(v, start, width))
    };
    let ddr5 = mem_type == MemType::DDR5;

    // Frequency extraction differs between DDR4 and DDR5:
    // DDR4: bits [6:0] = ratio, freq = ratio / 3 * bclk * 2
    // DDR5 (Zen 4/5): bits [15:0] = MCLK in MHz directly, MT/s = MCLK * 2
    let (ratio, frequency_mhz) = match mem_type {
        MemType::DDR5 => {
            let mclk = cfg & 0xFFFF;
            (mclk, (mclk as f64) * 2.0)
        }
        _ => {
            let r = cfg & 0x7F;
            (r, (r as f64 / 3.0) * 200.0)
        }
    };

    // Cmd2T and GDM bit positions differ between DDR4 and DDR5
    let (cmd2t, gdm) = match mem_type {
        MemType::DDR5 => ((cfg >> 17) & 1 != 0, (cfg >> 18) & 1 != 0),
        _ => ((cfg >> 10) & 1 != 0, (cfg >> 11) & 1 != 0),
    };

    let timings = MemTimings {
        mem_type: Some(mem_type),
        ratio,
        frequency_mhz,
        cmd2t,
        gdm,
        power_down: pwr.map(|v| (v >> 28) & 1 != 0),

        // Primary
        tcl: tim0 & 0x3F,
        trcdrd: (tim0 >> 16) & 0x3F,
        trcdwr: (tim0 >> 24) & 0x3F,
        tras: (tim0 >> 8) & 0x7F,
        trc: tim1 & 0xFF,
        trp: (tim1 >> 16) & 0x3F,

        // Secondary
        trrds: tim2 & 0x1F,
        trrdl: (tim2 >> 8) & 0x1F,
        trtp: (tim2 >> 24) & 0x1F,
        tfaw: tim3 & if ddr5 { 0xFF } else { 0x7F },
        tcwl: tim4 & 0x3F,
        twtrs: (tim4 >> 8) & 0x1F,
        twtrl: (tim4 >> 16) & 0x7F,
        twr: tim5 & if ddr5 { 0xFF } else { 0x7F },

        // Tertiary
        trdrddd: tim6 & 0xF,
        trdrdsd: (tim6 >> 8) & 0xF,
        trdrdsc: (tim6 >> 16) & 0xF,
        trdrdscl: (tim6 >> 24) & if ddr5 { 0x3F } else { 0xF },
        twrwrdd: tim7 & 0xF,
        twrwrsd: (tim7 >> 8) & 0xF,
        twrwrsc: (tim7 >> 16) & 0xF,
        twrwrscl: (tim7 >> 24) & 0x3F,
        twrrd: tim8 & 0xF,
        trdwr: (tim8 >> 8) & if ddr5 { 0x3F } else { 0x1F },

        // Refresh
        trefi: refi & 0xFFFF,
        trfc: rfc.map(|v| v & if ddr5 { 0xFFFF } else { 0x7FF }),
        trfc2: rfc.map(|v| if ddr5 { v >> 16 } else { (v >> 11) & 0x7FF }),
        trfc4: if ddr5 {
            None
        } else {
            rfc.map(|v| (v >> 22) & 0x3FF)
        },
        trfcsb,
        trcpage: field(0x5021C, 20, 12),
        tmod: field(0x50234, 8, 6),
        tmodpda: field(0x50234, 24, 6),
        tmrd: field(0x50234, 0, 6),
        tmrdpda: field(0x50234, 16, 6),
        txs: field(0x50238, 0, if ddr5 { 12 } else { 11 }),
        tdllk: field(0x50238, 16, if ddr5 { 12 } else { 11 }),
        tstag: field(0x50250, 16, if ddr5 { 11 } else { 8 }),
        tcke: field(0x50254, 24, 5),
        tcpded: field(0x50254, 16, if ddr5 { 5 } else { 4 }),
        txp: field(0x50254, 0, 6),
        tphywrd: field(0x50258, 24, 3),
        tphyrdl: field(0x50258, 16, if ddr5 { 8 } else { 6 }),
        tphywrl: field(0x50258, 8, if ddr5 { 8 } else { 5 }),
        trddataen: field(0x50258, 0, 7),
        nitro: if ddr5 {
            registers
                .get(&0x50284)
                .copied()
                .flatten()
                .map(|v| [(v >> 8) & 3, (v >> 4) & 3, v & 3])
        } else {
            None
        },
    };
    Ok((timings, registers))
}

/// Extract a single bit field from a u32.
pub fn extract_bits(value: u32, start: u32, width: u32) -> u32 {
    (value >> start) & ((1 << width) - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_fixture(values: &[(u32, u32)], mem_type: MemType) -> (MemTimings, MemRegisters) {
        read_channel_timings(
            |addr| Ok(values.iter().find(|v| v.0 == addr).map_or(0, |v| v.1)),
            mem_type,
        )
        .unwrap()
    }

    #[test]
    fn decode_9950x_ddr5_snapshot() {
        // Captured on both UMC channels of a 9950X, DDR5-6000, 2026-10-21.
        let (t, raw) = decode_fixture(
            &[
                (0x50200, 0x80050BB8),
                (0x50204, 0x24244C1E),
                (0x50208, 0x00240070),
                (0x5020C, 0x17000F08),
                (0x50210, 0x20),
                (0x50214, 0x001E081C),
                (0x50218, 0x5A),
                (0x50220, 0x08010B0B),
                (0x50224, 0x17010B0B),
                (0x50228, 0x1508),
                (0x50230, 0x00C02D9D),
                (0x5012C, 0x15381140),
                (0x50234, 0x20202A2A),
                (0x50238, 0x07000392),
                (0x50250, 0x00070000),
                (0x50254, 0x000F0D17),
                (0x50258, 0x06241012),
                (0x50260, 0x00C00138),
                (0x50264, 0x00C00138),
                (0x50268, 0x01E00374),
                (0x5026C, 0x01E00374),
                (0x502C8, 0x005A0186),
                (0x502CC, 0x005A0186),
                (0x50284, 0x00000231),
            ],
            MemType::DDR5,
        );
        assert_eq!(t.frequency_mhz, 6000.0);
        assert_eq!((t.tcl, t.trcdrd, t.trp, t.tras), (30, 36, 36, 76));
        assert_eq!(
            (t.trfc, t.trfc2, t.trfcsb, t.trfc4),
            (Some(884), Some(480), Some(390), None)
        );
        assert_eq!((t.trdrdscl, t.twrwrscl), (8, 23));
        assert_eq!(
            (t.tmod, t.tmrdpda, t.txs, t.tdllk),
            (Some(42), Some(32), Some(914), Some(1792))
        );
        assert_eq!(
            (t.tphyrdl, t.tphywrl, t.tphywrd),
            (Some(36), Some(16), Some(6))
        );
        assert_eq!(t.power_down, Some(true));
        assert_eq!(t.nitro, Some([2, 3, 1]));
        // All refresh banks must be captured, even after a usable value is found.
        assert_eq!(raw.get(&0x502CC), Some(&Some(0x005A0186)));
        assert_eq!(raw.get(&0x50260), Some(&Some(0x00C00138)));
    }

    #[test]
    fn decode_ddr4_second_refresh_bank_and_reserved_bits() {
        let (t, raw) = decode_fixture(
            &[
                (UMC_CFG, 48 | (1 << 11)),
                (UMC_RFC, 0x21060138),
                (UMC_RFC + 4, 350 | (260 << 11) | (160 << 22)),
                (UMC_TIM3, 0x98),
                (UMC_TIM5, 0x8C),
                (UMC_TIM6, 0x34000000),
                (UMC_TIM8, 0x2C00),
            ],
            MemType::DDR4,
        );
        assert_eq!(
            (t.trfc, t.trfc2, t.trfc4),
            (Some(350), Some(260), Some(160))
        );
        assert_eq!(t.trfcsb, None);
        assert_eq!(t.nitro, None);
        assert_eq!((t.tfaw, t.twr, t.trdrdscl, t.trdwr), (24, 12, 4, 12));
        assert!(t.gdm);
        assert!(!raw.contains_key(&0x502C0));
    }

    #[test]
    fn unavailable_optional_registers_are_not_zero_or_disabled() {
        let (t, raw) = read_channel_timings(
            |addr| match addr {
                UMC_PWR_CFG | 0x50258 | 0x50284 => Err(SmuError::SmnError {
                    address: addr,
                    reason: "fixture read failure".into(),
                }),
                0x50260..=0x5026C => Ok(0x00C00138),
                0x502C0..=0x502CC => Ok(u32::MAX),
                _ => Ok(0),
            },
            MemType::DDR5,
        )
        .unwrap();
        assert_eq!((t.trfc, t.trfc2, t.trfcsb), (None, None, None));
        assert_eq!(t.power_down, None);
        assert_eq!(t.tphyrdl, None);
        assert_eq!(t.nitro, None);
        assert_eq!(raw.get(&0x50258), Some(&None));
        assert_eq!(raw.get(&0x502CC), Some(&None));
    }

    #[test]
    fn required_register_failure_aborts_decoding() {
        assert!(read_channel_timings(|_| Ok(u32::MAX), MemType::DDR5).is_err());
    }

    // =========================================================================
    // Register address constants
    // =========================================================================

    #[test]
    fn test_umc_register_addresses() {
        assert_eq!(UMC_CFG, 0x50200);
        assert_eq!(UMC_TIM0, 0x50204);
        assert_eq!(UMC_TIM1, 0x50208);
        assert_eq!(UMC_TIM2, 0x5020C);
        assert_eq!(UMC_TIM3, 0x50210);
        assert_eq!(UMC_TIM4, 0x50214);
        assert_eq!(UMC_TIM5, 0x50218);
        assert_eq!(UMC_RFC, 0x50260);
        assert_eq!(UMC_REFI, 0x50230);
    }

    #[test]
    fn test_dimm_detection_addresses() {
        assert_eq!(DDR4_DIMM0_ADDR, 0x50000);
        assert_eq!(DDR4_DIMM1_ADDR, 0x50008);
        assert_eq!(DDR5_DIMM0_ADDR, 0x50020);
        assert_eq!(DDR5_DIMM1_ADDR, 0x50028);
    }

    #[test]
    fn test_channel_offset_calculation() {
        assert_eq!(0u32 << 20, 0x000000);
        assert_eq!(1u32 << 20, 0x100000);
        assert_eq!(2u32 << 20, 0x200000);
        assert_eq!(7u32 << 20, 0x700000);
    }

    // =========================================================================
    // Bit extraction
    // =========================================================================

    #[test]
    fn test_extract_bits() {
        assert_eq!(extract_bits(0xFF, 0, 8), 0xFF);
        assert_eq!(extract_bits(0xFF00, 8, 8), 0xFF);
        assert_eq!(extract_bits(0b1010_0000, 5, 3), 0b101);
        assert_eq!(extract_bits(0xDEADBEEF, 0, 4), 0xF);
        assert_eq!(extract_bits(0xDEADBEEF, 4, 4), 0xE);
    }

    #[test]
    fn test_extract_bits_single() {
        assert_eq!(extract_bits(0b1000, 3, 1), 1);
        assert_eq!(extract_bits(0b0000, 3, 1), 0);
    }

    // =========================================================================
    // Timing register parsing (DDR4 register layout)
    // =========================================================================

    #[test]
    fn test_parse_tim0_cl_ras_rcd() {
        // CL=16, RAS=36, RCDRD=16, RCDWR=16
        let tim0: u32 = 16 | (36 << 8) | (16 << 16) | (16 << 24);
        assert_eq!(tim0 & 0x3F, 16); // CL
        assert_eq!((tim0 >> 8) & 0x7F, 36); // RAS
        assert_eq!((tim0 >> 16) & 0x3F, 16); // RCDRD
        assert_eq!((tim0 >> 24) & 0x3F, 16); // RCDWR
    }

    #[test]
    fn test_parse_tim1_rc_rp() {
        // RC=52, RP=16
        let tim1: u32 = 52 | (16 << 16);
        assert_eq!(tim1 & 0xFF, 52); // RC
        assert_eq!((tim1 >> 16) & 0x3F, 16); // RP
    }

    #[test]
    fn test_parse_tim2_rrds_rrdl_rtp() {
        // RRDS=4, RRDL=6, RTP=8
        let tim2: u32 = 4 | (6 << 8) | (8 << 24);
        assert_eq!(tim2 & 0x1F, 4);
        assert_eq!((tim2 >> 8) & 0x1F, 6);
        assert_eq!((tim2 >> 24) & 0x1F, 8);
    }

    #[test]
    fn test_parse_tim3_faw() {
        let tim3: u32 = 24;
        assert_eq!(tim3 & 0xFF, 24);
    }

    #[test]
    fn test_parse_tim4_cwl_wtrs_wtrl() {
        // CWL=14, WTRS=4, WTRL=8
        let tim4: u32 = 14 | (4 << 8) | (8 << 16);
        assert_eq!(tim4 & 0x3F, 14);
        assert_eq!((tim4 >> 8) & 0x1F, 4);
        assert_eq!((tim4 >> 16) & 0x7F, 8);
    }

    #[test]
    fn test_parse_tim5_wr() {
        let tim5: u32 = 12;
        assert_eq!(tim5 & 0xFF, 12);
    }

    #[test]
    fn test_parse_tim6_rdrd() {
        // RDRDDD=8, RDRDSD=4, RDRDSC=1, RDRDSCL=2
        let tim6: u32 = 8 | (4 << 8) | (1 << 16) | (2 << 24);
        assert_eq!(tim6 & 0xF, 8);
        assert_eq!((tim6 >> 8) & 0xF, 4);
        assert_eq!((tim6 >> 16) & 0xF, 1);
        assert_eq!((tim6 >> 24) & 0x3F, 2);
    }

    #[test]
    fn test_parse_tim7_wrwr() {
        // WRWRDD=8, WRWRSD=4, WRWRSC=1, WRWRSCL=2
        let tim7: u32 = 8 | (4 << 8) | (1 << 16) | (2 << 24);
        assert_eq!(tim7 & 0xF, 8);
        assert_eq!((tim7 >> 8) & 0xF, 4);
        assert_eq!((tim7 >> 16) & 0xF, 1);
        assert_eq!((tim7 >> 24) & 0x3F, 2);
    }

    #[test]
    fn test_parse_tim8_wrrd_rdwr() {
        // WRRD=3, RDWR=12
        let tim8: u32 = 3 | (12 << 8);
        assert_eq!(tim8 & 0xF, 3);
        assert_eq!((tim8 >> 8) & 0x3F, 12);
    }

    #[test]
    fn test_parse_refi() {
        let refi: u32 = 7800;
        assert_eq!(refi & 0xFFFF, 7800);
    }

    #[test]
    fn test_parse_rfc() {
        // RFC=350, RFC2=260, RFC4=160
        let rfc: u32 = 350 | (260 << 11) | (160 << 22);
        assert_eq!(rfc & 0x7FF, 350);
        assert_eq!((rfc >> 11) & 0x7FF, 260);
        assert_eq!((rfc >> 22) & 0x3FF, 160);
    }

    // =========================================================================
    // Config register (DDR4 vs DDR5)
    // =========================================================================

    #[test]
    fn test_parse_cfg_ddr4() {
        // Ratio=40, Cmd2T=1 (bit10), GDM=1 (bit11)
        let cfg: u32 = 40 | (1 << 10) | (1 << 11);
        assert_eq!(cfg & 0x7F, 40);
        assert!((cfg >> 10) & 1 != 0); // Cmd2T
        assert!((cfg >> 11) & 1 != 0); // GDM
    }

    #[test]
    fn test_parse_cfg_ddr5() {
        // Ratio=48, Cmd2T=1 (bit17), GDM=1 (bit18)
        let cfg: u32 = 48 | (1 << 17) | (1 << 18);
        assert_eq!(cfg & 0x7F, 48);
        assert!((cfg >> 17) & 1 != 0); // DDR5 Cmd2T
        assert!((cfg >> 18) & 1 != 0); // DDR5 GDM
    }

    // =========================================================================
    // Frequency calculation
    // =========================================================================

    #[test]
    fn test_frequency_ddr4_3200() {
        // DDR4-3200: ratio=48, freq = 48/3 * 100 * 2 = 3200
        let ratio = 48u32;
        let freq = (ratio as f64 / 3.0) * 200.0;
        assert!((freq - 3200.0).abs() < 1.0);
    }

    #[test]
    fn test_frequency_ddr4_3600() {
        // DDR4-3600: ratio=54, freq = 54/3 * 200 = 3600
        let ratio = 54u32;
        let freq = (ratio as f64 / 3.0) * 200.0;
        assert!((freq - 3600.0).abs() < 1.0);
    }

    #[test]
    fn test_frequency_ddr5_6000() {
        // DDR5-6000: MCLK=3000 in register bits [15:0], MT/s = 3000 * 2
        let mclk = 3000u32;
        let freq = (mclk as f64) * 2.0;
        assert!((freq - 6000.0).abs() < 1.0);
    }

    #[test]
    fn test_frequency_ddr5_6400() {
        // DDR5-6400: MCLK=3200 in register bits [15:0], MT/s = 3200 * 2
        let mclk = 3200u32;
        let freq = (mclk as f64) * 2.0;
        assert!((freq - 6400.0).abs() < 1.0);
    }

    #[test]
    fn test_frequency_ddr5_from_real_register() {
        // Real Zen 5 register: 0x80050BB8, bits [15:0] = 0x0BB8 = 3000
        let cfg: u32 = 0x80050BB8;
        let mclk = cfg & 0xFFFF;
        assert_eq!(mclk, 3000);
        let freq = (mclk as f64) * 2.0;
        assert!((freq - 6000.0).abs() < 1.0);
    }

    // =========================================================================
    // MemType
    // =========================================================================

    #[test]
    fn test_mem_type_display() {
        assert_eq!(MemType::DDR4.to_string(), "DDR4");
        assert_eq!(MemType::DDR5.to_string(), "DDR5");
        assert_eq!(MemType::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_mem_type_eq() {
        assert_eq!(MemType::DDR4, MemType::DDR4);
        assert_ne!(MemType::DDR4, MemType::DDR5);
    }

    // =========================================================================
    // MemTimings — realistic DDR4-3200 CL16 values
    // =========================================================================

    #[test]
    fn test_mem_timings_realistic_ddr4() {
        let t = MemTimings {
            mem_type: Some(MemType::DDR4),
            ratio: 48,
            frequency_mhz: 3200.0,
            cmd2t: false,
            gdm: true,
            power_down: Some(false),
            tcl: 16,
            trcdrd: 16,
            trcdwr: 16,
            trp: 16,
            tras: 36,
            trc: 52,
            trrds: 4,
            trrdl: 6,
            tfaw: 24,
            twtrs: 4,
            twtrl: 12,
            twr: 12,
            tcwl: 14,
            trtp: 8,
            trdrdscl: 4,
            twrwrscl: 4,
            trdrdsc: 1,
            trdrdsd: 5,
            trdrddd: 8,
            twrwrsc: 1,
            twrwrsd: 5,
            twrwrdd: 8,
            trdwr: 12,
            twrrd: 3,
            trefi: 7800,
            trfc: Some(350),
            trfc2: Some(260),
            trfc4: Some(160),
            ..Default::default()
        };

        assert_eq!(t.tcl, 16);
        assert_eq!(t.trcdrd, 16);
        assert_eq!(t.trp, 16);
        assert_eq!(t.tras, 36);
        assert!((t.frequency_mhz - 3200.0).abs() < 1.0);
        assert!(t.gdm);
        assert!(!t.cmd2t);
    }

    // =========================================================================
    // MemTimings — realistic DDR5-6000 CL30 values
    // =========================================================================

    #[test]
    fn test_mem_timings_realistic_ddr5() {
        let t = MemTimings {
            mem_type: Some(MemType::DDR5),
            ratio: 3000, // MCLK in MHz for DDR5
            frequency_mhz: 6000.0,
            cmd2t: true,
            gdm: false,
            power_down: Some(true),
            tcl: 30,
            trcdrd: 36,
            trcdwr: 36,
            trp: 36,
            tras: 72,
            trc: 108,
            trrds: 8,
            trrdl: 12,
            tfaw: 32,
            twtrs: 4,
            twtrl: 16,
            twr: 48,
            tcwl: 28,
            trtp: 12,
            trdrdscl: 4,
            twrwrscl: 4,
            trdrdsc: 1,
            trdrdsd: 5,
            trdrddd: 8,
            twrwrsc: 1,
            twrwrsd: 5,
            twrwrdd: 8,
            trdwr: 16,
            twrrd: 4,
            trefi: 3900,
            trfc: Some(880),
            trfc2: Some(660),
            trfc4: None,
            ..Default::default()
        };

        assert_eq!(t.tcl, 30);
        assert_eq!(t.tras, 72);
        assert!((t.frequency_mhz - 6000.0).abs() < 1.0);
        assert!(t.cmd2t);
        assert!(!t.gdm);
    }

    // =========================================================================
    // MemTimings Default
    // =========================================================================

    #[test]
    fn test_mem_timings_default() {
        let t = MemTimings::default();
        assert!(t.mem_type.is_none());
        assert_eq!(t.ratio, 0);
        assert_eq!(t.tcl, 0);
        assert!(!t.cmd2t);
        assert!(!t.gdm);
    }

    // =========================================================================
    // MemConfig
    // =========================================================================

    #[test]
    fn test_mem_config_empty() {
        let mc = MemConfig {
            mem_type: MemType::Unknown,
            channels: vec![],
        };
        assert!(mc.channels.is_empty());
    }

    // =========================================================================
    // Full register encode/decode roundtrip
    // =========================================================================

    #[test]
    fn test_roundtrip_tim0() {
        let cl = 16u32;
        let ras = 36u32;
        let rcdrd = 16u32;
        let rcdwr = 16u32;
        let encoded = cl | (ras << 8) | (rcdrd << 16) | (rcdwr << 24);

        assert_eq!(encoded & 0x3F, cl);
        assert_eq!((encoded >> 8) & 0x7F, ras);
        assert_eq!((encoded >> 16) & 0x3F, rcdrd);
        assert_eq!((encoded >> 24) & 0x3F, rcdwr);
    }

    #[test]
    fn test_roundtrip_rfc() {
        let rfc = 350u32;
        let rfc2 = 260u32;
        let rfc4 = 160u32;
        let encoded = rfc | (rfc2 << 11) | (rfc4 << 22);

        assert_eq!(encoded & 0x7FF, rfc);
        assert_eq!((encoded >> 11) & 0x7FF, rfc2);
        assert_eq!((encoded >> 22) & 0x3FF, rfc4);
    }

    #[test]
    fn test_roundtrip_cfg_ddr4() {
        let ratio = 54u32;
        let cmd2t = true;
        let gdm = true;
        let encoded = ratio | ((cmd2t as u32) << 10) | ((gdm as u32) << 11);

        assert_eq!(encoded & 0x7F, 54);
        assert_eq!((encoded >> 10) & 1, 1);
        assert_eq!((encoded >> 11) & 1, 1);
    }

    #[test]
    fn test_roundtrip_cfg_ddr5() {
        // DDR5: bits [15:0] = MCLK, bit 17 = Cmd2T, bit 18 = GDM
        let mclk = 3000u32;
        let cmd2t = true;
        let gdm = false;
        let encoded = mclk | ((cmd2t as u32) << 17) | ((gdm as u32) << 18);

        assert_eq!(encoded & 0xFFFF, 3000);
        assert_eq!((encoded >> 17) & 1, 1);
        assert_eq!((encoded >> 18) & 1, 0);
    }

    #[test]
    fn test_cfg_ddr5_real_register() {
        // Real Zen 5 9950X register value
        let cfg: u32 = 0x80050BB8;
        // MCLK
        assert_eq!(cfg & 0xFFFF, 3000);
        // Cmd2T (bit 17) = 0 → 1T
        assert_eq!((cfg >> 17) & 1, 0);
        // GDM (bit 18) = 1 → Enabled
        assert_eq!((cfg >> 18) & 1, 1);
    }

    // =========================================================================
    // Channel enabled bit
    // =========================================================================

    #[test]
    fn test_channel_enabled_bit() {
        let enabled: u32 = 0; // bit 19 = 0 → enabled
        let disabled: u32 = 1 << 19; // bit 19 = 1 → disabled
        assert_eq!((enabled >> 19) & 1, 0);
        assert_eq!((disabled >> 19) & 1, 1);
    }

    // =========================================================================
    // Edge cases
    // =========================================================================

    #[test]
    fn test_timing_max_values() {
        // Verify mask widths handle maximum values
        assert_eq!(0x3F, 63); // 6-bit max (CL, RCDRD, etc.)
        assert_eq!(0x7F, 127); // 7-bit max (RAS, ratio)
        assert_eq!(0xFF, 255); // 8-bit max (RC, FAW, WR)
        assert_eq!(0x1F, 31); // 5-bit max (RRDS, RRDL, RTP, WTRS)
        assert_eq!(0x7FF, 2047); // 11-bit max (RFC, RFC2)
        assert_eq!(0x3FF, 1023); // 10-bit max (RFC4)
        assert_eq!(0xFFFF, 65535); // 16-bit max (REFI)
    }
}
