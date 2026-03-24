//! SMU command handlers and display

use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Cell, CellAlignment, ContentArrangement, Table};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use zentools::smu::{self, driver, msr, pmtable, smn, CpuMetrics, SmuInfo, SMU_DRV_PATH};

use crate::SmuCommands;

pub fn handle_command(command: SmuCommands) -> Result<()> {
    match command {
        SmuCommands::Info { verbose } => info(verbose),
        SmuCommands::PmTable { force, raw, update } => pm_table(force, raw, update),
        SmuCommands::Monitor { interval } => monitor(interval),
        SmuCommands::Check => check(),
        SmuCommands::Debug => debug(),
    }
}

// =============================================================================
// Handlers
// =============================================================================

fn check() -> Result<()> {
    println!("Checking available data sources...\n");

    match driver::check_driver() {
        Ok(_) => println!("[OK] ryzen_smu driver loaded at {}", SMU_DRV_PATH),
        Err(e) => println!("[--] ryzen_smu driver: {}", e),
    }

    if msr::RaplReader::is_available() {
        println!("[OK] MSR access available (RAPL power monitoring)");
    } else {
        println!("[--] MSR access unavailable (try: sudo modprobe msr)");
    }

    let smn = smn::SmnReader::new(false);
    if smn.is_available() {
        println!("[OK] SMN access available (temperature monitoring)");
    } else {
        println!("[--] SMN access unavailable (requires root)");
    }

    println!();
    Ok(())
}

fn info(verbose: bool) -> Result<()> {
    let info = driver::read_info()?;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("AMD Ryzen SMU Information").set_alignment(CellAlignment::Center),
        ]);

    table.add_row(vec!["SMU Version", &info.version.to_string()]);
    table.add_row(vec!["Codename", info.codename.as_str()]);
    table.add_row(vec!["Generation", info.codename.generation()]);
    table.add_row(vec!["Driver Version", &info.drv_version]);
    table.add_row(vec![
        "PM Table Version",
        &format!("0x{:X}", info.pm_table_version),
    ]);
    table.add_row(vec![
        "PM Table Size",
        &format!("{} bytes", info.pm_table_size),
    ]);

    if let Some(mp1_ver) = info.mp1_if_version {
        table.add_row(vec!["MP1 IF Version", &mp1_ver.to_string()]);
    }

    if pmtable::is_experimental(info.pm_table_version) {
        table.add_row(vec!["PM Table Status", "EXPERIMENTAL (partial mapping)"]);
    }

    println!();
    println!("{}", table);

    if verbose {
        println!("\nDriver Path: {}", SMU_DRV_PATH);
        println!("PM Table Path: {}/pm_table", SMU_DRV_PATH);
    }

    Ok(())
}

