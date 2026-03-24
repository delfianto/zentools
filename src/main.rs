//! zen - AMD Ryzen management tool
//!
//! Single binary with busybox-style dispatch:
//! - `zen` — full CLI with all subcommands
//! - `epp` — EPP management only (symlink to zen)
//! - `smu` — SMU monitoring only (symlink to zen)

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Cell, CellAlignment, ContentArrangement, Table};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use zentools::epp::{EppManager, EppProfile};
use zentools::smu::{self, driver, msr, pmtable, smn, CpuMetrics, SMU_DRV_PATH};

// =============================================================================
// CLI Definitions
// =============================================================================

/// zen - AMD Ryzen CPU management utility
#[derive(Parser, Debug)]
#[command(name = "zen", author, version, about, long_about = None)]
#[command(after_help = get_extended_help())]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// EPP performance level: -p0 -p1 -p2 -p3
    #[arg(short = 'p', value_name = "0-3", global = true)]
    perf_level: Option<u8>,

    /// Show EPP status
    #[arg(short = 's', long, global = true)]
    show_epp: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage Energy Performance Preference (EPP)
    Epp {
        #[command(subcommand)]
        command: EppCommands,
    },

    /// Read System Management Unit (SMU) information
    Smu {
        #[command(subcommand)]
        command: SmuCommands,
    },
}

#[derive(Subcommand, Debug)]
enum EppCommands {
    /// Show current EPP settings for all CPUs
    Show,
    /// Set EPP profile to 'performance'
    Performance,
    /// Set EPP profile to 'balance-performance'
    BalancePerformance,
    /// Set EPP profile to 'balance-power'
    BalancePower,
    /// Set EPP profile to 'power'
    Power,
    /// Set EPP profile by level (0=performance, 1=balance-perf, 2=balance-power, 3=power)
    Level { level: u8 },
}

/// EPP-only CLI (symlink: epp -> zen)
#[derive(Parser, Debug)]
#[command(name = "epp", about = "AMD Ryzen EPP management")]
struct EppCli {
    #[command(subcommand)]
    command: EppCommands,
}

#[derive(Subcommand, Debug)]
enum SmuCommands {
    /// Show SMU information (version, codename, etc.)
    Info {
        #[arg(short, long)]
        verbose: bool,
    },
    /// Read and display PM table
    PmTable {
        /// Force reading even if version unsupported
        #[arg(short, long)]
        force: bool,
        /// Show raw hex dump
        #[arg(short, long)]
        raw: bool,
        /// Update continuously (seconds between updates, 0=once)
        #[arg(short, long, default_value = "0")]
        update: u64,
    },
    /// Live CPU monitoring (temperature, power, voltage)
    Monitor {
        /// Update interval in seconds
        #[arg(short, long, default_value = "1")]
        interval: u64,
    },
    /// Check if SMU driver is loaded
    Check,
    /// Debug: Show all available sysfs files and their contents
    Debug,
}

/// SMU-only CLI (symlink: smu -> zen)
#[derive(Parser, Debug)]
#[command(name = "smu", about = "AMD Ryzen SMU monitoring")]
struct SmuCli {
    #[command(subcommand)]
    command: SmuCommands,
}

// =============================================================================
// Busybox Dispatch
// =============================================================================

