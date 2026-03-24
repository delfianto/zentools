//! Memory timing command handler and display

use anyhow::Result;
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

    let title = format!(
        "zen mem - {} Memory Timings ({} channel{})",
        config.mem_type,
        config.channels.len(),
        if config.channels.len() > 1 { "s" } else { "" }
    );

    let w = 58;
    let sep = format!("+{}+", "=".repeat(w));
    let div = format!("+{}+", "-".repeat(w));

    // Title
    println!();
    println!("{}", sep);
    println!("| {:^w$} |", title, w = w);
    println!("{}", sep);
    println!("| {:>15}: {:<width$} |", "Frequency", format!("{:.0} MT/s", t.frequency_mhz), width = w - 18);
    println!("| {:>15}: {:<width$} |", "Ratio", t.ratio, width = w - 18);
    println!("| {:>15}: {:<width$} |", "GDM", if t.gdm { "Enabled" } else { "Disabled" }, width = w - 18);
    println!("| {:>15}: {:<width$} |", "Command Rate", if t.cmd2t { "2T" } else { "1T" }, width = w - 18);
    println!("| {:>15}: {:<width$} |", "Power Down", if t.power_down { "Enabled" } else { "Disabled" }, width = w - 18);
    println!("{}", div);

    // Timing sections as a unified table
    print_section(&div, w, "Primary", &[
        ("tCL", t.tcl, "tRAS", t.tras),
        ("tRCDRD", t.trcdrd, "tRC", t.trc),
        ("tRCDWR", t.trcdwr, "tRP", t.trp),
    ]);
    print_section(&div, w, "Secondary", &[
        ("tRRDS", t.trrds, "tRRDL", t.trrdl),
        ("tFAW", t.tfaw, "tWTRS", t.twtrs),
        ("tWTRL", t.twtrl, "tWR", t.twr),
        ("tCWL", t.tcwl, "tRTP", t.trtp),
    ]);
    print_section(&div, w, "Tertiary", &[
        ("tRDRDSCL", t.trdrdscl, "tWRWRSCL", t.twrwrscl),
        ("tRDRDSC", t.trdrdsc, "tWRWRSC", t.twrwrsc),
        ("tRDRDSD", t.trdrdsd, "tWRWRSD", t.twrwrsd),
        ("tRDRDDD", t.trdrddd, "tWRWRDD", t.twrwrdd),
        ("tRDWR", t.trdwr, "tWRRD", t.twrrd),
    ]);
    print_section(&div, w, "Refresh", &[
        ("tRFC", t.trfc, "tRFC2", t.trfc2),
        ("tRFC4", t.trfc4, "tREFI", t.trefi),
    ]);

    // Channels
    if config.channels.len() > 1 {
        println!("| {:^w$} |", "Channels", w = w);
        println!("| {:-<w$} |", "", w = w);
        for ch in &config.channels {
            let dimms = match (ch.dimm0_present, ch.dimm1_present) {
                (true, true) => "Slot 0 + Slot 1",
                (true, false) => "Slot 0",
                (false, true) => "Slot 1",
                _ => "empty",
            };
            println!(
                "|   Ch {:>2}  {:<16} {:>10} MT/s       |",
                ch.channel_id, dimms, format!("{:.0}", ch.timings.frequency_mhz)
            );
        }
        println!("{}", sep);
    }

    // Raw registers
    if raw {
        println!();
        let base = ch.channel_id << 20;
        println!(
            "+{}+",
            "=".repeat(52)
        );
        println!(
            "| {:^50} |",
            format!("Raw Registers (Ch {}, base 0x{:06X})", ch.channel_id, base)
        );
        println!(
            "+{}+{}+{}+",
            "-".repeat(9),
            "-".repeat(14),
            "-".repeat(27)
        );
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
                println!(
                    "| 0x{:05X} | 0x{:08X}   | {:<25} |",
                    addr, val, label
                );
            }
        }
        println!(
            "+{}+{}+{}+",
            "-".repeat(9),
            "-".repeat(14),
            "-".repeat(27)
        );
    }

    println!();
    Ok(())
}

fn print_section(div: &str, w: usize, title: &str, rows: &[(&str, u32, &str, u32)]) {
    println!("| {:^w$} |", title, w = w);
    println!("| {:-<w$} |", "", w = w);
    for (l1, v1, l2, v2) in rows {
        println!(
            "|   {:<12} {:>5}     {:<12} {:>5}       |",
            l1, v1, l2, v2
        );
    }
    println!("{}", div);
}
