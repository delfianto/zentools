//! PM table field definitions and version-specific parsing
//!
//! Maps PM table byte offsets to named fields for each known table version.
//! Zen 2/3 offsets from ryzen_smu's monitor_cpu.py. Zen 5 offsets are partial.

use super::types::{CoreMetrics, CpuMetrics, MetricsSource, PmTableData};

/// Data type of a PM table field
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    F32,
    U32,
}

/// A single field in the PM table
#[derive(Debug, Clone)]
pub struct PmTableField {
    pub name: &'static str,
    pub offset: usize,
    pub data_type: FieldType,
    pub unit: &'static str,
}

/// Maximum number of cores supported in per-core parsing
const MAX_CORES: usize = 32;

// =============================================================================
// Zen 2/3 PM table field map (versions 0x240xxx)
// Offsets from ryzen_smu monitor_cpu.py
// =============================================================================

const ZEN2_FIELDS: &[PmTableField] = &[
    // PBO Limits
    PmTableField { name: "PPT Limit", offset: 0x000, data_type: FieldType::F32, unit: "W" },
    PmTableField { name: "PPT Current", offset: 0x004, data_type: FieldType::F32, unit: "W" },
    PmTableField { name: "TDC Limit", offset: 0x008, data_type: FieldType::F32, unit: "A" },
    PmTableField { name: "TDC Current", offset: 0x00C, data_type: FieldType::F32, unit: "A" },
    PmTableField { name: "TjMax", offset: 0x010, data_type: FieldType::F32, unit: "C" },
    PmTableField { name: "Tctl", offset: 0x014, data_type: FieldType::F32, unit: "C" },
    PmTableField { name: "EDC Limit", offset: 0x020, data_type: FieldType::F32, unit: "A" },
    PmTableField { name: "EDC Current", offset: 0x024, data_type: FieldType::F32, unit: "A" },
    PmTableField { name: "SVI2 Voltage", offset: 0x02C, data_type: FieldType::F32, unit: "V" },
    // Power
    PmTableField { name: "Core Power", offset: 0x060, data_type: FieldType::F32, unit: "W" },
    PmTableField { name: "SoC Power", offset: 0x064, data_type: FieldType::F32, unit: "W" },
    // Voltage
    PmTableField { name: "Peak Voltage", offset: 0x0A0, data_type: FieldType::F32, unit: "V" },
    PmTableField { name: "SoC Voltage", offset: 0x0B0, data_type: FieldType::F32, unit: "V" },
    PmTableField { name: "SoC Current", offset: 0x0B8, data_type: FieldType::F32, unit: "A" },
    // Clocks
    PmTableField { name: "FCLK", offset: 0x0C0, data_type: FieldType::F32, unit: "MHz" },
    PmTableField { name: "FCLK Avg", offset: 0x0C4, data_type: FieldType::F32, unit: "MHz" },
    PmTableField { name: "UCLK", offset: 0x128, data_type: FieldType::F32, unit: "MHz" },
    PmTableField { name: "MCLK", offset: 0x138, data_type: FieldType::F32, unit: "MHz" },
    // Memory voltage
    PmTableField { name: "cLDO_VDDP", offset: 0x1F4, data_type: FieldType::F32, unit: "V" },
    PmTableField { name: "cLDO_VDDG", offset: 0x1F8, data_type: FieldType::F32, unit: "V" },
];

/// Per-core field offsets for Zen 2/3 (multiply core index by 4 and add to base)
struct Zen2CoreOffsets {
    power_base: usize,
    frequency_base: usize,
    activity_base: usize,
    sleep_base: usize,
}

const ZEN2_CORE: Zen2CoreOffsets = Zen2CoreOffsets {
    power_base: 0x24C,
    frequency_base: 0x30C,
    activity_base: 0x32C,
    sleep_base: 0x36C,
};

// =============================================================================
// Zen 5 PM table field map (versions 0x620xxx)
// WARNING: These offsets are EXPERIMENTAL and partially mapped.
// The first section (PBO limits) is likely similar to Zen 2/3, but
// offsets beyond ~0x30 are UNVERIFIED for Zen 5.
// =============================================================================

