use anyhow::{Result, anyhow};
use std::path::Path;
use zentools::{
    aod::AodCapture,
    monitor::{self, Session, Snapshot},
};

pub fn firmware(path: Option<&Path>) -> Result<()> {
    let capture = match path {
        Some(path) => AodCapture::from_path(path)?,
        None => monitor::cached_firmware().map_err(|e| anyhow!(e))?,
    };
    println!(
        "{}\nFirmware settings; electrical encodings are provisional.\n",
        capture.source
    );
    for row in capture.settings {
        println!(
            "{:<30} {:<32} raw {:>8}  [0x{:08X}]",
            row.name, row.display, row.raw, row.id
        );
    }
    Ok(())
}

pub fn save(path: &Path, aod: Option<&Path>) -> Result<()> {
    let mut session = Session::new();
    let _ = session.sample();
    std::thread::sleep(std::time::Duration::from_millis(250));
    let mut snapshot = session.sample();
    if let Some(path) = aod {
        snapshot.firmware = Some(AodCapture::from_path(path)?);
    }
    snapshot.write_to(path).map_err(|e| anyhow!(e))?;
    println!("Capture saved to {}", path.display());
    Ok(())
}

pub fn compare(before: &Path, after: &Path, all: bool) -> Result<()> {
    let before = Snapshot::from_path(before).map_err(|e| anyhow!(e))?;
    let after = Snapshot::from_path(after).map_err(|e| anyhow!(e))?;
    let rows = monitor::compare(&before, &after).map_err(|e| anyhow!(e))?;
    println!(
        "{} changed / {} compared fields",
        rows.iter().filter(|r| r.changed).count(),
        rows.len()
    );
    for row in rows.iter().filter(|r| all || r.changed) {
        println!(
            "{} · {}: {} → {}",
            row.category, row.name, row.before, row.after
        );
    }
    Ok(())
}
