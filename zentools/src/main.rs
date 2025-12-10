//! ZenTools - Unified AMD Ryzen management tool
//!
//! Combines EPP (Energy Performance Preference) and SMU (System Management Unit)
//! management into a single CLI tool.

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Cell, CellAlignment, ContentArrangement, Table};
use zen_epp::{EppManager, EppProfile};
use zen_smu::{SmuManager, SMU_DRV_PATH};

/// ZenTools - AMD Ryzen CPU management utility
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(after_help = get_extended_help())]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Shorthand for EPP performance level (0-3)
    #[arg(short = 'p', value_name = "LEVEL", global = true)]
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

#[derive(Subcommand, Debug)]
enum SmuCommands {
    /// Show SMU information (version, codename, etc.)
    Info {
        /// Show verbose information
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

    /// Check if SMU driver is loaded
    Check,

    /// Debug: Show all available sysfs files and their contents
    Debug,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Handle global shorthand flags
    if cli.show_epp {
        return handle_epp_show();
    }

    if let Some(level) = cli.perf_level {
        return handle_epp_level(level);
    }

    // Handle subcommands
    match cli.command {
        Some(Commands::Epp { command }) => handle_epp_command(command),
        Some(Commands::Smu { command }) => handle_smu_command(command),
        None => {
            // No command provided, show help
            Cli::command().print_help()?;
            Ok(())
        }
    }
}

// ============================================================================
// EPP Command Handlers
// ============================================================================

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

    // Group by profile for cleaner output
    let mut by_profile: std::collections::HashMap<&str, Vec<u32>> =
        std::collections::HashMap::new();

    for info in &cpu_infos {
        by_profile
            .entry(info.profile.as_str())
            .or_default()
            .push(info.cpu_num);
    }

    // Display grouped
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
    println!("✓ Successfully applied to {} CPUs", manager.cpu_count());

    Ok(())
}

fn handle_epp_level(level: u8) -> Result<()> {
    let profile = EppProfile::from_level(level)
        .ok_or_else(|| anyhow::anyhow!("Invalid level: {}. Must be 0-3.", level))?;

    handle_epp_set(profile)
}

// ============================================================================
// SMU Command Handlers
// ============================================================================

fn handle_smu_command(command: SmuCommands) -> Result<()> {
    match command {
        SmuCommands::Info { verbose } => handle_smu_info(verbose),
        SmuCommands::PmTable {
            force,
            raw,
            update,
        } => handle_smu_pm_table(force, raw, update),
        SmuCommands::Check => handle_smu_check(),
        SmuCommands::Debug => handle_smu_debug(),
    }
}

fn handle_smu_check() -> Result<()> {
    println!("Checking SMU driver...");

    match SmuManager::check_driver() {
        Ok(_) => {
            println!("✓ SMU driver is loaded and accessible");
            println!("  Path: {}", SMU_DRV_PATH);
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ SMU driver check failed: {}", e);
            eprintln!("\nTo load the driver:");
            eprintln!("  sudo modprobe ryzen_smu");
            eprintln!("\nOr install from: https://github.com/amkillam/ryzen_smu");
            Err(e.into())
        }
    }
}

fn handle_smu_debug() -> Result<()> {
    use std::fs;
    use std::path::Path;

    println!("=== SMU Driver Debug Information ===");
    println!();

    let smu_path = Path::new(SMU_DRV_PATH);

    if !smu_path.exists() {
        eprintln!("✗ SMU driver path does not exist: {}", SMU_DRV_PATH);
        return Ok(());
    }

    println!("✓ SMU driver path exists: {}", SMU_DRV_PATH);
    println!();

    // List all files in the directory
    println!("Available sysfs files:");
    let entries = fs::read_dir(smu_path)?;
    let mut files: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    files.sort_by_key(|e| e.file_name());

    for entry in &files {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        println!("  - {}", name_str);
    }

    println!();
    println!("=== File Contents ===");
    println!();

    // Read and display contents of each file
    for entry in &files {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_file() {
            print!("[{}]: ", name_str);

            match fs::read(&path) {
                Ok(data) => {
                    // Try to display as text first
                    if let Ok(text) = String::from_utf8(data.clone()) {
                        let trimmed = text.trim();
                        if trimmed.len() < 100 && !trimmed.contains('\0') {
                            println!("{}", trimmed);
                            continue;
                        }
                    }

                    // Display size for binary/large files
                    println!("<binary data, {} bytes>", data.len());

                    // Show hex for small binary files
                    if data.len() <= 16 {
                        print!("  hex: ");
                        for byte in data {
                            print!("{:02x} ", byte);
                        }
                        println!();
                    }
                }
                Err(e) => {
                    println!("<error reading: {}>", e);
                }
            }
        }
    }

    println!();
    println!("=== Parsed SMU Info ===");
    println!();

    match SmuManager::read_info() {
        Ok(info) => {
            println!("SMU Version:      {}", info.version);
            println!("Codename:         {} (raw value from parsing)", info.codename.as_str());
            println!("Driver Version:   {}", info.drv_version);
            println!("PM Table Version: 0x{:X}", info.pm_table_version);
            println!("PM Table Size:    {} bytes", info.pm_table_size);
            if let Some(mp1) = info.mp1_if_version {
                println!("MP1 IF Version:   {}", mp1);
            }
        }
        Err(e) => {
            eprintln!("Error reading SMU info: {}", e);
        }
    }

    Ok(())
}