const ZEN5_FIELDS: &[PmTableField] = &[
    // PBO Limits — likely at same offsets as Zen 2/3 (needs verification)
    PmTableField { name: "PPT Limit", offset: 0x000, data_type: FieldType::F32, unit: "W" },
    PmTableField { name: "PPT Current", offset: 0x004, data_type: FieldType::F32, unit: "W" },
    PmTableField { name: "TDC Limit", offset: 0x008, data_type: FieldType::F32, unit: "A" },
    PmTableField { name: "TDC Current", offset: 0x00C, data_type: FieldType::F32, unit: "A" },
    PmTableField { name: "TjMax", offset: 0x010, data_type: FieldType::F32, unit: "C" },
    PmTableField { name: "Tctl", offset: 0x014, data_type: FieldType::F32, unit: "C" },
    PmTableField { name: "EDC Limit", offset: 0x020, data_type: FieldType::F32, unit: "A" },
    PmTableField { name: "EDC Current", offset: 0x024, data_type: FieldType::F32, unit: "A" },
];

/// Get the field map for a given PM table version
pub fn get_field_map(version: u32) -> Option<&'static [PmTableField]> {
    match version {
        // Zen 2 (Matisse, Castle Peak)
        0x240903 | 0x240802 | 0x240803 => Some(ZEN2_FIELDS),
        // Zen 5 (Granite Ridge) — partial mapping
        0x620105 | 0x620205 => Some(ZEN5_FIELDS),
        _ => None,
    }
}

/// Check if a PM table version has per-core field mappings
pub fn has_per_core_fields(version: u32) -> bool {
    matches!(version, 0x240903 | 0x240802 | 0x240803)
}

/// Check if the PM table version mapping is experimental (incomplete)
pub fn is_experimental(version: u32) -> bool {
    matches!(version, 0x620105 | 0x620205)
}

/// Parse a PM table into structured metrics
pub fn parse_pm_table(pm_table: &PmTableData) -> CpuMetrics {
    let mut metrics = CpuMetrics {
        source: MetricsSource::PmTable,
        ..Default::default()
    };

    let fields = match get_field_map(pm_table.version) {
        Some(f) => f,
        None => return metrics,
    };

    for field in fields {
        let value = match field.data_type {
            FieldType::F32 => pm_table.read_f32(field.offset).map(|v| v as f64),
            FieldType::U32 => pm_table.read_u32(field.offset).map(|v| v as f64),
        };

        // Skip NaN, inf, and near-zero values (0.0 in PM table means "not available")
        let value = match value {
            Some(v) if v.is_finite() && v.abs() > 0.001 => v,
            _ => continue,
        };

        match field.name {
            "PPT Limit" => metrics.ppt_limit_w = Some(value),
            "PPT Current" => metrics.ppt_current_w = Some(value),
            "TDC Limit" => metrics.tdc_limit_a = Some(value),
            "TDC Current" => metrics.tdc_current_a = Some(value),
            "TjMax" => metrics.tjmax_c = Some(value),
            "Tctl" => metrics.tctl_temp_c = Some(value),
            "EDC Limit" => metrics.edc_limit_a = Some(value),
            "EDC Current" => metrics.edc_current_a = Some(value),
            "SVI2 Voltage" => metrics.core_voltage_v = Some(value),
            "Core Power" => metrics.core_power_w = Some(value),
            "SoC Power" => metrics.soc_power_w = Some(value),
            "Peak Voltage" => metrics.peak_voltage_v = Some(value),
            "SoC Voltage" => metrics.soc_voltage_v = Some(value),
            "SoC Current" => {} // Not in CpuMetrics yet
            "FCLK" | "FCLK Avg" => metrics.fclk_mhz = Some(value),
            "UCLK" => metrics.uclk_mhz = Some(value),
            "MCLK" => metrics.mclk_mhz = Some(value),
            _ => {}
        }
    }

    // Parse per-core data for versions that have it
    if has_per_core_fields(pm_table.version) {
        metrics.per_core = parse_per_core_zen2(pm_table);
    }

    metrics
}

/// Parse per-core metrics from a Zen 2/3 PM table
fn parse_per_core_zen2(pm_table: &PmTableData) -> Vec<CoreMetrics> {
    let mut cores = Vec::new();

    for i in 0..MAX_CORES {
        let freq_offset = ZEN2_CORE.frequency_base + i * 4;
        let freq = pm_table.read_f32(freq_offset).map(|v| v as f64);

        // Stop at first core with no frequency data (past the actual core count)
        match freq {
            Some(f) if f > 0.0 => {}
            _ => break,
        }

        cores.push(CoreMetrics {
            core_id: i as u32,
            power_w: pm_table
                .read_f32(ZEN2_CORE.power_base + i * 4)
                .map(|v| v as f64),
            frequency_mhz: freq.map(|f| f * 1000.0), // GHz -> MHz
            activity_pct: pm_table
                .read_f32(ZEN2_CORE.activity_base + i * 4)
                .map(|v| v as f64),
            sleep_pct: pm_table
                .read_f32(ZEN2_CORE.sleep_base + i * 4)
                .map(|v| v as f64),
        });
    }

    cores
}

