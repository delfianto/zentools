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

    // ── Main table: header + all timing sections in one ──────────────────
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![Cell::new(format!(
            "zen mem - {} Memory Timings ({} ch)",
            config.mem_type, config.channels.len()
        ))
        .set_alignment(CellAlignment::Center)]);

    table.add_row(vec![format!(
        " Frequency: {:.0} MT/s   Ratio: {}   GDM: {}   Cmd: {}   PwrDn: {}",
        t.frequency_mhz,
        t.ratio,
        if t.gdm { "On" } else { "Off" },
        if t.cmd2t { "2T" } else { "1T" },
        if t.power_down { "On" } else { "Off" },
    )]);

    // Section helper: adds a section header + timing rows to the table
    fn add_section(table: &mut Table, title: &str, rows: &[(&str, u32, &str, u32)]) {
        table.add_row(vec![Cell::new(format!("  --- {} ---", title))
            .set_alignment(CellAlignment::Center)]);
        for (l1, v1, l2, v2) in rows {
            table.add_row(vec![format!(
                "    {:<12} {:>5}       {:<12} {:>5}",
                l1, v1, l2, v2
            )]);
        }
    }

    add_section(&mut table, "Primary", &[
        ("tCL", t.tcl, "tRAS", t.tras),
        ("tRCDRD", t.trcdrd, "tRC", t.trc),
        ("tRCDWR", t.trcdwr, "tRP", t.trp),
    ]);
    add_section(&mut table, "Secondary", &[
        ("tRRDS", t.trrds, "tRRDL", t.trrdl),
        ("tFAW", t.tfaw, "tWTRS", t.twtrs),
        ("tWTRL", t.twtrl, "tWR", t.twr),
        ("tCWL", t.tcwl, "tRTP", t.trtp),
    ]);
    add_section(&mut table, "Tertiary", &[
        ("tRDRDSCL", t.trdrdscl, "tWRWRSCL", t.twrwrscl),
        ("tRDRDSC", t.trdrdsc, "tWRWRSC", t.twrwrsc),
        ("tRDRDSD", t.trdrdsd, "tWRWRSD", t.twrwrsd),
        ("tRDRDDD", t.trdrddd, "tWRWRDD", t.twrwrdd),
        ("tRDWR", t.trdwr, "tWRRD", t.twrrd),
    ]);
    add_section(&mut table, "Refresh", &[
        ("tRFC", t.trfc, "tRFC2", t.trfc2),
        ("tRFC4", t.trfc4, "tREFI", t.trefi),
    ]);

    // Channels
    if config.channels.len() > 1 {
        table.add_row(vec![Cell::new("  --- Channels ---")
            .set_alignment(CellAlignment::Center)]);
        for ch in &config.channels {
            let dimms = match (ch.dimm0_present, ch.dimm1_present) {
                (true, true) => "Slot 0 + 1",
                (true, false) => "Slot 0",
                (false, true) => "Slot 1",
                _ => "empty",
            };
            table.add_row(vec![format!(
                "    Ch {}   {:<14}  {:.0} MT/s",
                ch.channel_id, dimms, ch.timings.frequency_mhz
            )]);
        }
    }

    println!("{}", table);

    // ── Raw registers (separate table) ───────────────────────────────────
    if raw {
        let base = ch.channel_id << 20;
        let mut raw_table = Table::new();
        raw_table
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
                raw_table.add_row(vec![
                    Cell::new(format!("0x{:05X}", addr)).set_alignment(CellAlignment::Right),
                    Cell::new(format!("0x{:08X}", val)).set_alignment(CellAlignment::Right),
                    Cell::new(label),
                ]);
            }
        }

        println!("{}", raw_table);
    }

    println!();
    Ok(())
}
