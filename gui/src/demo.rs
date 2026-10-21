//! Explicit preview data. No hardware is read and writes are disabled in this mode.
use std::collections::BTreeMap;
use zentools::{
    aod::AodCapture,
    monitor::{Identity, SNAPSHOT_VERSION, Snapshot, now_ms},
    smu::{
        CoreMetrics, CpuCodename, CpuMetrics, MetricsSource,
        driver::CpuTopology,
        mem::{MemChannel, MemConfig, MemType, decode_registers},
    },
};

pub fn snapshot(tick: u64) -> Snapshot {
    let phase = tick as f64 * 0.25;
    let values = [
        (0x50200, 0x80050bb8),
        (0x50204, 0x24244c1e),
        (0x50208, 0x00240070),
        (0x5020c, 0x17000f08),
        (0x50210, 0x20),
        (0x50214, 0x001e081c),
        (0x50218, 0x5a),
        (0x50220, 0x08010b0b),
        (0x50224, 0x17010b0b),
        (0x50228, 0x1508),
        (0x50230, 0x00c02d9d),
        (0x5012c, 0x15381140),
        (0x5021c, 2),
        (0x50234, 0x20202a2a),
        (0x50238, 0x07000392),
        (0x50250, 0x00070000),
        (0x50254, 0x000f0d17),
        (0x50258, 0x06241012),
        (0x50260, 0x00c00138),
        (0x50264, 0x00c00138),
        (0x50268, 0x01e00374),
        (0x5026c, 0x01e00374),
        (0x502c0, 0),
        (0x502c4, 0),
        (0x502c8, 0x005a0186),
        (0x502cc, 0x005a0186),
        (0x50284, 0x231),
    ];
    let registers: BTreeMap<_, _> = values.into_iter().map(|(k, v)| (k, Some(v))).collect();
    let timings = decode_registers(&registers, MemType::DDR5).expect("valid preview fixture");
    let channels = (0..2)
        .map(|id| MemChannel {
            channel_id: id,
            dimm0_present: true,
            dimm1_present: false,
            timings: timings.clone(),
            registers: registers.clone(),
        })
        .collect();
    let metrics = CpuMetrics {
        source: MetricsSource::Hybrid,
        tctl_temp_c: Some(55.0 + phase.sin() * 5.0),
        package_power_w: Some(62.0 + phase.sin() * 13.0),
        peak_core_freq_mhz: Some(5400.0 + phase.cos() * 180.0),
        core_voltage_v: Some(1.14),
        soc_voltage_v: Some(1.20),
        mclk_mhz: Some(3000.0),
        uclk_mhz: Some(3000.0),
        fclk_mhz: Some(2000.0),
        ppt_current_w: Some(62.0 + phase.sin() * 13.0),
        ppt_limit_w: Some(230.0),
        tdc_current_a: Some(38.0),
        tdc_limit_a: Some(160.0),
        edc_current_a: Some(64.0),
        edc_limit_a: Some(225.0),
        per_core: (0..16)
            .map(|id| CoreMetrics {
                core_id: id,
                frequency_mhz: Some(4200.0 + (phase + id as f64).sin() * 1100.0),
                temp_c: Some(48.0 + (phase + id as f64).cos() * 8.0),
                power_w: Some(2.0 + (phase + id as f64).sin().abs() * 4.0),
                c0_pct: Some(5.0 + (phase + id as f64).sin().abs() * 50.0),
                ..CoreMetrics::default()
            })
            .collect(),
        ..CpuMetrics::default()
    };
    Snapshot {
        schema_version: SNAPSHOT_VERSION,
        timestamp_ms: now_ms(),
        identity: Identity {
            model: "AMD Ryzen 9 9950X 16-Core Processor".into(),
            codename: Some(CpuCodename::GraniteRidge),
            board: "MPG X870E CARBON WIFI (MS-7E49)".into(),
            bios: "1.AA3".into(),
            boot_id: "preview".into(),
            topology: Some(CpuTopology {
                logical_cpus: 32,
                physical_cores: 16,
                sockets: 1,
                smt: true,
            }),
        },
        metrics,
        memory: Some(MemConfig {
            mem_type: MemType::DDR5,
            channels,
        }),
        memory_error: None,
        memory_timestamp_ms: Some(now_ms()),
        firmware: Some(
            AodCapture::parse(include_str!(
                "../../tools/aod-probe/fixtures/ms7e49-1aa3.txt"
            ))
            .expect("valid AOD fixture"),
        ),
        firmware_error: None,
        epp_profiles: (0..32)
            .map(|id| (id, "balance_performance".into()))
            .collect(),
        epp_error: None,
        cpu_usage_pct: Some(18.0 + phase.sin() * 9.0),
        candidates: Vec::new(),
        pm_table_version: None,
        pm_table: None,
    }
}
