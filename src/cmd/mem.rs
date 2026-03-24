//! Memory timing command handler and display

use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Cell, CellAlignment, ContentArrangement, Table};
use zentools::smu::{driver, mem, smn};

pub fn handle(raw: bool) -> Result<()> {
    let is_zen5 = driver::read_info()
        .map(|i| i.codename.is_zen5())
        .unwrap_or(false);

    let smn_reader = smn::SmnReader::new(is_zen5);
    if !smn_reader.is_available() {
        anyhow::bail!("SMN access not available. Requires root and PCI config access.");
    }

    let config = mem::read_mem_config(&smn_reader)?;

    if config.channels.is_empty() {
        anyhow::bail!("No memory channels detected");
    }

    let ch = &config.channels[0];
    let t = &ch.timings;

    println!();
    display_header(&config, t);
    display_primary(t);
    display_secondary(t);
    display_tertiary(t);
    display_refresh(t);
    display_channels(&config);

    if raw {
        display_raw_registers(&smn_reader, ch.channel_id);
    }

    println!();
    Ok(())
}

fn display_header(config: &mem::MemConfig, t: &mem::MemTimings) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![Cell::new(format!(
            "zen mem - {} Memory Timings ({} channel{})",
            config.mem_type,
            config.channels.len(),
            if config.channels.len() > 1 { "s" } else { "" }
        ))
        .set_alignment(CellAlignment::Center)]);

    table.add_row(vec!["Frequency", &format!("{:.0} MT/s", t.frequency_mhz)]);
    table.add_row(vec!["Ratio", &format!("{}", t.ratio)]);
    table.add_row(vec!["GDM", if t.gdm { "Enabled" } else { "Disabled" }]);
    table.add_row(vec!["Command Rate", if t.cmd2t { "2T" } else { "1T" }]);
    table.add_row(vec!["Power Down", if t.power_down { "Enabled" } else { "Disabled" }]);

    println!("{}", table);
}

fn timing_table(title: &str, rows: &[(&str, u32, &str, u32)]) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new(title).set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
        ]);

    for (l1, v1, l2, v2) in rows {
        table.add_row(vec![
            Cell::new(l1),
            Cell::new(v1).set_alignment(CellAlignment::Right),
            Cell::new(l2),
            Cell::new(v2).set_alignment(CellAlignment::Right),
        ]);
    }

    table
}

fn display_primary(t: &mem::MemTimings) {
    println!("{}", timing_table("Primary", &[
        ("tCL", t.tcl, "tRAS", t.tras),
        ("tRCDRD", t.trcdrd, "tRC", t.trc),
        ("tRCDWR", t.trcdwr, "tRP", t.trp),
    ]));
}

fn display_secondary(t: &mem::MemTimings) {
    println!("{}", timing_table("Secondary", &[
        ("tRRDS", t.trrds, "tRRDL", t.trrdl),
        ("tFAW", t.tfaw, "tWTRS", t.twtrs),
        ("tWTRL", t.twtrl, "tWR", t.twr),
        ("tCWL", t.tcwl, "tRTP", t.trtp),
    ]));
}

fn display_tertiary(t: &mem::MemTimings) {
    println!("{}", timing_table("Tertiary", &[
        ("tRDRDSCL", t.trdrdscl, "tWRWRSCL", t.twrwrscl),
        ("tRDRDSC", t.trdrdsc, "tWRWRSC", t.twrwrsc),
        ("tRDRDSD", t.trdrdsd, "tWRWRSD", t.twrwrsd),
        ("tRDRDDD", t.trdrddd, "tWRWRDD", t.twrwrdd),
        ("tRDWR", t.trdwr, "tWRRD", t.twrrd),
    ]));
}

fn display_refresh(t: &mem::MemTimings) {
    println!("{}", timing_table("Refresh", &[
        ("tRFC", t.trfc, "tRFC2", t.trfc2),
        ("tRFC4", t.trfc4, "tREFI", t.trefi),
    ]));
}

fn display_channels(config: &mem::MemConfig) {
    if config.channels.len() <= 1 {
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Channel").set_alignment(CellAlignment::Center),
            Cell::new("DIMMs").set_alignment(CellAlignment::Center),
            Cell::new("Speed").set_alignment(CellAlignment::Center),
        ]);

    for ch in &config.channels {
        let dimms = match (ch.dimm0_present, ch.dimm1_present) {
            (true, true) => "Slot 0 + Slot 1",
            (true, false) => "Slot 0",
            (false, true) => "Slot 1",
            _ => "empty",
        };
        table.add_row(vec![
            Cell::new(format!("Ch {}", ch.channel_id)).set_alignment(CellAlignment::Center),
            Cell::new(dimms),
            Cell::new(format!("{:.0} MT/s", ch.timings.frequency_mhz))
                .set_alignment(CellAlignment::Right),
        ]);
    }

    println!("{}", table);
}

fn display_raw_registers(smn_reader: &smn::SmnReader, channel_id: u32) {
    let base = channel_id << 20;
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Address").set_alignment(CellAlignment::Center),
            Cell::new("Value").set_alignment(CellAlignment::Center),
            Cell::new("Register").set_alignment(CellAlignment::Left),
        ]);

    let regs: &[(&str, u32)] = &[
        ("CFG (ratio/GDM/2T)", 0x50200),
        ("TIM0 (CL/RAS/RCD)", 0x50204),
        ("TIM1 (RC/RP)", 0x50208),
        ("TIM2 (RRDS/RRDL/RTP)", 0x5020C),
        ("TIM3 (FAW)", 0x50210),
        ("TIM4 (CWL/WTRS/WTRL)", 0x50214),
        ("TIM5 (WR)", 0x50218),
        ("TIM6 (RDRD*)", 0x50220),
        ("TIM7 (WRWR*)", 0x50224),
        ("TIM8 (WRRD/RDWR)", 0x50228),
        ("REFI", 0x50230),
        ("RFC", 0x50260),
    ];
    for (label, addr) in regs {
        if let Ok(val) = smn_reader.read_register(base | addr) {
            table.add_row(vec![
                Cell::new(format!("0x{:05X}", addr)).set_alignment(CellAlignment::Right),
                Cell::new(format!("0x{:08X}", val)).set_alignment(CellAlignment::Right),
                Cell::new(label),
            ]);
        }
    }

    println!("{}", table);
}