/// Dump all named fields from a PM table as (name, value, unit) tuples
pub fn dump_named_fields(pm_table: &PmTableData) -> Vec<(&'static str, f64, &'static str)> {
    let fields = match get_field_map(pm_table.version) {
        Some(f) => f,
        None => return Vec::new(),
    };

    let mut result = Vec::new();
    for field in fields {
        let value = match field.data_type {
            FieldType::F32 => pm_table.read_f32(field.offset).map(|v| v as f64),
            FieldType::U32 => pm_table.read_u32(field.offset).map(|v| v as f64),
        };

        if let Some(v) = value
            && v.is_finite()
            && v.abs() > 0.0001
        {
            result.push((field.name, v, field.unit));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a PM table with specific f32 values at offsets
    fn make_pm_table(version: u32, size: usize, values: &[(usize, f32)]) -> PmTableData {
        let mut data = vec![0u8; size];
        for &(offset, val) in values {
            if offset + 4 <= size {
                data[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
            }
        }
        PmTableData { version, data }
    }

    // =========================================================================
    // get_field_map
    // =========================================================================

    #[test]
    fn test_get_field_map_zen2_versions() {
        assert!(get_field_map(0x240903).is_some());
        assert!(get_field_map(0x240802).is_some());
        assert!(get_field_map(0x240803).is_some());
    }

    #[test]
    fn test_get_field_map_zen5_versions() {
        assert!(get_field_map(0x620105).is_some());
        assert!(get_field_map(0x620205).is_some());
    }

    #[test]
    fn test_get_field_map_unknown() {
        assert!(get_field_map(0x000000).is_none());
        assert!(get_field_map(0x999999).is_none());
        assert!(get_field_map(0x240901).is_none());
        assert!(get_field_map(u32::MAX).is_none());
    }

    #[test]
    fn test_get_field_map_zen2_has_more_fields_than_zen5() {
        let zen2 = get_field_map(0x240903).unwrap();
        let zen5 = get_field_map(0x620205).unwrap();
        assert!(zen2.len() > zen5.len(), "Zen 2 should have more mapped fields than experimental Zen 5");
    }

    // =========================================================================
    // is_experimental
    // =========================================================================

    #[test]
    fn test_is_experimental_zen5() {
        assert!(is_experimental(0x620105));
        assert!(is_experimental(0x620205));
    }

    #[test]
    fn test_is_not_experimental_zen2() {
        assert!(!is_experimental(0x240903));
        assert!(!is_experimental(0x240802));
        assert!(!is_experimental(0x240803));
    }

    #[test]
    fn test_is_not_experimental_unknown() {
        assert!(!is_experimental(0x999999));
    }

    // =========================================================================
    // has_per_core_fields
    // =========================================================================

    #[test]
    fn test_has_per_core_fields_zen2() {
        assert!(has_per_core_fields(0x240903));
        assert!(has_per_core_fields(0x240802));
        assert!(has_per_core_fields(0x240803));
    }

    #[test]
    fn test_no_per_core_fields_zen5() {
        assert!(!has_per_core_fields(0x620105));
        assert!(!has_per_core_fields(0x620205));
    }

    #[test]
    fn test_no_per_core_fields_unknown() {
        assert!(!has_per_core_fields(0x999999));
    }

    // =========================================================================
    // PmTableField structure
    // =========================================================================

    #[test]
    fn test_zen2_fields_offsets_ascending() {
        // Verify field offsets are in ascending order
        let fields = get_field_map(0x240903).unwrap();
        for window in fields.windows(2) {
            assert!(
                window[0].offset < window[1].offset,
                "fields should be in ascending offset order: {} (0x{:x}) >= {} (0x{:x})",
                window[0].name,
                window[0].offset,
                window[1].name,
                window[1].offset
            );
        }
    }

    #[test]
    fn test_zen2_fields_all_f32() {
        let fields = get_field_map(0x240903).unwrap();
        for field in fields {
            assert_eq!(
                field.data_type,
                FieldType::F32,
                "Zen 2 field '{}' should be F32",
                field.name
            );
        }
    }

    #[test]
    fn test_field_units_valid() {
        let valid_units = ["W", "A", "C", "V", "MHz"];
        for version in [0x240903u32, 0x620205] {
            if let Some(fields) = get_field_map(version) {
                for field in fields {
                    assert!(
                        valid_units.contains(&field.unit),
                        "field '{}' has unexpected unit '{}'",
                        field.name,
                        field.unit
                    );
                }
            }
        }
    }

    #[test]
    fn test_field_names_non_empty() {
        for version in [0x240903u32, 0x240802, 0x620105, 0x620205] {
            if let Some(fields) = get_field_map(version) {
                for field in fields {
                    assert!(!field.name.is_empty());
                }
            }
        }
    }

    // =========================================================================
    // parse_pm_table — Zen 2
    // =========================================================================

    #[test]
    fn test_parse_pm_table_zen2_basic() {
        let pt = make_pm_table(0x240903, 1024, &[
            (0x000, 142.0),  // PPT Limit
            (0x014, 65.5),   // Tctl
            (0x0C0, 1800.0), // FCLK
        ]);

        let metrics = parse_pm_table(&pt);
        assert_eq!(metrics.source, MetricsSource::PmTable);
        assert!((metrics.ppt_limit_w.unwrap() - 142.0).abs() < 0.1);
        assert!((metrics.tctl_temp_c.unwrap() - 65.5).abs() < 0.1);
        assert!((metrics.fclk_mhz.unwrap() - 1800.0).abs() < 0.1);
    }

    #[test]
    fn test_parse_pm_table_zen2_all_fields() {
        let pt = make_pm_table(0x240903, 0x200, &[
            (0x000, 142.0),   // PPT Limit
            (0x004, 95.3),    // PPT Current
            (0x008, 120.0),   // TDC Limit
            (0x00C, 45.2),    // TDC Current
            (0x010, 95.0),    // TjMax
            (0x014, 72.1),    // Tctl
            (0x020, 200.0),   // EDC Limit
            (0x024, 89.5),    // EDC Current
            (0x02C, 1.325),   // SVI2 Voltage
            (0x060, 88.7),    // Core Power
            (0x064, 12.3),    // SoC Power
            (0x0A0, 1.45),    // Peak Voltage
            (0x0B0, 1.1),     // SoC Voltage
            (0x0C0, 1800.0),  // FCLK
            (0x128, 3200.0),  // UCLK
            (0x138, 3200.0),  // MCLK
            (0x1F4, 0.95),    // cLDO_VDDP
            (0x1F8, 1.025),   // cLDO_VDDG
        ]);

        let metrics = parse_pm_table(&pt);
        assert!((metrics.ppt_limit_w.unwrap() - 142.0).abs() < 0.1);
        assert!((metrics.ppt_current_w.unwrap() - 95.3).abs() < 0.1);
        assert!((metrics.tdc_limit_a.unwrap() - 120.0).abs() < 0.1);
        assert!((metrics.tdc_current_a.unwrap() - 45.2).abs() < 0.1);
        assert!((metrics.tjmax_c.unwrap() - 95.0).abs() < 0.1);
        assert!((metrics.tctl_temp_c.unwrap() - 72.1).abs() < 0.1);
        assert!((metrics.edc_limit_a.unwrap() - 200.0).abs() < 0.1);
        assert!((metrics.edc_current_a.unwrap() - 89.5).abs() < 0.1);
        assert!((metrics.core_voltage_v.unwrap() - 1.325).abs() < 0.01);
        assert!((metrics.core_power_w.unwrap() - 88.7).abs() < 0.1);
        assert!((metrics.soc_power_w.unwrap() - 12.3).abs() < 0.1);
        assert!((metrics.peak_voltage_v.unwrap() - 1.45).abs() < 0.01);
        assert!((metrics.soc_voltage_v.unwrap() - 1.1).abs() < 0.01);
        assert!((metrics.fclk_mhz.unwrap() - 1800.0).abs() < 0.1);
        assert!((metrics.uclk_mhz.unwrap() - 3200.0).abs() < 0.1);
        assert!((metrics.mclk_mhz.unwrap() - 3200.0).abs() < 0.1);
    }

    // =========================================================================
    // parse_pm_table — Zen 5 (experimental)
    // =========================================================================

    #[test]
    fn test_parse_pm_table_zen5_pbo_limits() {
        let pt = make_pm_table(0x620205, 0x994, &[
            (0x000, 200.0),  // PPT Limit
            (0x004, 150.0),  // PPT Current
            (0x010, 95.0),   // TjMax
            (0x014, 68.0),   // Tctl
        ]);

        let metrics = parse_pm_table(&pt);
        assert_eq!(metrics.source, MetricsSource::PmTable);
        assert!((metrics.ppt_limit_w.unwrap() - 200.0).abs() < 0.1);
        assert!((metrics.ppt_current_w.unwrap() - 150.0).abs() < 0.1);
        assert!((metrics.tjmax_c.unwrap() - 95.0).abs() < 0.1);
        assert!((metrics.tctl_temp_c.unwrap() - 68.0).abs() < 0.1);
    }

    #[test]
    fn test_parse_pm_table_zen5_no_per_core() {
        let pt = make_pm_table(0x620205, 0x994, &[]);
        let metrics = parse_pm_table(&pt);
        assert!(metrics.per_core.is_empty());
    }

    // =========================================================================
    // parse_pm_table — edge cases
    // =========================================================================

    #[test]
    fn test_parse_pm_table_unknown_version() {
        let pt = PmTableData {
            version: 0x999999,
            data: vec![0; 256],
        };
        let metrics = parse_pm_table(&pt);
        assert_eq!(metrics.source, MetricsSource::PmTable);
        // All fields should remain None
        assert!(metrics.ppt_limit_w.is_none());
        assert!(metrics.tctl_temp_c.is_none());
        assert!(metrics.per_core.is_empty());
    }

    #[test]
    fn test_parse_pm_table_empty_data() {
        let pt = PmTableData {
            version: 0x240903,
            data: vec![],
        };
        let metrics = parse_pm_table(&pt);
        // Should not crash, fields remain None
        assert!(metrics.ppt_limit_w.is_none());
    }

    #[test]
    fn test_parse_pm_table_too_small_for_some_fields() {
        // Only 32 bytes — covers PPT but not FCLK at 0xC0
        let pt = make_pm_table(0x240903, 32, &[
            (0x000, 142.0),
        ]);
        let metrics = parse_pm_table(&pt);
        assert!((metrics.ppt_limit_w.unwrap() - 142.0).abs() < 0.1);
        assert!(metrics.fclk_mhz.is_none()); // offset 0xC0 out of bounds
    }

    #[test]
    fn test_parse_pm_table_nan_values_skipped() {
        let mut data = vec![0u8; 32];
        data[0x000..0x004].copy_from_slice(&f32::NAN.to_le_bytes());
        let pt = PmTableData {
            version: 0x240903,
            data,
        };
        let metrics = parse_pm_table(&pt);
        assert!(metrics.ppt_limit_w.is_none(), "NaN should be skipped");
    }

    #[test]
    fn test_parse_pm_table_inf_values_skipped() {
        let mut data = vec![0u8; 32];
        data[0x000..0x004].copy_from_slice(&f32::INFINITY.to_le_bytes());
        let pt = PmTableData {
            version: 0x240903,
            data,
        };
        let metrics = parse_pm_table(&pt);
        assert!(metrics.ppt_limit_w.is_none(), "Infinity should be skipped");
    }

    #[test]
    fn test_parse_pm_table_near_zero_skipped() {
        let pt = make_pm_table(0x240903, 32, &[
            (0x000, 0.0001), // below 0.001 threshold
        ]);
        let metrics = parse_pm_table(&pt);
        assert!(metrics.ppt_limit_w.is_none(), "near-zero should be skipped");
    }

    // =========================================================================
    // Per-core parsing
    // =========================================================================

    #[test]
    fn test_parse_per_core_zen2_with_cores() {
        let mut data = vec![0u8; 0x400];

        // Set up 4 cores
        for i in 0..4u32 {
            let freq = 4.5 + (i as f32) * 0.1; // GHz (4.5, 4.6, 4.7, 4.8)
            let power = 10.0 + (i as f32) * 5.0;
            let activity = 50.0 + (i as f32) * 10.0;
            let sleep = 5.0 + (i as f32) * 2.0;

            let off = (i as usize) * 4;
            data[0x30C + off..0x310 + off].copy_from_slice(&freq.to_le_bytes());
            data[0x24C + off..0x250 + off].copy_from_slice(&power.to_le_bytes());
            data[0x32C + off..0x330 + off].copy_from_slice(&activity.to_le_bytes());
            data[0x36C + off..0x370 + off].copy_from_slice(&sleep.to_le_bytes());
        }

        let pt = PmTableData {
            version: 0x240903,
            data,
        };
        let metrics = parse_pm_table(&pt);

        assert_eq!(metrics.per_core.len(), 4);

        assert_eq!(metrics.per_core[0].core_id, 0);
        assert!((metrics.per_core[0].frequency_mhz.unwrap() - 4500.0).abs() < 1.0);
        assert!((metrics.per_core[0].power_w.unwrap() - 10.0).abs() < 0.1);
        assert!((metrics.per_core[0].activity_pct.unwrap() - 50.0).abs() < 0.1);
        assert!((metrics.per_core[0].sleep_pct.unwrap() - 5.0).abs() < 0.1);

        assert_eq!(metrics.per_core[3].core_id, 3);
        assert!((metrics.per_core[3].frequency_mhz.unwrap() - 4800.0).abs() < 1.0);
    }

    #[test]
    fn test_parse_per_core_zen2_no_cores() {
        // All zeros at core frequency offsets = no active cores
        let data = vec![0u8; 0x400];
        let pt = PmTableData {
            version: 0x240903,
            data,
        };
        let metrics = parse_pm_table(&pt);
        assert!(metrics.per_core.is_empty());
    }

    #[test]
    fn test_parse_per_core_stops_at_zero_frequency() {
        let mut data = vec![0u8; 0x400];

        // Only 2 cores with frequency > 0
        data[0x30C..0x310].copy_from_slice(&4.5f32.to_le_bytes()); // core 0
        data[0x310..0x314].copy_from_slice(&4.6f32.to_le_bytes()); // core 1
        // core 2 = 0.0 -> stop

        let pt = PmTableData {
            version: 0x240903,
            data,
        };
        let metrics = parse_pm_table(&pt);
        assert_eq!(metrics.per_core.len(), 2);
    }

    // =========================================================================
    // dump_named_fields
    // =========================================================================

    #[test]
    fn test_dump_named_fields_zen2() {
        let pt = make_pm_table(0x240903, 1024, &[
            (0x000, 142.0),
            (0x014, 65.5),
        ]);

        let fields = dump_named_fields(&pt);
        assert!(fields.len() >= 2);

        let ppt = fields.iter().find(|(name, _, _)| *name == "PPT Limit");
        assert!(ppt.is_some());
        assert!((ppt.unwrap().1 - 142.0).abs() < 0.1);
        assert_eq!(ppt.unwrap().2, "W");

        let tctl = fields.iter().find(|(name, _, _)| *name == "Tctl");
        assert!(tctl.is_some());
        assert!((tctl.unwrap().1 - 65.5).abs() < 0.1);
        assert_eq!(tctl.unwrap().2, "C");
    }

    #[test]
    fn test_dump_named_fields_unknown_version() {
        let pt = PmTableData {
            version: 0x999999,
            data: vec![0; 256],
        };
        let fields = dump_named_fields(&pt);
        assert!(fields.is_empty());
    }

    #[test]
    fn test_dump_named_fields_skips_near_zero() {
        // All zeros -> no fields dumped (all below threshold)
        let pt = PmTableData {
            version: 0x240903,
            data: vec![0; 1024],
        };
        let fields = dump_named_fields(&pt);
        assert!(fields.is_empty());
    }

    #[test]
    fn test_dump_named_fields_skips_nan() {
        let mut data = vec![0u8; 1024];
        data[0x000..0x004].copy_from_slice(&f32::NAN.to_le_bytes());
        let pt = PmTableData {
            version: 0x240903,
            data,
        };
        let fields = dump_named_fields(&pt);
        // NaN should not appear
        assert!(!fields.iter().any(|(name, _, _)| *name == "PPT Limit"));
    }

    // =========================================================================
    // Cross-version consistency
    // =========================================================================

    #[test]
    fn test_zen2_zen5_share_pbo_offsets() {
        // The first few PBO fields should be at the same offsets
        let zen2 = get_field_map(0x240903).unwrap();
        let zen5 = get_field_map(0x620205).unwrap();

        let common_names = ["PPT Limit", "PPT Current", "TDC Limit", "TDC Current", "TjMax", "Tctl"];

        for name in &common_names {
            let z2 = zen2.iter().find(|f| f.name == *name);
            let z5 = zen5.iter().find(|f| f.name == *name);
            assert!(z2.is_some(), "Zen 2 missing field '{}'", name);
            assert!(z5.is_some(), "Zen 5 missing field '{}'", name);
            assert_eq!(
                z2.unwrap().offset,
                z5.unwrap().offset,
                "field '{}' offset mismatch between Zen 2 and Zen 5",
                name
            );
        }
    }
}
