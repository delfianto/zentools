//! Memory timing command handler and display

use anyhow::Result;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::{presets::UTF8_FULL, Cell, CellAlignment, ContentArrangement, Table};
use zentools::smu::{driver, mem, smn};

pub fn handle(raw: bool) -> Result<()> {
    let smu_info = driver::read_info().ok();

    let is_zen5 = smu_info
        .as_ref()
        .map(|i| i.codename.is_zen5())
        .unwrap_or(false);

    // Determine DDR type from CPU generation (register probing is unreliable)
    let mem_type = if smu_info
        .as_ref()
        .map(|i| i.codename.is_ddr5())
        .unwrap_or(false)
    {
        mem::MemType::DDR5
    } else {
        mem::MemType::DDR4
    };

    let smn_reader = smn::SmnReader::new(is_zen5);
    if !smn_reader.is_available() {
        anyhow::bail!("SMN access not available. Requires root and PCI config access.");
    }

    let config = mem::read_mem_config(&smn_reader, mem_type)?;

    if config.channels.is_empty() {
        anyhow::bail!("No memory channels detected");
    }

    let ch = &config.channels[0];
    let t = &ch.timings;

    println!();

    // ── Info table ───────────────────────────────────────────────────────
    let mut info = Table::new();
    info.load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Memory Info").set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
        ]);

    info.add_row(vec![
        Cell::new("Type"),
        Cell::new(config.mem_type.to_string()),
        Cell::new("Channels"),
        Cell::new(config.channels.len().to_string()),
    ]);
    info.add_row(vec![
        Cell::new("Frequency"),
        Cell::new(format!("{:.0} MT/s", t.frequency_mhz)),
        Cell::new("Ratio"),
        Cell::new(t.ratio.to_string()),
    ]);
    info.add_row(vec![
        Cell::new("GDM"),
        Cell::new(if t.gdm { "Enabled" } else { "Disabled" }),
        Cell::new("Cmd Rate"),
        Cell::new(if t.cmd2t { "2T" } else { "1T" }),
    ]);
    info.add_row(vec![
        Cell::new("Power Down"),
        Cell::new(if t.power_down { "On" } else { "Off" }),
        Cell::new(""),
        Cell::new(""),
    ]);

    // Channel rows
    for ch in &config.channels {
        let dimms = match (ch.dimm0_present, ch.dimm1_present) {
            (true, true) => "Slot 0 + 1",
            (true, false) => "Slot 0",
            (false, true) => "Slot 1",
            _ => "empty",
        };
        info.add_row(vec![
            Cell::new(format!("Ch {}", ch.channel_id)),
            Cell::new(dimms),
            Cell::new("Speed"),
            Cell::new(format!("{:.0} MT/s", ch.timings.frequency_mhz)),
        ]);
    }

    println!("{}", info);

    // ── Timing table ─────────────────────────────────────────────────────
    let mut tim = Table::new();
    tim.load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Timing").set_alignment(CellAlignment::Left),
            Cell::new("Val").set_alignment(CellAlignment::Right),
            Cell::new("Timing").set_alignment(CellAlignment::Left),
            Cell::new("Val").set_alignment(CellAlignment::Right),
        ]);

    fn section(table: &mut Table, label: &str, rows: &[(&str, u32, &str, u32)]) {
        table.add_row(vec![
            Cell::new(label),
            Cell::new(""),
            Cell::new(""),
            Cell::new(""),
        ]);
        for &(l1, v1, l2, v2) in rows {
            table.add_row(vec![
                Cell::new(l1).set_alignment(CellAlignment::Left),
                Cell::new(v1).set_alignment(CellAlignment::Right),
                Cell::new(l2).set_alignment(CellAlignment::Left),
                Cell::new(v2).set_alignment(CellAlignment::Right),
            ]);
        }
    }

    section(&mut tim, "PRIMARY", &[
        ("tCL", t.tcl, "tRAS", t.tras),
        ("tRCDRD", t.trcdrd, "tRC", t.trc),
        ("tRCDWR", t.trcdwr, "tRP", t.trp),
    ]);
    section(&mut tim, "SECONDARY", &[
        ("tRRDS", t.trrds, "tRRDL", t.trrdl),
        ("tFAW", t.tfaw, "tWTRS", t.twtrs),
        ("tWTRL", t.twtrl, "tWR", t.twr),
        ("tCWL", t.tcwl, "tRTP", t.trtp),
    ]);
    section(&mut tim, "TERTIARY", &[
        ("tRDRDSCL", t.trdrdscl, "tWRWRSCL", t.twrwrscl),
        ("tRDRDSC", t.trdrdsc, "tWRWRSC", t.twrwrsc),
        ("tRDRDSD", t.trdrdsd, "tWRWRSD", t.twrwrsd),
        ("tRDRDDD", t.trdrddd, "tWRWRDD", t.twrwrdd),
        ("tRDWR", t.trdwr, "tWRRD", t.twrrd),
    ]);
    section(&mut tim, "REFRESH", &[
        ("tRFC", t.trfc, "tRFC2", t.trfc2),
        ("tRFC4", t.trfc4, "tREFI", t.trefi),
    ]);

    println!("{}", tim);

    // ── Raw registers ────────────────────────────────────────────────────
    if raw {
        let base = ch.channel_id << 20;
        let mut raw_table = Table::new();
        raw_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("Addr").set_alignment(CellAlignment::Center),
                Cell::new("Value").set_alignment(CellAlignment::Center),
                Cell::new("Register").set_alignment(CellAlignment::Left),
            ]);

        let regs: &[(&str, u32)] = &[
            ("CFG", 0x50200),
            ("TIM0 CL/RAS/RCD", 0x50204),
            ("TIM1 RC/RP", 0x50208),
            ("TIM2 RRDS/RRDL", 0x5020C),
            ("TIM3 FAW", 0x50210),
            ("TIM4 CWL/WTRS", 0x50214),
            ("TIM5 WR", 0x50218),
            ("TIM6 RDRD*", 0x50220),
            ("TIM7 WRWR*", 0x50224),
            ("TIM8 WRRD/RDWR", 0x50228),
            ("REFI", 0x50230),
            ("RFC", 0x50260),
        ];
        for (label, addr) in regs {
            if let Ok(val) = smn_reader.read_register(base | addr) {
                raw_table.add_row(vec![
                    Cell::new(format!("{:05X}", addr)).set_alignment(CellAlignment::Right),
                    Cell::new(format!("{:08X}", val)).set_alignment(CellAlignment::Right),
                    Cell::new(label),
                ]);
            }
        }

        println!("{}", raw_table);
    }

    println!();
    Ok(())
}