fn main() {
    let binary_name = std::env::args()
        .next()
        .and_then(|s| {
            std::path::Path::new(&s)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
        })
        .unwrap_or_default();

    let result = match binary_name.as_str() {
        "epp" => run_epp_personality(),
        "smu" => run_smu_personality(),
        _ => run_full(),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_epp_personality() -> Result<()> {
    let cli = EppCli::parse();
    handle_epp_command(cli.command)
}

fn run_smu_personality() -> Result<()> {
    let cli = SmuCli::parse();
    handle_smu_command(cli.command)
}

fn run_full() -> Result<()> {
    let cli = Cli::parse();

    if cli.show_epp {
        return handle_epp_show();
    }

    if let Some(level) = cli.perf_level {
        return handle_epp_level(level);
    }

    match cli.command {
        Some(Commands::Epp { command }) => handle_epp_command(command),
        Some(Commands::Smu { command }) => handle_smu_command(command),
        None => {
            Cli::command().print_help()?;
            Ok(())
        }
    }
}

// =============================================================================
// EPP Handlers
// =============================================================================

fn handle_epp_command(command: EppCommands) -> Result<()> {
    match command {
        EppCommands::Show => handle_epp_show(),
        EppCommands::Performance => handle_epp_set(EppProfile::Performance),
        EppCommands::BalancePerformance => handle_epp_set(EppProfile::BalancePerformance),
        EppCommands::BalancePower => handle_epp_set(EppProfile::BalancePower),
        EppCommands::Power => handle_epp_set(EppProfile::Power),
        EppCommands::Level { level } => handle_epp_level(level),
    }
}

fn handle_epp_show() -> Result<()> {
    let manager = EppManager::new()?;
    let cpu_infos = manager.read_all()?;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![Cell::new(format!(
            "AMD EPP Status ({} CPUs)",
            manager.cpu_count()
        ))
        .set_alignment(CellAlignment::Center)]);

    let mut by_profile: std::collections::HashMap<&str, Vec<u32>> =
        std::collections::HashMap::new();

    for info in &cpu_infos {
        by_profile
            .entry(info.profile.as_str())
            .or_default()
            .push(info.cpu_num);
    }

    for profile in EppProfile::all() {
        if let Some(cpus) = by_profile.get(profile.as_str()) {
            let cpu_list = if cpus.len() > 8 {
                format!("{} CPUs: 0-{}", cpus.len(), cpus.len() - 1)
            } else {
                format!("CPUs: {:?}", cpus)
            };

            table.add_row(vec![profile.as_str(), &cpu_list]);
            table.add_row(vec!["", profile.description()]);
        }
    }

    println!();
    println!("{}", table);
    println!();

    Ok(())
}

fn handle_epp_set(profile: EppProfile) -> Result<()> {
    let manager = EppManager::new()?;

    println!("Setting EPP to '{}' for all CPUs...", profile.as_str());
    manager.apply_profile(profile)?;
    println!("Successfully applied to {} CPUs", manager.cpu_count());

    Ok(())
}

fn handle_epp_level(level: u8) -> Result<()> {
    let profile = EppProfile::from_level(level)
        .ok_or_else(|| anyhow::anyhow!("Invalid level: {}. Must be 0-3.", level))?;

    handle_epp_set(profile)
}

// =============================================================================
// SMU Handlers
// =============================================================================

fn handle_smu_command(command: SmuCommands) -> Result<()> {
    match command {
        SmuCommands::Info { verbose } => handle_smu_info(verbose),
        SmuCommands::PmTable { force, raw, update } => handle_smu_pm_table(force, raw, update),
        SmuCommands::Monitor { interval } => handle_smu_monitor(interval),
        SmuCommands::Check => handle_smu_check(),
        SmuCommands::Debug => handle_smu_debug(),
    }
}

fn handle_smu_check() -> Result<()> {
    println!("Checking available data sources...\n");

    // Check ryzen_smu driver
    match driver::check_driver() {
        Ok(_) => {
            println!("[OK] ryzen_smu driver loaded at {}", SMU_DRV_PATH);
        }
        Err(e) => {
            println!("[--] ryzen_smu driver: {}", e);
        }
    }

    // Check MSR access
    if msr::RaplReader::is_available() {
        println!("[OK] MSR access available (RAPL power monitoring)");
    } else {
        println!("[--] MSR access unavailable (try: sudo modprobe msr)");
    }

    // Check SMN access
    let smn = smn::SmnReader::new(false);
    if smn.is_available() {
        println!("[OK] SMN access available (temperature monitoring)");
    } else {
        println!("[--] SMN access unavailable (requires root)");
    }

    println!();
    Ok(())
}

fn handle_smu_debug() -> Result<()> {
    // ── Section 1: CPU Info ──────────────────────────────────────────────
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

    // ── Section 2: Driver sysfs ──────────────────────────────────────────
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

    // ── Section 3: Parsed SMU Info ───────────────────────────────────────
    let smu_info = if driver_ok {
        println!("\n=== SMU Info (parsed) ===\n");
        match driver::read_info() {
            Ok(info) => {
                println!("  SMU Firmware:     {}", info.version);
                println!("  CPU Codename:     {}", info.codename.as_str());
                println!("  Generation:       {}", info.codename.generation());
                println!("  Driver Version:   {}", info.drv_version);
                println!("  PM Table Version: 0x{:06X}{}", info.pm_table_version,
                    if pmtable::is_experimental(info.pm_table_version) { " [EXPERIMENTAL]" } else { "" }
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

    // ── Section 4: Direct Register Probes ────────────────────────────────
    println!("\n=== Direct Register Probes ===\n");

    let is_zen5 = smu_info
        .as_ref()
        .map(|i| i.codename.is_zen5())
        .unwrap_or(false);

    // SMN temperature
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
                Ok(None) => {} // sensor not present
                Err(_) => break,
            }
        }

        // SVI voltage (Zen 5 only)
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

    // RAPL power
    if let Ok(mut rapl) = msr::RaplReader::new() {
        println!("  RAPL unit:        {:.4} uJ/tick", rapl.energy_unit_uj());
        // Prime the counters
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

    // ── Section 5: PM Table Field Scan ───────────────────────────────────
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

                // Scan for unmapped non-zero f32 values beyond the known fields
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

                // Per-core data if available
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

fn handle_smu_info(verbose: bool) -> Result<()> {
    let info = driver::read_info()?;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("AMD Ryzen SMU Information").set_alignment(CellAlignment::Center)
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

    let experimental = pmtable::is_experimental(info.pm_table_version);
    if experimental {
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

fn handle_smu_pm_table(force: bool, raw: bool, update_interval: u64) -> Result<()> {
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

fn handle_smu_monitor(interval: u64) -> Result<()> {
    let running = setup_signal_handler();

    // Gather system info for header
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

    // Prime RAPL counters
    if let Some(ref mut rapl) = rapl_reader {
        let _ = rapl.read_package_power();
        let _ = rapl.read_core_power();
    }

    // Wait one interval before first display
    std::thread::sleep(std::time::Duration::from_secs(interval));

    while running.load(Ordering::Relaxed) {
        print!("\x1B[2J\x1B[1;1H");

        let metrics = smu::read_metrics(
            if smn_available { Some(&smn_reader) } else { None },
            rapl_reader.as_mut(),
        );

        // Read CCD temps directly (not in read_metrics by default for all CCDs)
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
// Display Functions
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
        // Show named fields if available
        let named = pmtable::dump_named_fields(&pm_table);

        if named.is_empty() {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec![
                    Cell::new("PM Table Information").set_alignment(CellAlignment::Center)
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
    smu_info: &Option<smu::SmuInfo>,
    metrics: &CpuMetrics,
    ccd_temps: &[Option<f64>],
) {
    let w = 74; // display width
    let sep = "=".repeat(w);
    let line = "-".repeat(w);

    // ── Header ───────────────────────────────────────────────────────────
    println!("{}", sep);
    println!(" zen monitor  [source: {}]", metrics.source);
    println!("{}", sep);
    println!(" CPU Model        {}", cpu_model);
    if let Some(info) = smu_info {
        println!(" Codename         {:<30} SMU {}", info.codename.as_str(), info.version);
    }
    if let Some(t) = topo {
        let smt = if t.smt { "SMT on" } else { "SMT off" };
        println!(" Cores            {} ({} threads, {})", t.physical_cores, t.logical_cpus, smt);
    }
    println!("{}", sep);

    // ── Per-core table ───────────────────────────────────────────────────
    if !metrics.per_core.is_empty() {
        println!(
            " {:>4}  {:>8}  {:>7}  {:>6}  {:>6}  {:>5}  {:>5}  {:>5}",
            "Core", "Freq", "Power", "Volt", "Temp", "C0%", "C1%", "C6%"
        );
        println!("{}", line);

        for core in &metrics.per_core {
            let freq_str = core.frequency_mhz
                .filter(|&f| f > 0.1)
                .map(|f| format!("{:.0} MHz", f))
                .unwrap_or_else(|| "Sleep".to_string());

            let power_str = core.power_w
                .map(|p| format!("{:.3} W", p))
                .unwrap_or_else(|| "-".to_string());

            let volt_str = core.voltage_v
                .filter(|&v| v > 0.1)
                .map(|v| format!("{:.3}V", v))
                .unwrap_or_else(|| "-".to_string());

            let temp_str = core.temp_c
                .filter(|&t| t > 0.1)
                .map(|t| format!("{:.1}C", t))
                .unwrap_or_else(|| "-".to_string());

            let c0 = core.c0_pct.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "-".to_string());
            let c1 = core.cc1_pct.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "-".to_string());
            let c6 = core.cc6_pct.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "-".to_string());

            println!(
                " {:>4}  {:>8}  {:>7}  {:>6}  {:>6}  {:>5}  {:>5}  {:>5}",
                core.core_id, freq_str, power_str, volt_str, temp_str, c0, c1, c6
            );
        }
        println!("{}", line);
    }

    // ── Power & Voltage ──────────────────────────────────────────────────
    println!();
    let mut has_power = false;

    if let Some(f) = metrics.peak_core_freq_mhz {
        println!(" Peak Core Freq   {:.0} MHz", f);
        has_power = true;
    }
    if let Some(t) = metrics.tctl_temp_c {
        let peak_str = metrics.tjmax_c
            .map(|tj| format!("  (TjMax: {:.0} C)", tj))
            .unwrap_or_default();
        println!(" Temperature      {:.1} C{}", t, peak_str);
        has_power = true;
    }

    // CCD temperatures
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

    // ── PBO Limits ───────────────────────────────────────────────────────
    let has_pbo = metrics.ppt_limit_w.is_some()
        || metrics.tdc_limit_a.is_some()
        || metrics.edc_limit_a.is_some();

    if has_pbo {
        println!(" {:<18} {:>8} {:>8} {:>6}", "Limit", "Value", "Max", "Use%");
        println!("{}", line);

        if let (Some(limit), Some(current)) = (metrics.ppt_limit_w, metrics.ppt_current_w) {
            let pct = if limit > 0.0 { current / limit * 100.0 } else { 0.0 };
            println!(" {:<18} {:>7.1}W {:>7.1}W {:>5.1}%", "PPT", current, limit, pct);
        }
        if let (Some(limit), Some(current)) = (metrics.tdc_limit_a, metrics.tdc_current_a) {
            let pct = if limit > 0.0 { current / limit * 100.0 } else { 0.0 };
            println!(" {:<18} {:>7.1}A {:>7.1}A {:>5.1}%", "TDC", current, limit, pct);
        }
        if let (Some(limit), Some(current)) = (metrics.edc_limit_a, metrics.edc_current_a) {
            let pct = if limit > 0.0 { current / limit * 100.0 } else { 0.0 };
            println!(" {:<18} {:>7.1}A {:>7.1}A {:>5.1}%", "EDC", current, limit, pct);
        }
        println!();
    }

    // ── Clocks ───────────────────────────────────────────────────────────
    let has_clocks = metrics.fclk_mhz.is_some()
        || metrics.uclk_mhz.is_some()
        || metrics.mclk_mhz.is_some();

    if has_clocks {
        // Detect coupled mode
        let coupled = match (metrics.uclk_mhz, metrics.mclk_mhz) {
            (Some(u), Some(m)) if (u - m).abs() < 1.0 => "Coupled",
            (Some(_), Some(_)) => "Decoupled",
            _ => "Unknown",
        };
        println!(" Memory Mode      {}", coupled);

        if let Some(f) = metrics.fclk_mhz {
            let avg = metrics.fclk_avg_mhz
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

fn setup_signal_handler() -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .ok(); // Ignore if handler can't be set

    running
}

fn get_extended_help() -> &'static str {
    r#"
EXAMPLES:
    # EPP Management
    zen epp show                    # Show current EPP settings
    zen epp performance             # Set to performance mode
    zen -p0                         # Quick set to performance
    zen -p2                         # Quick set to balance-power
    zen -s                          # Quick show EPP status

    # SMU Information
    zen smu check                   # Check available data sources
    zen smu info                    # Show SMU information
    zen smu monitor                 # Live CPU monitoring
    zen smu monitor -i 2            # Monitor every 2 seconds
    zen smu pm-table --force        # Read PM table (force if unsupported)
    zen smu pm-table --force --raw  # Show raw PM table data

    # Busybox-style (symlinks created by `just install`)
    epp show                        # Same as `zen epp show`
    smu info                        # Same as `zen smu info`

EPP PROFILES:
    -p0 / performance         - Maximum performance, higher power usage
    -p1 / balance-performance - Balanced, leaning toward performance (default)
    -p2 / balance-power       - Balanced, leaning toward power saving
    -p3 / power               - Maximum power saving, may limit performance

REQUIREMENTS:
    - AMD Ryzen CPU (Zen 2 or newer)
    - amd-pstate driver in 'active' mode (for EPP)
    - Root/sudo access for SMU reads and EPP writes
    - For full monitoring: ryzen_smu kernel module OR msr module loaded
"#
}
