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
    // ── Header ───────────────────────────────────────────────────────────
    let mut hdr = Table::new();
    hdr.load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Zen Monitor").set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
        ]);

    hdr.add_row(vec!["CPU", cpu_model]);
    hdr.add_row(vec!["Detection Mode", &metrics.source.to_string()]);
    if let Some(info) = smu_info {
        hdr.add_row(vec!["Codename", info.codename.as_str()]);
        hdr.add_row(vec!["SMU", &info.version.to_string()]);
    }
    if let Some(t) = topo {
        hdr.add_row(vec![
            "Topology",
            &format!(
                "{} cores / {} threads ({})",
                t.physical_cores,
                t.logical_cpus,
                if t.smt { "SMT" } else { "no SMT" }
            ),
        ]);
    }

    println!("{}", hdr);

    // ── Per-core table ───────────────────────────────────────────────────
    if !metrics.per_core.is_empty() {
        let mut cores = Table::new();
        cores
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("Core"),
                Cell::new("Freq").set_alignment(CellAlignment::Right),
                Cell::new("Power").set_alignment(CellAlignment::Right),
                Cell::new("Volt").set_alignment(CellAlignment::Right),
                Cell::new("Temp").set_alignment(CellAlignment::Right),
                Cell::new("C0%").set_alignment(CellAlignment::Right),
                Cell::new("C1%").set_alignment(CellAlignment::Right),
                Cell::new("C6%").set_alignment(CellAlignment::Right),
            ]);

        for core in &metrics.per_core {
            cores.add_row(vec![
                Cell::new(core.core_id),
                Cell::new(
                    core.frequency_mhz
                        .filter(|&f| f > 0.1)
                        .map(|f| format!("{:.0}", f))
                        .unwrap_or_else(|| "Sleep".into()),
                )
                .set_alignment(CellAlignment::Right),
                Cell::new(fmt_opt_f(core.power_w, 2)).set_alignment(CellAlignment::Right),
                Cell::new(fmt_opt_f(core.voltage_v.filter(|&v| v > 0.1), 3))
                    .set_alignment(CellAlignment::Right),
                Cell::new(fmt_opt_f(core.temp_c.filter(|&t| t > 0.1), 1))
                    .set_alignment(CellAlignment::Right),
                Cell::new(fmt_opt(core.c0_pct)).set_alignment(CellAlignment::Right),
                Cell::new(fmt_opt(core.cc1_pct)).set_alignment(CellAlignment::Right),
                Cell::new(fmt_opt(core.cc6_pct)).set_alignment(CellAlignment::Right),
            ]);
        }

        println!("{}", cores);
    }

    // ── System metrics ───────────────────────────────────────────────────
    let mut sys = Table::new();
    sys.load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Metric").set_alignment(CellAlignment::Left),
            Cell::new("Value").set_alignment(CellAlignment::Right),
            Cell::new("Metric").set_alignment(CellAlignment::Left),
            Cell::new("Value").set_alignment(CellAlignment::Right),
        ]);

    // Row 1: Temperature + Peak Freq
    let temp_str = metrics
        .tctl_temp_c
        .map(|t| {
            metrics
                .tjmax_c
                .map(|tj| format!("{:.1} / {:.0} C", t, tj))
                .unwrap_or_else(|| format!("{:.1} C", t))
        })
        .unwrap_or_else(|| "-".into());
    let freq_str = metrics
        .peak_core_freq_mhz
        .map(|f| format!("{:.0} MHz", f))
        .unwrap_or_else(|| "-".into());
    sys.add_row(vec![
        Cell::new("Tctl / TjMax"),
        Cell::new(&temp_str).set_alignment(CellAlignment::Right),
        Cell::new("Peak Freq"),
        Cell::new(&freq_str).set_alignment(CellAlignment::Right),
    ]);

    // CCD temps
    for (i, temp) in ccd_temps.iter().enumerate() {
        if let Some(t) = temp {
            sys.add_row(vec![
                Cell::new(format!("CCD{} Temp", i)),
                Cell::new(format!("{:.1} C", t)).set_alignment(CellAlignment::Right),
                Cell::new(""),
                Cell::new(""),
            ]);
        }
    }

    // Power row
    let pkg = metrics
        .package_power_w
        .or(metrics.core_power_w)
        .map(|p| format!("{:.2} W", p))
        .unwrap_or_else(|| "-".into());
    let soc = metrics
        .soc_power_w
        .map(|p| format!("{:.2} W", p))
        .unwrap_or_else(|| "-".into());
    sys.add_row(vec![
        Cell::new("Pkg Power"),
        Cell::new(&pkg).set_alignment(CellAlignment::Right),
        Cell::new("SoC Power"),
        Cell::new(&soc).set_alignment(CellAlignment::Right),
    ]);

    // Voltage row
    let core_v = metrics
        .peak_voltage_v
        .or(metrics.core_voltage_v)
        .map(|v| format!("{:.4} V", v))
        .unwrap_or_else(|| "-".into());
    let soc_v = metrics
        .soc_voltage_v
        .map(|v| format!("{:.4} V", v))
        .unwrap_or_else(|| "-".into());
    sys.add_row(vec![
        Cell::new("Core Volt"),
        Cell::new(&core_v).set_alignment(CellAlignment::Right),
        Cell::new("SoC Volt"),
        Cell::new(&soc_v).set_alignment(CellAlignment::Right),
    ]);

    println!("{}", sys);

    // ── PBO Limits ───────────────────────────────────────────────────────
    let has_pbo = metrics.ppt_limit_w.is_some()
        || metrics.tdc_limit_a.is_some()
        || metrics.edc_limit_a.is_some();

    if has_pbo {
        let mut pbo = Table::new();
        pbo.load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("Limit"),
                Cell::new("Current").set_alignment(CellAlignment::Right),
                Cell::new("Max").set_alignment(CellAlignment::Right),
                Cell::new("Use%").set_alignment(CellAlignment::Right),
            ]);

        fn pbo_row(table: &mut Table, name: &str, current: Option<f64>, limit: Option<f64>, unit: &str) {
            if let (Some(lim), Some(cur)) = (limit, current) {
                let pct = if lim > 0.0 { cur / lim * 100.0 } else { 0.0 };
                table.add_row(vec![
                    Cell::new(name),
                    Cell::new(format!("{:.1} {}", cur, unit)).set_alignment(CellAlignment::Right),
                    Cell::new(format!("{:.1} {}", lim, unit)).set_alignment(CellAlignment::Right),
                    Cell::new(format!("{:.1}%", pct)).set_alignment(CellAlignment::Right),
                ]);
            }
        }

        pbo_row(&mut pbo, "PPT", metrics.ppt_current_w, metrics.ppt_limit_w, "W");
        pbo_row(&mut pbo, "TDC", metrics.tdc_current_a, metrics.tdc_limit_a, "A");
        pbo_row(&mut pbo, "EDC", metrics.edc_current_a, metrics.edc_limit_a, "A");

        println!("{}", pbo);
    }

    // ── Clocks ───────────────────────────────────────────────────────────
    let has_clocks = metrics.fclk_mhz.is_some()
        || metrics.uclk_mhz.is_some()
        || metrics.mclk_mhz.is_some();

    if has_clocks {
        let mut clk = Table::new();
        clk.load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("Clock").set_alignment(CellAlignment::Left),
                Cell::new("Value").set_alignment(CellAlignment::Right),
                Cell::new("Clock").set_alignment(CellAlignment::Left),
                Cell::new("Value").set_alignment(CellAlignment::Right),
            ]);

        let coupled = match (metrics.uclk_mhz, metrics.mclk_mhz) {
            (Some(u), Some(m)) if (u - m).abs() < 1.0 => "Coupled",
            (Some(_), Some(_)) => "Decoupled",
            _ => "-",
        };

        clk.add_row(vec![
            Cell::new("FCLK"),
            Cell::new(
                metrics
                    .fclk_mhz
                    .map(|f| format!("{:.0} MHz", f))
                    .unwrap_or_else(|| "-".into()),
            )
            .set_alignment(CellAlignment::Right),
            Cell::new("Mode"),
            Cell::new(coupled).set_alignment(CellAlignment::Right),
        ]);
        clk.add_row(vec![
            Cell::new("UCLK"),
            Cell::new(
                metrics
                    .uclk_mhz
                    .map(|u| format!("{:.0} MHz", u))
                    .unwrap_or_else(|| "-".into()),
            )
            .set_alignment(CellAlignment::Right),
            Cell::new("MCLK"),
            Cell::new(
                metrics
                    .mclk_mhz
                    .map(|m| format!("{:.0} MHz", m))
                    .unwrap_or_else(|| "-".into()),
            )
            .set_alignment(CellAlignment::Right),
        ]);

        if metrics.vddp_v.is_some() || metrics.vddg_v.is_some() {
            clk.add_row(vec![
                Cell::new("VDDP"),
                Cell::new(
                    metrics
                        .vddp_v
                        .map(|v| format!("{:.4} V", v))
                        .unwrap_or_else(|| "-".into()),
                )
                .set_alignment(CellAlignment::Right),
                Cell::new("VDDG"),
                Cell::new(
                    metrics
                        .vddg_v
                        .map(|v| format!("{:.4} V", v))
                        .unwrap_or_else(|| "-".into()),
                )
                .set_alignment(CellAlignment::Right),
            ]);
        }

        println!("{}", clk);
    }
}

// =============================================================================
// Utilities
// =============================================================================

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|v| format!("{:.1}", v))
        .unwrap_or_else(|| "-".to_string())
}

fn fmt_opt_f(v: Option<f64>, prec: usize) -> String {
    v.map(|v| format!("{:.prec$}", v, prec = prec))
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
