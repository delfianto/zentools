//! AMD Ryzen SMU interface
//!
//! Provides access to AMD Ryzen CPU telemetry through multiple data sources:
//! - **Tier 1**: Direct hardware reads via MSR (RAPL power) and SMN (temperature)
//! - **Tier 2**: ryzen_smu kernel driver sysfs interface (PM table)
//! - **Tier 3**: Graceful fallback with partial data

pub mod driver;
pub mod msr;
pub mod pmtable;
pub mod smn;
mod types;

pub use types::*;

/// Read unified CPU metrics from all available sources.
///
/// Tries the PM table first (most complete), then fills gaps with direct
/// register reads. Returns whatever data is available.
pub fn read_metrics(smn_reader: Option<&smn::SmnReader>, rapl: Option<&mut msr::RaplReader>) -> CpuMetrics {
    let mut metrics = CpuMetrics::default();
    let mut used_pmtable = false;
    let mut used_direct = false;

    // Tier 2: Try PM table via ryzen_smu driver
    if let Ok(pm_data) = driver::read_pm_table(true) {
        let pm_metrics = pmtable::parse_pm_table(&pm_data);
        metrics = pm_metrics;
        used_pmtable = true;
    }

    // Tier 1: Fill gaps with direct register reads
    if let Some(reader) = smn_reader {
        // Temperature from SMN
        if metrics.tctl_temp_c.is_none()
            && let Ok(temp) = reader.read_tctl()
        {
            metrics.tctl_temp_c = Some(temp);
            used_direct = true;
        }

        if metrics.ccd_temps_c.is_empty() {
            let max_ccds = if reader.is_available() { 8 } else { 0 };
            if let Ok(temps) = reader.read_all_ccd_temps(max_ccds)
                && !temps.is_empty()
            {
                metrics.ccd_temps_c = temps;
                used_direct = true;
            }
        }

        // Voltage from SMN SVI (experimental)
        if metrics.core_voltage_v.is_none()
            && let Ok(Some(v)) = reader.read_core_voltage()
        {
            metrics.core_voltage_v = Some(v);
            used_direct = true;
        }
        if metrics.soc_voltage_v.is_none()
            && let Ok(Some(v)) = reader.read_soc_voltage()
        {
            metrics.soc_voltage_v = Some(v);
            used_direct = true;
        }
    }

    // Power from RAPL
    if let Some(rapl) = rapl {
        if metrics.package_power_w.is_none()
            && let Ok(Some(power)) = rapl.read_package_power()
        {
            metrics.package_power_w = Some(power);
            used_direct = true;
        }
        if metrics.core_power_w.is_none()
            && let Ok(Some(power)) = rapl.read_core_power()
        {
            metrics.core_power_w = Some(power);
            used_direct = true;
        }
    }

    // Set source indicator
    metrics.source = match (used_pmtable, used_direct) {
        (true, true) => MetricsSource::Hybrid,
        (true, false) => MetricsSource::PmTable,
        (false, _) => MetricsSource::DirectRegisters,
    };

    metrics
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // SmuVersion (via types re-export)
    // =========================================================================

    #[test]
    fn test_smu_version_parsing() {
        let version: SmuVersion = "SMU v98.82.0".parse().unwrap();
        assert_eq!(version.major, 98);
        assert_eq!(version.minor, 82);
        assert_eq!(version.patch, 0);
        assert_eq!(version.to_string(), "SMU v98.82.0");

        let version2: SmuVersion = "98.82.0".parse().unwrap();
        assert_eq!(version2.major, 98);

        let version3: SmuVersion = "  98.82.0  ".parse().unwrap();
        assert_eq!(version3.major, 98);
    }

    // =========================================================================
    // CpuCodename (via types re-export)
    // =========================================================================

    #[test]
    fn test_codename_conversion() {
        assert!(matches!(CpuCodename::from_u32(23), CpuCodename::GraniteRidge));
        assert!(matches!(CpuCodename::from_u32(12), CpuCodename::Vermeer));
        assert!(matches!(CpuCodename::from_u32(20), CpuCodename::Raphael));
        assert!(matches!(CpuCodename::from_u32(4), CpuCodename::Matisse));
        assert!(matches!(CpuCodename::from_u32(22), CpuCodename::StrixPoint));
        assert!(matches!(CpuCodename::from_u32(999), CpuCodename::Unknown(999)));
    }

    #[test]
    fn test_codename_properties() {
        assert!(CpuCodename::GraniteRidge.is_desktop());
        assert!(CpuCodename::GraniteRidge.is_zen5());
        assert!(CpuCodename::Raphael.is_desktop());
        assert!(!CpuCodename::Raphael.is_zen5());
        assert!(CpuCodename::StrixPoint.is_mobile());
        assert!(CpuCodename::StrixPoint.is_zen5());
        assert!(CpuCodename::StormPeak.is_hedt());
        assert!(CpuCodename::CastlePeak.is_hedt());

        assert_eq!(CpuCodename::GraniteRidge.generation(), "Zen 5");
        assert_eq!(CpuCodename::Raphael.generation(), "Zen 4");
        assert_eq!(CpuCodename::Vermeer.generation(), "Zen 3");
        assert_eq!(CpuCodename::Matisse.generation(), "Zen 2");
    }

    // =========================================================================
    // PmTableData (via types re-export)
    // =========================================================================

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
        assert_eq!(pm_table.read_f32(100), None);
    }

    // =========================================================================
    // read_metrics — with no data sources
    // =========================================================================

    #[test]
    fn test_read_metrics_no_sources() {
        // Both sources None => should return default metrics
        let metrics = read_metrics(None, None);
        assert_eq!(metrics.source, MetricsSource::DirectRegisters);
        // All fields should be None (no driver, no SMN, no RAPL)
        assert!(metrics.tctl_temp_c.is_none());
        assert!(metrics.package_power_w.is_none());
        assert!(metrics.core_voltage_v.is_none());
    }

    #[test]
    fn test_read_metrics_with_unavailable_smn() {
        // SMN reader pointing to nonexistent device
        let smn = smn::SmnReader::with_device("/nonexistent".to_string(), false);
        let metrics = read_metrics(Some(&smn), None);
        // SMN is "available" check will fail, so no data filled
        assert!(metrics.tctl_temp_c.is_none());
    }

    // =========================================================================
    // CpuMetrics — construction and modification
    // =========================================================================

    #[test]
    fn test_cpu_metrics_default_all_none() {
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

    #[test]
    fn test_cpu_metrics_can_set_fields() {
        let mut m = CpuMetrics::default();
        m.tctl_temp_c = Some(65.0);
        m.package_power_w = Some(88.5);
        m.source = MetricsSource::Hybrid;

        assert_eq!(m.tctl_temp_c, Some(65.0));
        assert_eq!(m.package_power_w, Some(88.5));
        assert_eq!(m.source, MetricsSource::Hybrid);
    }

    #[test]
    fn test_cpu_metrics_clone() {
        let mut m = CpuMetrics::default();
        m.tctl_temp_c = Some(70.0);
        m.per_core.push(CoreMetrics {
            core_id: 0,
            power_w: Some(5.0),
            ..Default::default()
        });

        let m2 = m.clone();
        assert_eq!(m2.tctl_temp_c, Some(70.0));
        assert_eq!(m2.per_core.len(), 1);
        assert_eq!(m2.per_core[0].core_id, 0);
    }

    // =========================================================================
    // MetricsSource — source logic
    // =========================================================================

    #[test]
    fn test_metrics_source_variants() {
        assert_eq!(MetricsSource::PmTable, MetricsSource::PmTable);
        assert_eq!(MetricsSource::DirectRegisters, MetricsSource::DirectRegisters);
        assert_eq!(MetricsSource::Hybrid, MetricsSource::Hybrid);
        assert_ne!(MetricsSource::PmTable, MetricsSource::Hybrid);
    }

    // =========================================================================
    // Re-export validation
    // =========================================================================

    #[test]
    fn test_public_api_types_accessible() {
        // Verify that key types are accessible through the smu module
        let _: SmuError = SmuError::InvalidCodename(0);
        let _: CpuCodename = CpuCodename::GraniteRidge;
        let _: MetricsSource = MetricsSource::Hybrid;
        let _: PmTableData = PmTableData {
            version: 0,
            data: vec![],
        };
        let _: CpuMetrics = CpuMetrics::default();
        let _: CoreMetrics = CoreMetrics::default();
    }
}
