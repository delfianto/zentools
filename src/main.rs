use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

// Bring in clap macros and types, including CommandFactory and ArgGroup
use clap::{ArgGroup, CommandFactory, FromArgMatches, Parser, ValueEnum};

/// Manages AMD Energy Performance Preference (EPP) settings.
struct AmdEppMgr {
    epp_paths: Vec<PathBuf>,
}

impl AmdEppMgr {
    /// Initializes the manager, find all CPU EPP paths from sysfs.
    /// Attempt to read the /sys/devices/system/cpu/cpu*/cpufreq/energy_performance_preference
    fn new() -> Result<Self, io::Error> {
        let mut epp_paths = Vec::new();
        let cpu_dir = PathBuf::from("/sys/devices/system/cpu/");

        for entry in fs::read_dir(&cpu_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
                    // Safely strip "cpu" prefix and attempt to parse the remaining digits
                    if let Some(cpu_num_str) = dir_name.strip_prefix("cpu") {
                        if cpu_num_str.parse::<u32>().is_ok() {
                            let epp_path = path.join("cpufreq/energy_performance_preference");
                            if epp_path.exists() {
                                epp_paths.push(epp_path);
                            }
                        }
                    }
                }
            }
        }

        epp_paths.sort();

        if epp_paths.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Error: No CPU energy preference files found.",
            ));
        }

        Ok(Self { epp_paths })
    }

    /// Applies the specified EPP profile key to all detected CPU EPP files.
    /// This requires the application itself to be run with root permissions.
    fn apply_profile(&self, profile_key: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("Applying EPP setting: {}", profile_key);

        // Append a newline to the profile key since the kernel expects it that way.
        // This is consistent with the behavior of writing to /sys files using shell command.
        let sys_profile = format!("{}\n", profile_key);

        for path in &self.epp_paths {
            let mut file = match fs::OpenOptions::new().write(true).open(path) {
                Ok(file) => file,
                Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => {
                    let error_message = format!(
                        "\nPermission error for writing to {}.\n\
                         Ensure you have root privileges to modify EPP settings.\n\
                         Error details: {}",
                        path.display(),
                        e
                    );
                    return Err(error_message.into());
                }
                Err(e) => {
                    // Other I/O errors
                    let error_message = format!(
                        "\nFailed to open {} for writing.\nError details: {}",
                        path.display(),
                        e
                    );
                    return Err(error_message.into());
                }
            };

            // Attempt to write and flush.
            // `?` will propagate any `io::Error` immediately.
            file.write_all(sys_profile.as_bytes())?;
            file.flush()?;
        }

        println!(
            "Successfully set value to {} for all detected CPU cores.",
            profile_key
        );
        Ok(())
    }

    /// Reads the current EPP value for all CPUs and prints them.
    fn read_profile(&self) -> Result<(), io::Error> {
        // CPU_Label : EPP_Value
        let mut cpu_data: Vec<(String, String)> = Vec::new();

        for path in &self.epp_paths {
            let cpu_num_str = path
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("cpu"));

            let cpu_num: u32 = match cpu_num_str {
                Some(num_str) => num_str.parse().map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid CPU number in path: {}", e),
                    )
                })?,
                None => {
                    eprintln!(
                        "Warning: Could not extract CPU number from path: {:?}",
                        path
                    );
                    continue;
                }
            };

            let mut epp_value = String::new();
            fs::File::open(path)?.read_to_string(&mut epp_value)?;
            let epp_value = epp_value.trim().to_string(); // Trim and convert to String

            cpu_data.push((format!("CPU{:02}", cpu_num), epp_value));
        }

        cpu_data.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by CPU label

        // Determine max length for CPU label column
        let max_label_len = cpu_data
            .iter()
            .map(|(label, _)| label.len())
            .max()
            .unwrap_or(0);

        const NUM_COLUMNS: usize = 3;
        const COLUMN_SPACING: usize = 2;

        // Print formatted output
        for chunk in cpu_data.chunks(NUM_COLUMNS) {
            let mut line_parts = Vec::new();
            for (label, value) in chunk {
                let entry_str = format!("{}: {}", label, value);
                // Pad each entry to max_overall_entry_len
                line_parts.push(format!("{:width$}", entry_str, width = max_label_len));
            }
            // Join the padded entries with spacing
            println!("{}", line_parts.join(" ".repeat(COLUMN_SPACING).as_str()));
        }

        Ok(())
    }
}

// --- CLI Definitions with clap ---
/// The actual EPP values that can be written to Linux sysfs.
#[derive(Debug, Clone, ValueEnum)]
enum EppValue {
    Performance,
    BalancePerformance,
    BalancePower,
    Power,
}