fn handle_smu_info(verbose: bool) -> Result<()> {
    let info = SmuManager::read_info()?;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![Cell::new("AMD Ryzen SMU Information")
            .set_alignment(CellAlignment::Center)]);

    table.add_row(vec!["SMU Version", &info.version.to_string()]);
    table.add_row(vec!["Codename", info.codename.as_str()]);
    table.add_row(vec!["Driver Version", &info.drv_version]);
    table.add_row(vec![
        "PM Table Version",
        &format!("0x{:X}", info.pm_table_version),
    ]);
    table.add_row(vec!["PM Table Size", &info.pm_table_size.to_string()]);

    if let Some(mp1_ver) = info.mp1_if_version {
        table.add_row(vec!["MP1 IF Version", &mp1_ver.to_string()]);
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
    if update_interval > 0 {
        // Continuous monitoring mode
        println!("Monitoring PM table every {} seconds (Ctrl+C to stop)...\n", update_interval);
        loop {
            // Clear screen
            print!("\x1B[2J\x1B[1;1H");

            if let Err(e) = display_pm_table(force, raw) {
                eprintln!("Error: {}", e);
                if !force {
                    return Err(e);
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(update_interval));
        }
    } else {
        // Single read
        display_pm_table(force, raw)
    }
}

fn display_pm_table(force: bool, raw: bool) -> Result<()> {
    let pm_table = SmuManager::read_pm_table(force)?;

    if raw {
        println!("\n=== PM Table Raw Dump ({} bytes) ===", pm_table.size());
        println!("Version: 0x{:X}\n", pm_table.version);

        // Hex dump
        for (i, chunk) in pm_table.data.chunks(16).enumerate() {
            print!("{:04x}: ", i * 16);
            for byte in chunk {
                print!("{:02x} ", byte);
            }
            println!();
        }

        // Float values
        println!("\n=== Notable Float Values ===");
        for i in 0..(pm_table.size() / 4).min(64) {
            if let Some(val) = pm_table.read_f32(i * 4) {
                if val.is_finite() && val.abs() > 0.0001 && val.abs() < 100000.0 {
                    println!("  Offset 0x{:04x} ({}): {:.4}", i * 4, i, val);
                }
            }
        }

        // U32 values
        println!("\n=== Notable U32 Values ===");
        for i in 0..(pm_table.size() / 4).min(64) {
            if let Some(val) = pm_table.read_u32(i * 4) {
                if val > 0 && val < 0x7FFFFFFF {
                    println!("  Offset 0x{:04x} ({}): {} (0x{:08x})", i * 4, i, val, val);
                }
            }
        }
    } else {
        let metrics = SmuManager::parse_basic_metrics(&pm_table)?;

        println!("\n╭────────────────────────────────────────────────────────╮");
        println!("│                  PM Table Information                  │");
        println!("├────────────────────────────────────────────────────────┤");
        println!("│ Version          : 0x{:<33X} │", metrics.table_version);
        println!("│ Size             : {:<36} │", metrics.table_size);
        println!("╰────────────────────────────────────────────────────────╯");

        if force {
            println!("\nNote: PM table version not fully supported yet.");
            println!("Use --raw to see raw data for reverse engineering.");
        }
    }

    Ok(())
}

// ============================================================================
// Help Text
// ============================================================================

fn get_extended_help() -> &'static str {
    r#"
EXAMPLES:
    # EPP Management
    zentools epp show                    # Show current EPP settings
    zentools epp performance             # Set to performance mode
    zentools -p 2                        # Quick set to level 2 (balance-power)
    zentools -s                          # Quick show EPP status

    # SMU Information
    zentools smu check                   # Check if driver is loaded
    zentools smu info                    # Show SMU information
    zentools smu info --verbose          # Verbose SMU info
    zentools smu pm-table --force        # Read PM table (force if unsupported)
    zentools smu pm-table --force --raw  # Show raw PM table data
    zentools smu pm-table -f -u 2        # Monitor PM table every 2 seconds

EPP PROFILES:
    0 / performance        - Maximum performance, higher power usage
    1 / balance-performance - Balanced, leaning toward performance (default)
    2 / balance-power      - Balanced, leaning toward power saving
    3 / power              - Maximum power saving, may limit performance

REQUIREMENTS:
    - AMD Ryzen CPU (Zen 2 or newer)
    - amd-pstate driver in 'active' mode (for EPP)
    - ryzen_smu kernel module loaded (for SMU)
    - Root/sudo access for EPP writes and SMU reads

MORE INFO:
    EPP: https://docs.kernel.org/admin-guide/pm/amd-pstate.html
    SMU: https://github.com/amkillam/ryzen_smu
"#
}
