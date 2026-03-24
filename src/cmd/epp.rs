//! EPP command handlers and display

use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Cell, CellAlignment, ContentArrangement, Table};
use zentools::epp::{EppManager, EppProfile};

use crate::EppCommands;

pub fn handle_command(command: EppCommands) -> Result<()> {
    match command {
        EppCommands::Show => show(),
        EppCommands::Performance => set(EppProfile::Performance),
        EppCommands::BalancePerformance => set(EppProfile::BalancePerformance),
        EppCommands::BalancePower => set(EppProfile::BalancePower),
        EppCommands::Power => set(EppProfile::Power),
        EppCommands::Level { level } => set_level(level),
    }
}

pub fn show() -> Result<()> {
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

pub fn set(profile: EppProfile) -> Result<()> {
    let manager = EppManager::new()?;

    println!("Setting EPP to '{}' for all CPUs...", profile.as_str());
    manager.apply_profile(profile)?;
    println!("Successfully applied to {} CPUs", manager.cpu_count());

    Ok(())
}

pub fn set_level(level: u8) -> Result<()> {
    let profile = EppProfile::from_level(level)
        .ok_or_else(|| anyhow::anyhow!("Invalid level: {}. Must be 0-3.", level))?;

    set(profile)
}