impl EppValue {
    /// Maps the enum variant to the actual string value written to the file.
    fn as_str(&self) -> &'static str {
        match self {
            EppValue::Performance => "performance",
            EppValue::BalancePerformance => "balance_performance",
            EppValue::BalancePower => "balance_power",
            EppValue::Power => "power",
        }
    }

    /// Provides a description for each EPP value.
    fn description(&self) -> &'static str {
        match self {
            EppValue::Performance => {
                "Prioritizes performance above power saving.\n\
                CPU reaches higher clock speeds aggressively."
            }
            EppValue::BalancePerformance => {
                "Aims for a balance but leans towards performance.\n\
                This is the default value in many systems."
            }
            EppValue::BalancePower => {
                "Aims for a balance but leans towards power saving.\n\
                More conservative clock speed increases."
            }
            EppValue::Power => {
                "Strongly prioritizes power saving.\n\
                Favors lower frequencies, may limit peak performance."
            }
        }
    }

    /// Converts a profile level (0-4) to an EppValue.
    /// 0: Performance, 1: BalancePerformance, 2: BalancePower, 3: Power
    fn from_level(level: u8) -> Option<Self> {
        match level {
            0 => Some(EppValue::Performance),
            1 => Some(EppValue::BalancePerformance),
            2 => Some(EppValue::BalancePower),
            3 => Some(EppValue::Power), // Both 3 and 4 map to Power for max powersave
            _ => None,
        }
    }
}

/// Helper function to generate the custom help section for EPP profiles.
fn get_profile_help_section() -> String {
    let mut help_text = String::new();
    help_text.push_str("EPP Profiles Explanations:\n");

    let indent = "  ";
    for variant in EppValue::value_variants() {
        let name = variant.as_str();
        let description = variant.description();

        // Print the profile name with a bullet
        help_text.push_str(&format!("- {}\n", name));

        // Split the description into lines and add fixed indentation to each
        for line in description.lines() {
            help_text.push_str(&format!("{}{}\n", indent, line));
        }
    }
    help_text
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Manage AMD Energy Performance Preference (EPP) settings.", long_about = None)]
#[command(help_template = "{usage}\n\n{about}\n\n{options}\n{after-help}")]
#[clap(group(ArgGroup::new("epp_action").multiple(false)))]
struct Cli {
    // EPP setting flags (mutually exclusive)
    #[arg(long, help = "Set EPP profile to 'performance'.", group = "epp_action")]
    performance: bool,

    #[arg(
        long,
        help = "Set EPP profile to 'balance-performance'.",
        group = "epp_action"
    )]
    balance_performance: bool,

    #[arg(
        long,
        help = "Set EPP profile to 'balance-power'.",
        group = "epp_action"
    )]
    balance_power: bool,

    #[arg(long, help = "Set EPP profile to 'power'.", group = "epp_action")]
    power: bool,

    // Short aliases -p0 to -p4
    #[arg(
        short = 'p',
        value_name = "LEVEL",
        help = "Set EPP profile by level.\n\
        0=performance, 1=balance-performance,\n\
        2=balance-power, 3=power",
        group = "epp_action"
    )]
    profile_level: Option<u8>,

    // Show current profile (mutually exclusive with setting profiles)
    #[arg(
        long,
        short = 's',
        help = "Show current EPP values for all CPU cores.",
        group = "epp_action"
    )]
    show: bool,
}

fn main() {
    if let Err(e) = run_app() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Cli::command();
    command = command.after_help(get_profile_help_section());

    let matches = command.get_matches();
    let cli = Cli::from_arg_matches(&matches)?;

    // Instantiate AmdEppMgr
    let epp_mgr = AmdEppMgr::new()?;

    // Determine the desired action based on provided flags
    let mut profile_to_set: Option<EppValue> = None;

    if cli.performance {
        profile_to_set = Some(EppValue::Performance);
    } else if cli.power {
        profile_to_set = Some(EppValue::Power);
    } else if cli.balance_performance {
        profile_to_set = Some(EppValue::BalancePerformance);
    } else if cli.balance_power {
        profile_to_set = Some(EppValue::BalancePower);
    } else if let Some(level) = cli.profile_level {
        profile_to_set = EppValue::from_level(level);
        if profile_to_set.is_none() {
            return Err(
                format!("Invalid profile level: {}. Must be between 0 and 3.", level).into(),
            );
        }
    }

    // If no arguments were provided, or none of the action flags were set, print help.
    if let Some(profile) = profile_to_set {
        epp_mgr.apply_profile(profile.as_str())?;
    } else if cli.show {
        epp_mgr.read_profile()?;
    } else {
        Cli::command().print_help()?;
    }

    Ok(())
}
