//! zen - AMD Ryzen management tool
//!
//! Single binary with busybox-style dispatch:
//! - `zen` — full CLI with all subcommands
//! - `epp` — EPP management only (symlink to zen)
//! - `smu` — SMU monitoring only (symlink to zen)
//! - `mem` — memory timings only (symlink to zen)

mod cmd;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};

// =============================================================================
// CLI Definitions
// =============================================================================

/// zen - AMD Ryzen CPU management utility
#[derive(Parser, Debug)]
#[command(name = "zen", author, version, about, long_about = None)]
#[command(after_help = EXTENDED_HELP)]
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
    /// Show DDR4/DDR5 memory timings (like ZenTimings for Linux)
    Mem {
        /// Show raw register values alongside parsed timings
        #[arg(short, long)]
        raw: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum EppCommands {
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
pub enum SmuCommands {
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

/// EPP-only CLI (symlink: epp -> zen)
#[derive(Parser, Debug)]
#[command(name = "epp", about = "AMD Ryzen EPP management")]
struct EppCli {
    #[command(subcommand)]
    command: EppCommands,
}

/// SMU-only CLI (symlink: smu -> zen)
#[derive(Parser, Debug)]
#[command(name = "smu", about = "AMD Ryzen SMU monitoring")]
struct SmuCli {
    #[command(subcommand)]
    command: SmuCommands,
}

// =============================================================================
// Dispatch
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
        "epp" => {
            let cli = EppCli::parse();
            cmd::epp::handle_command(cli.command)
        }
        "smu" => {
            let cli = SmuCli::parse();
            cmd::smu::handle_command(cli.command)
        }
        "mem" => cmd::mem::handle(std::env::args().any(|a| a == "--raw" || a == "-r")),
        _ => run(),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.show_epp {
        return cmd::epp::show();
    }

    if let Some(level) = cli.perf_level {
        return cmd::epp::set_level(level);
    }

    match cli.command {
        Some(Commands::Epp { command }) => cmd::epp::handle_command(command),
        Some(Commands::Smu { command }) => cmd::smu::handle_command(command),
        Some(Commands::Mem { raw }) => cmd::mem::handle(raw),
        None => {
            Cli::command().print_help()?;
            Ok(())
        }
    }
}

// =============================================================================
// Help Text
// =============================================================================

const EXTENDED_HELP: &str = r#"
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

    # Memory Timings (ZenTimings for Linux)
    zen mem                         # Show DDR4/DDR5 timings
    zen mem --raw                   # Include raw register values

    # Busybox-style (symlinks created by `just install`)
    epp show                        # Same as `zen epp show`
    smu info                        # Same as `zen smu info`
    mem                             # Same as `zen mem`

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
"#;
