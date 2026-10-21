//! Memory timing command handler and display

use anyhow::Result;
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets::UTF8_FULL};
use zentools::smu::{mem, smn};

pub fn handle(raw: bool) -> Result<()> {
    let identity = zentools::monitor::Identity::detect();
    let is_zen5 = identity.codename.is_some_and(|c| c.is_zen5());
    let mem_type = identity.memory_type();
    if mem_type == mem::MemType::Unknown {
        anyhow::bail!("CPU memory layout is unknown; refusing to assume DDR4");
    }

    let smn_reader = smn::SmnReader::new(is_zen5);
    if !smn_reader.is_available() {
        anyhow::bail!("SMN access not available. Requires root and PCI config access.");
    }

    let config = mem::read_mem_config(&smn_reader, mem_type)?;

    if config.channels.is_empty() {
        anyhow::bail!("No memory channels detected");
    }

    for ch in &config.channels {
        display_channel(ch, config.mem_type, raw);
    }
    Ok(())
}

fn display_channel(ch: &mem::MemChannel, mem_type: mem::MemType, raw: bool) {
    let t = &ch.timings;

    println!("\nChannel {}", ch.channel_id);

    // ── Info table ───────────────────────────────────────────────────────
    let mut info = Table::new();
    info.load_style(UTF8_FULL.with_rounded_corners())
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Memory Info").set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
            Cell::new("").set_alignment(CellAlignment::Center),
        ]);

    info.add_row(vec![
        Cell::new("Type"),
        Cell::new(mem_type.to_string()),
        Cell::new("MCLK"),
        Cell::new(format!("{:.0} MHz", t.frequency_mhz / 2.0)),
    ]);
    info.add_row(vec![
        Cell::new("Frequency"),
        Cell::new(format!("{:.0} MT/s", t.frequency_mhz)),
        Cell::new("DIMMs"),
        Cell::new(match (ch.dimm0_present, ch.dimm1_present) {
            (true, true) => "Slot 0 + 1",
            (true, false) => "Slot 0",
            (false, true) => "Slot 1",
            _ => "empty",
        }),
    ]);
    info.add_row(vec![
        Cell::new("GDM"),
        Cell::new(if t.gdm { "Enabled" } else { "Disabled" }),
        Cell::new("Cmd Rate"),
        Cell::new(if t.cmd2t { "2T" } else { "1T" }),
    ]);
    info.add_row(vec![
        Cell::new("Power Down Enable"),
        Cell::new(match t.power_down {
            Some(true) => "On",
            Some(false) => "Off",
            None => "N/A",
        }),
        Cell::new(""),
        Cell::new(""),
    ]);
    if mem_type == mem::MemType::DDR5 {
        info.add_row(vec![
            Cell::new("Nitro RX/TX/Control"),
            Cell::new(t.nitro.map_or_else(
                || "N/A".into(),
                |[rx, tx, control]| format!("{rx}/{tx}/{control}"),
            )),
            Cell::new(""),
            Cell::new(""),
        ]);
    }

    println!("{}", info);

    // ── Timing table ─────────────────────────────────────────────────────
    let mut tim = Table::new();
    tim.load_style(UTF8_FULL.with_rounded_corners())
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

    section(
        &mut tim,
        "PRIMARY",
        &[
            ("tCL", t.tcl, "tRAS", t.tras),
            ("tRCDRD", t.trcdrd, "tRC", t.trc),
            ("tRCDWR", t.trcdwr, "tRP", t.trp),
        ],
    );
    section(
        &mut tim,
        "SECONDARY",
        &[
            ("tRRDS", t.trrds, "tRRDL", t.trrdl),
            ("tFAW", t.tfaw, "tWTRS", t.twtrs),
            ("tWTRL", t.twtrl, "tWR", t.twr),
            ("tCWL", t.tcwl, "tRTP", t.trtp),
        ],
    );
    section(
        &mut tim,
        "TERTIARY",
        &[
            ("tRDRDSCL", t.trdrdscl, "tWRWRSCL", t.twrwrscl),
            ("tRDRDSC", t.trdrdsc, "tWRWRSC", t.twrwrsc),
            ("tRDRDSD", t.trdrdsd, "tWRWRSD", t.twrwrsd),
            ("tRDRDDD", t.trdrddd, "tWRWRDD", t.twrwrdd),
            ("tRDWR", t.trdwr, "tWRRD", t.twrrd),
        ],
    );
    fn optional_section(
        table: &mut Table,
        label: &str,
        rows: &[(&str, Option<u32>, &str, Option<u32>)],
    ) {
        table.add_row(vec![label, "", "", ""]);
        for &(l1, v1, l2, v2) in rows {
            table.add_row(vec![
                Cell::new(l1),
                Cell::new(v1.map_or_else(|| "N/A".into(), |v| v.to_string())),
                Cell::new(l2),
                Cell::new(if l2.is_empty() {
                    String::new()
                } else {
                    v2.map_or_else(|| "N/A".into(), |v| v.to_string())
                }),
            ]);
        }
    }
    optional_section(
        &mut tim,
        "REFRESH",
        &[
            ("tRFC", t.trfc, "tRFC2", t.trfc2),
            (
                if mem_type == mem::MemType::DDR5 {
                    "tRFCsb"
                } else {
                    "tRFC4"
                },
                if mem_type == mem::MemType::DDR5 {
                    t.trfcsb
                } else {
                    t.trfc4
                },
                "tREFI",
                Some(t.trefi),
            ),
        ],
    );
    optional_section(
        &mut tim,
        "ADDITIONAL",
        &[
            ("tRCPage", t.trcpage, "tSTAG", t.tstag),
            ("tMOD", t.tmod, "tMODPDA", t.tmodpda),
            ("tMRD", t.tmrd, "tMRDPDA", t.tmrdpda),
            ("tXS", t.txs, "tDLLK", t.tdllk),
            ("tCKE", t.tcke, "tCPDED", t.tcpded),
            ("tXP", t.txp, "tRDDATAEN", t.trddataen),
            ("tPHYWRD", t.tphywrd, "tPHYWRL", t.tphywrl),
            ("tPHYRDL", t.tphyrdl, "", None),
        ],
    );

    println!("{}", tim);

    // ── Raw registers ────────────────────────────────────────────────────
    if raw {
        let base = ch.channel_id << 20;
        let mut raw_table = Table::new();
        raw_table
            .load_style(UTF8_FULL.with_rounded_corners())
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("Addr").set_alignment(CellAlignment::Center),
                Cell::new("Value").set_alignment(CellAlignment::Center),
                Cell::new("Register").set_alignment(CellAlignment::Left),
            ]);

        for (&addr, &value) in &ch.registers {
            raw_table.add_row(vec![
                Cell::new(format!("{:08X}", base | addr)),
                Cell::new(value.map_or_else(|| "unavailable".into(), |v| format!("{:08X}", v))),
                Cell::new(match addr {
                    0x5012C => "Power/refresh config",
                    0x50200 => "CFG",
                    0x50204 => "CL/RAS/RCD",
                    0x50208 => "RC/RP",
                    0x5020C => "RRDS/RRDL/RTP",
                    0x50210 => "FAW",
                    0x50214 => "CWL/WTRS/WTRL",
                    0x50218 => "WR",
                    0x5021C => "RCPage",
                    0x50220 => "RDRD*",
                    0x50224 => "WRWR*",
                    0x50228 => "WRRD/RDWR",
                    0x50230 => "REFI",
                    0x50234 => "MOD/MRD/PDA",
                    0x50238 => "XS/DLLK",
                    0x50250 => "STAG",
                    0x50254 => "CKE/CPDED/XP",
                    0x50258 => "PHY/RDDATAEN",
                    0x50284 => "Nitro RX/TX/control",
                    0x50260..=0x5026C => "RFC bank",
                    0x502C0..=0x502CC => "RFCsb bank",
                    _ => "UMC",
                }),
            ]);
        }

        println!("{}", raw_table);
    }

    println!();
}