fn debug() -> Result<()> {
    // ── CPU Info ─────────────────────────────────────────────────────────
    println!("=== CPU Information ===\n");

    if let Some(model) = driver::read_cpu_model() {
        println!("  Model: {}", model);
    }
    if let Some(topo) = driver::read_cpu_topology() {
        println!(
            "  Topology: {} cores / {} threads ({}SMT), {} socket(s)",
            topo.physical_cores,
            topo.logical_cpus,
            if topo.smt { "" } else { "no " },
            topo.sockets
        );
    }

    // ── Driver sysfs ─────────────────────────────────────────────────────
    println!("\n=== Driver sysfs ({}) ===\n", SMU_DRV_PATH);

    let driver_ok = match driver::list_sysfs_files() {
        Ok(files) => {
            for (name, info) in &files {
                match info {
                    driver::SysfsFileInfo::Text(text) => {
                        println!("  {:<20} {}", name, text);
                    }
                    driver::SysfsFileInfo::Binary(data) => {
                        if let Some(decoded) = driver::decode_binary_value(name, data) {
                            println!("  {:<20} {}", name, decoded);
                        } else {
                            let hex: Vec<String> =
                                data.iter().take(32).map(|b| format!("{:02x}", b)).collect();
                            let suffix = if data.len() > 32 { " ..." } else { "" };
                            println!(
                                "  {:<20} ({} bytes) {}{}",
                                name,
                                data.len(),
                                hex.join(" "),
                                suffix
                            );
                        }
                    }
                    driver::SysfsFileInfo::Error(e) => {
                        println!("  {:<20} <error: {}>", name, e);
                    }
                }
            }
            true
        }
        Err(e) => {
            println!("  Not available: {}", e);
            false
        }
    };

    // ── Parsed SMU Info ──────────────────────────────────────────────────
    let smu_info = if driver_ok {
        println!("\n=== SMU Info (parsed) ===\n");
        match driver::read_info() {
            Ok(info) => {
                println!("  SMU Firmware:     {}", info.version);
                println!("  CPU Codename:     {}", info.codename.as_str());
                println!("  Generation:       {}", info.codename.generation());
                println!("  Driver Version:   {}", info.drv_version);
                println!(
                    "  PM Table Version: 0x{:06X}{}",
                    info.pm_table_version,
                    if pmtable::is_experimental(info.pm_table_version) {
                        " [EXPERIMENTAL]"
                    } else {
                        ""
                    }
                );
                println!("  PM Table Size:    {} bytes", info.pm_table_size);
                if let Some(mp1) = info.mp1_if_version {
                    println!("  MP1 IF Version:   {}", mp1);
                }
                Some(info)
            }
            Err(e) => {
                println!("  Failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    // ── Direct Register Probes ───────────────────────────────────────────
    println!("\n=== Direct Register Probes ===\n");

    let is_zen5 = smu_info
        .as_ref()
        .map(|i| i.codename.is_zen5())
        .unwrap_or(false);

    let smn_reader = smn::SmnReader::new(is_zen5);
    if smn_reader.is_available() {
        match smn_reader.read_tctl() {
            Ok(temp) => println!("  Tctl (SMN):       {:.1} C", temp),
            Err(e) => println!("  Tctl (SMN):       error - {}", e),
        }

        let max_ccds: u32 = if is_zen5 { 2 } else { 8 };
        for i in 0..max_ccds {
            match smn_reader.read_ccd_temp(i) {
                Ok(Some(temp)) => println!("  CCD{} Temp (SMN):  {:.1} C", i, temp),
                Ok(None) => {}
                Err(_) => break,
            }
        }

        if is_zen5 {
            if let Ok(Some(v)) = smn_reader.read_core_voltage() {
                println!("  Core VID (SVI3):  {:.4} V", v);
            }
            if let Ok(Some(v)) = smn_reader.read_soc_voltage() {
                println!("  SoC VID (SVI3):   {:.4} V", v);
            }
        }
    } else {
        println!("  SMN: not accessible (need root + PCI config access)");
    }

    if let Ok(mut rapl) = msr::RaplReader::new() {
        println!("  RAPL unit:        {:.4} uJ/tick", rapl.energy_unit_uj());
        let _ = rapl.read_package_power();
        std::thread::sleep(std::time::Duration::from_millis(250));
        if let Ok(Some(power)) = rapl.read_package_power() {
            println!("  Pkg Power (RAPL): {:.2} W", power);
        }
        if let Ok(Some(power)) = rapl.read_core_power() {
            println!("  Core Power (RAPL):{:.2} W", power);
        } else {
            println!("  Core Power (RAPL): N/A (expected on Zen 5 desktop)");
        }
    } else {
        println!("  RAPL: not accessible (need root + msr module)");
    }

    // ── PM Table Field Scan ──────────────────────────────────────────────
    if driver_ok {
        println!("\n=== PM Table Field Scan ===\n");

        match driver::read_pm_table(true) {
            Ok(pm_data) => {
                let named = pmtable::dump_named_fields(&pm_data);

                if named.is_empty() {
                    println!("  No mapped fields for version 0x{:06X}", pm_data.version);
                } else {
                    for (name, value, unit) in &named {
                        println!("  {:<20} {:>10.2} {}", name, value, unit);
                    }
                }

                let known_max_offset = pmtable::get_field_map(pm_data.version)
                    .map(|fields| fields.iter().map(|f| f.offset).max().unwrap_or(0))
                    .unwrap_or(0);

                let mut unmapped_count = 0;
                let scan_start = if known_max_offset > 0 {
                    known_max_offset + 4
                } else {
                    0
                };

                for i in (scan_start..pm_data.size()).step_by(4) {
                    if let Some(val) = pm_data.read_f32(i)
                        && val.is_finite()
                        && val.abs() > 0.01
                        && val.abs() < 100_000.0
                    {
                        unmapped_count += 1;
                    }
                }

                if unmapped_count > 0 {
                    println!(
                        "\n  {} unmapped non-zero f32 values beyond offset 0x{:03X}",
                        unmapped_count, scan_start
                    );
                    println!("  Use `zen smu pm-table -f --raw` to inspect them");
                }

                if pmtable::has_per_core_fields(pm_data.version) {
                    let metrics = pmtable::parse_pm_table(&pm_data);
                    if !metrics.per_core.is_empty() {
                        println!("\n  Per-core ({} cores):", metrics.per_core.len());
                        for core in &metrics.per_core {
                            println!(
                                "    Core {:>2}: {:>7.0} MHz  {:>5.1} W  {:>5.1}% active",
                                core.core_id,
                                core.frequency_mhz.unwrap_or(0.0),
                                core.power_w.unwrap_or(0.0),
                                core.activity_pct.unwrap_or(0.0),
                            );
                        }
                    }
                }
            }
            Err(e) => {
                println!("  PM table read failed: {}", e);
            }
        }
    }

    println!();
    Ok(())
}

fn pm_table(force: bool, raw: bool, update_interval: u64) -> Result<()> {
    let running = setup_signal_handler();

    if update_interval > 0 {
        println!(
            "Monitoring PM table every {} seconds (Ctrl+C to stop)...\n",
            update_interval
        );
        while running.load(Ordering::Relaxed) {
            print!("\x1B[2J\x1B[1;1H");

            if let Err(e) = display_pm_table(force, raw) {
                eprintln!("Error: {}", e);
                if !force {
                    return Err(e);
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(update_interval));
        }
        println!("\nStopped.");
        Ok(())
    } else {
        display_pm_table(force, raw)
    }
}

fn monitor(interval: u64) -> Result<()> {
    let running = setup_signal_handler();

    let cpu_model = driver::read_cpu_model().unwrap_or_else(|| "Unknown".to_string());
    let topo = driver::read_cpu_topology();
    let smu_info = driver::read_info().ok();

    let is_zen5 = smu_info
        .as_ref()
        .map(|i| i.codename.is_zen5())
        .unwrap_or(false);

    let smn_reader = smn::SmnReader::new(is_zen5);
    let smn_available = smn_reader.is_available();

    let mut rapl_reader = msr::RaplReader::new().ok();

    if let Some(ref mut rapl) = rapl_reader {
        let _ = rapl.read_package_power();
        let _ = rapl.read_core_power();
    }

    std::thread::sleep(std::time::Duration::from_secs(interval));

    while running.load(Ordering::Relaxed) {
        print!("\x1B[2J\x1B[1;1H");

        let metrics = smu::read_metrics(
            if smn_available { Some(&smn_reader) } else { None },
            rapl_reader.as_mut(),
        );

        let mut ccd_temps: Vec<Option<f64>> = metrics.ccd_temps_c.clone();
        if ccd_temps.is_empty() && smn_available {
            let max_ccds: u32 = if is_zen5 { 2 } else { 8 };
            if let Ok(temps) = smn_reader.read_all_ccd_temps(max_ccds) {
                ccd_temps = temps;
            }
        }

        display_monitor(&cpu_model, &topo, &smu_info, &metrics, &ccd_temps);
        std::thread::sleep(std::time::Duration::from_secs(interval));
    }

    println!("\nStopped.");
    Ok(())
}

// =============================================================================
// Display helpers
// =============================================================================

fn display_pm_table(force: bool, raw: bool) -> Result<()> {
    let pm_table = driver::read_pm_table(force)?;

    if raw {
        println!("=== PM Table Raw Dump ({} bytes) ===", pm_table.size());
        println!("Version: 0x{:X}\n", pm_table.version);

        for (i, chunk) in pm_table.data.chunks(16).enumerate() {
            print!("{:04x}: ", i * 16);
            for byte in chunk {
                print!("{:02x} ", byte);
            }
            println!();
        }

        println!("\n=== Notable Float Values ===");
        for i in 0..(pm_table.size() / 4).min(64) {
            if let Some(val) = pm_table.read_f32(i * 4)
                && val.is_finite()
                && val.abs() > 0.0001
                && val.abs() < 100000.0
            {
                println!("  Offset 0x{:04x} ({}): {:.4}", i * 4, i, val);
            }
        }
    } else {
        let named = pmtable::dump_named_fields(&pm_table);

        if named.is_empty() {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec![
                    Cell::new("PM Table Information").set_alignment(CellAlignment::Center),
                ]);

            table.add_row(vec!["Version", &format!("0x{:X}", pm_table.version)]);
            table.add_row(vec!["Size", &format!("{} bytes", pm_table.size())]);
            table.add_row(vec!["Status", "No field mapping for this version"]);

            println!();
            println!("{}", table);
            println!("\nUse --raw to see raw data for reverse engineering.");
        } else {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec![
                    Cell::new("Field").set_alignment(CellAlignment::Left),
                    Cell::new("Value").set_alignment(CellAlignment::Right),
                    Cell::new("Unit").set_alignment(CellAlignment::Left),
                ]);

            for (name, value, unit) in &named {
                table.add_row(vec![
                    Cell::new(name),
                    Cell::new(format!("{:.2}", value)).set_alignment(CellAlignment::Right),
                    Cell::new(unit),
                ]);
            }

            println!();
            println!(
                "PM Table v0x{:X} ({} bytes){}",
                pm_table.version,
                pm_table.size(),
                if pmtable::is_experimental(pm_table.version) {
                    " [EXPERIMENTAL]"
                } else {
                    ""
                }
            );
            println!("{}", table);
        }
    }

    Ok(())
}

fn display_monitor(
    cpu_model: &str,
    topo: &Option<driver::CpuTopology>,
    smu_info: &Option<SmuInfo>,
    metrics: &CpuMetrics,
    ccd_temps: &[Option<f64>],
) {
    let w = 74;
    let sep = "=".repeat(w);
    let line = "-".repeat(w);

    println!("{}", sep);
    println!(" zen monitor  [source: {}]", metrics.source);
    println!("{}", sep);
    println!(" CPU Model        {}", cpu_model);
    if let Some(info) = smu_info {
        println!(
            " Codename         {:<30} SMU {}",
            info.codename.as_str(),
            info.version
        );
    }
    if let Some(t) = topo {
        let smt = if t.smt { "SMT on" } else { "SMT off" };
        println!(
            " Cores            {} ({} threads, {})",
            t.physical_cores, t.logical_cpus, smt
        );
    }
    println!("{}", sep);

    // Per-core table
    if !metrics.per_core.is_empty() {
        println!(
            " {:>4}  {:>8}  {:>7}  {:>6}  {:>6}  {:>5}  {:>5}  {:>5}",
            "Core", "Freq", "Power", "Volt", "Temp", "C0%", "C1%", "C6%"
        );
        println!("{}", line);

        for core in &metrics.per_core {
            let freq = core
                .frequency_mhz
                .filter(|&f| f > 0.1)
                .map(|f| format!("{:.0} MHz", f))
                .unwrap_or_else(|| "Sleep".to_string());
            let power = core
                .power_w
                .map(|p| format!("{:.3} W", p))
                .unwrap_or_else(|| "-".to_string());
            let volt = core
                .voltage_v
                .filter(|&v| v > 0.1)
                .map(|v| format!("{:.3}V", v))
                .unwrap_or_else(|| "-".to_string());
            let temp = core
                .temp_c
                .filter(|&t| t > 0.1)
                .map(|t| format!("{:.1}C", t))
                .unwrap_or_else(|| "-".to_string());
            let c0 = fmt_opt(core.c0_pct);
            let c1 = fmt_opt(core.cc1_pct);
            let c6 = fmt_opt(core.cc6_pct);

            println!(
                " {:>4}  {:>8}  {:>7}  {:>6}  {:>6}  {:>5}  {:>5}  {:>5}",
                core.core_id, freq, power, volt, temp, c0, c1, c6
            );
        }
        println!("{}", line);
    }

    // Power & Voltage
    println!();
    let mut has_power = false;

    if let Some(f) = metrics.peak_core_freq_mhz {
        println!(" Peak Core Freq   {:.0} MHz", f);
        has_power = true;
    }
    if let Some(t) = metrics.tctl_temp_c {
        let peak = metrics
            .tjmax_c
            .map(|tj| format!("  (TjMax: {:.0} C)", tj))
            .unwrap_or_default();
        println!(" Temperature      {:.1} C{}", t, peak);
        has_power = true;
    }
    for (i, temp) in ccd_temps.iter().enumerate() {
        if let Some(t) = temp {
            println!(" CCD{} Temp        {:.1} C", i, t);
            has_power = true;
        }
    }

    if let Some(p) = metrics.package_power_w.or(metrics.core_power_w) {
        println!(" Package Power    {:.2} W", p);
        has_power = true;
    }
    if metrics.package_power_w.is_some()
        && let Some(p) = metrics.core_power_w
    {
        println!(" Core Power       {:.2} W", p);
    }
    if let Some(p) = metrics.soc_power_w {
        println!(" SoC Power        {:.2} W", p);
        has_power = true;
    }

    if let Some(v) = metrics.peak_voltage_v.or(metrics.core_voltage_v) {
        println!(" Core Voltage     {:.4} V", v);
        has_power = true;
    }
    if let Some(v) = metrics.avg_core_voltage_v {
        println!(" Avg Core Voltage {:.4} V", v);
    }
    if let Some(v) = metrics.soc_voltage_v {
        println!(" SoC Voltage      {:.4} V", v);
    }

    if has_power {
        println!();
    }

    // PBO Limits
    let has_pbo = metrics.ppt_limit_w.is_some()
        || metrics.tdc_limit_a.is_some()
        || metrics.edc_limit_a.is_some();

    if has_pbo {
        println!(
            " {:<18} {:>8} {:>8} {:>6}",
            "Limit", "Value", "Max", "Use%"
        );
        println!("{}", line);

        if let (Some(limit), Some(current)) = (metrics.ppt_limit_w, metrics.ppt_current_w) {
            let pct = if limit > 0.0 {
                current / limit * 100.0
            } else {
                0.0
            };
            println!(
                " {:<18} {:>7.1}W {:>7.1}W {:>5.1}%",
                "PPT", current, limit, pct
            );
        }
        if let (Some(limit), Some(current)) = (metrics.tdc_limit_a, metrics.tdc_current_a) {
            let pct = if limit > 0.0 {
                current / limit * 100.0
            } else {
                0.0
            };
            println!(
                " {:<18} {:>7.1}A {:>7.1}A {:>5.1}%",
                "TDC", current, limit, pct
            );
        }
        if let (Some(limit), Some(current)) = (metrics.edc_limit_a, metrics.edc_current_a) {
            let pct = if limit > 0.0 {
                current / limit * 100.0
            } else {
                0.0
            };
            println!(
                " {:<18} {:>7.1}A {:>7.1}A {:>5.1}%",
                "EDC", current, limit, pct
            );
        }
        println!();
    }

    // Clocks
    let has_clocks = metrics.fclk_mhz.is_some()
        || metrics.uclk_mhz.is_some()
        || metrics.mclk_mhz.is_some();

    if has_clocks {
        let coupled = match (metrics.uclk_mhz, metrics.mclk_mhz) {
            (Some(u), Some(m)) if (u - m).abs() < 1.0 => "Coupled",
            (Some(_), Some(_)) => "Decoupled",
            _ => "Unknown",
        };
        println!(" Memory Mode      {}", coupled);

        if let Some(f) = metrics.fclk_mhz {
            let avg = metrics
                .fclk_avg_mhz
                .map(|a| format!(" (avg: {:.0})", a))
                .unwrap_or_default();
            println!(" Fabric Clock     {:.0} MHz{}", f, avg);
        }
        if let Some(u) = metrics.uclk_mhz {
            println!(" Uncore Clock     {:.0} MHz", u);
        }
        if let Some(m) = metrics.mclk_mhz {
            println!(" Memory Clock     {:.0} MHz", m);
        }
        if let Some(v) = metrics.vddp_v {
            println!(" cLDO_VDDP        {:.4} V", v);
        }
        if let Some(v) = metrics.vddg_v {
            println!(" cLDO_VDDG        {:.4} V", v);
        }
    }

    println!("{}", sep);
}

// =============================================================================
// Utilities
// =============================================================================

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|v| format!("{:.1}", v))
        .unwrap_or_else(|| "-".to_string())
}

fn setup_signal_handler() -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .ok();

    running
}
