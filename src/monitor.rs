//! Qt-independent monitoring, versioned captures, and a small helper protocol.
use crate::{
    aod::{self, AodCapture},
    epp::{EppManager, EppProfile},
    smu::{
        self, CpuCodename, CpuMetrics, cpufreq, driver,
        mem::{self, MemConfig, MemType},
        msr, smn,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub model: String,
    pub codename: Option<CpuCodename>,
    pub board: String,
    pub bios: String,
    pub boot_id: String,
    pub topology: Option<driver::CpuTopology>,
}

impl Identity {
    pub fn detect() -> Self {
        let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let codename = driver::read_info()
            .ok()
            .map(|i| i.codename)
            .filter(|c| !matches!(c, CpuCodename::Unknown(_)))
            .or_else(|| codename_from_cpuinfo(&cpuinfo));
        let read = |path| {
            fs::read_to_string(path)
                .unwrap_or_default()
                .trim()
                .to_string()
        };
        Self {
            model: driver::read_cpu_model().unwrap_or_else(|| "Unknown CPU".into()),
            codename,
            board: read("/sys/class/dmi/id/board_name"),
            bios: read("/sys/class/dmi/id/bios_version"),
            boot_id: read("/proc/sys/kernel/random/boot_id"),
            topology: driver::read_cpu_topology(),
        }
    }
    pub fn memory_type(&self) -> MemType {
        match self.codename {
            Some(c) if c.is_ddr5() => MemType::DDR5,
            Some(
                CpuCodename::Matisse
                | CpuCodename::CastlePeak
                | CpuCodename::Vermeer
                | CpuCodename::Cezanne
                | CpuCodename::Milan
                | CpuCodename::Chagall
                | CpuCodename::Renoir
                | CpuCodename::Lucienne
                | CpuCodename::SummitRidge
                | CpuCodename::PinnacleRidge,
            ) => MemType::DDR4,
            _ => MemType::Unknown,
        }
    }
}

/// Deliberately bounded fallback for known AMD family/model pairs; never guess a future layout.
fn codename_from_cpuinfo(text: &str) -> Option<CpuCodename> {
    let fields: BTreeMap<_, _> = text
        .split("\n\n")
        .next()?
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    if fields.get("vendor_id") != Some(&"AuthenticAMD") {
        return None;
    }
    let family: u32 = fields.get("cpu family")?.parse().ok()?;
    let model: u32 = fields.get("model")?.parse().ok()?;
    match (family, model) {
        (0x1a, 0x44) => Some(CpuCodename::GraniteRidge),
        (0x19, 0x61) => Some(CpuCodename::Raphael),
        (0x19, 0x21) => Some(CpuCodename::Vermeer),
        (0x17, 0x71) => Some(CpuCodename::Matisse),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub offset: u32,
    pub label: String,
    pub value: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub timestamp_ms: u64,
    pub identity: Identity,
    pub metrics: CpuMetrics,
    pub memory: Option<MemConfig>,
    pub memory_error: Option<String>,
    pub memory_timestamp_ms: Option<u64>,
    pub firmware: Option<AodCapture>,
    pub firmware_error: Option<String>,
    pub epp_profiles: Vec<(u32, String)>,
    pub epp_error: Option<String>,
    pub cpu_usage_pct: Option<f64>,
    pub candidates: Vec<Candidate>,
    pub pm_table_version: Option<u32>,
    pub pm_table: Option<smu::PmTableData>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

impl Snapshot {
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        if metadata.len() > 8 * 1024 * 1024 {
            return Err("Snapshot exceeds 8 MiB".into());
        }
        let snapshot: Self = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if snapshot.schema_version != SNAPSHOT_VERSION {
            return Err("Unsupported snapshot version".into());
        }
        if snapshot.metrics.per_core.len() > 1024
            || snapshot
                .memory
                .as_ref()
                .is_some_and(|m| m.channels.len() > 32)
        {
            return Err("Snapshot topology is too large".into());
        }
        Ok(snapshot)
    }
    pub fn write_to(&self, path: &Path) -> Result<(), String> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        fs::write(path, bytes).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Difference {
    pub category: String,
    pub name: String,
    pub before: String,
    pub after: String,
    pub changed: bool,
}

pub fn compare(before: &Snapshot, after: &Snapshot) -> Result<Vec<Difference>, String> {
    if before.identity.model != after.identity.model
        || before.identity.board != after.identity.board
    {
        return Err("Captures belong to different CPU / board combinations".into());
    }
    let mut rows = Vec::new();
    let mut add = |category: &str, name: String, old: String, new: String| {
        rows.push(Difference {
            category: category.into(),
            name,
            changed: old != new,
            before: old,
            after: new,
        })
    };
    let reading =
        |v: Option<f64>, unit: &str| v.map_or_else(|| "N/A".into(), |v| format!("{v:.2} {unit}"));
    for (name, old, new, unit) in [
        (
            "Package power",
            before.metrics.package_power_w,
            after.metrics.package_power_w,
            "W",
        ),
        (
            "CPU temperature",
            before.metrics.tctl_temp_c,
            after.metrics.tctl_temp_c,
            "°C",
        ),
        ("CPU usage", before.cpu_usage_pct, after.cpu_usage_pct, "%"),
        (
            "Peak sampled clock",
            before.metrics.peak_core_freq_mhz,
            after.metrics.peak_core_freq_mhz,
            "MHz",
        ),
    ] {
        add(
            "Telemetry",
            name.into(),
            reading(old, unit),
            reading(new, unit),
        );
    }
    if let (Some(old), Some(new)) = (&before.memory, &after.memory)
        && old.mem_type != new.mem_type
    {
        return Err("Captures use different DDR layouts".into());
    }
    let registers = |snapshot: &Snapshot| -> BTreeMap<(u32, u32), Option<u32>> {
        snapshot
            .memory
            .iter()
            .flat_map(|m| &m.channels)
            .flat_map(|ch| {
                ch.registers
                    .iter()
                    .map(move |(&address, &value)| ((ch.channel_id, address), value))
            })
            .collect()
    };
    let old = registers(before);
    let new = registers(after);
    let mut keys: Vec<_> = old.keys().chain(new.keys()).copied().collect();
    keys.sort_unstable();
    keys.dedup();
    let raw = |value: Option<&Option<u32>>| {
        value
            .copied()
            .flatten()
            .map_or_else(|| "N/A".into(), |v| format!("0x{v:08X}"))
    };
    for (ch, address) in keys {
        add(
            "Memory registers",
            format!("Channel {ch} · 0x{:08X}", (ch << 20) | address),
            raw(old.get(&(ch, address))),
            raw(new.get(&(ch, address))),
        );
    }
    if let (Some(old), Some(new)) = (&before.pm_table, &after.pm_table) {
        if old.version == new.version && old.size() == new.size() {
            for offset in (0..old.size()).step_by(4) {
                let word = |table: &smu::PmTableData| {
                    table
                        .read_u32(offset)
                        .map_or_else(|| "N/A".into(), |v| format!("0x{v:08X}"))
                };
                add(
                    "PM table raw words",
                    format!("0x{offset:03X}"),
                    word(old),
                    word(new),
                );
            }
        } else {
            add(
                "PM table schema",
                "Raw words require matching version and size".into(),
                format!("0x{:06X} / {} bytes", old.version, old.size()),
                format!("0x{:06X} / {} bytes", new.version, new.size()),
            );
        }
    }
    if before.pm_table.is_some() != after.pm_table.is_some() {
        let describe = |table: &Option<smu::PmTableData>| {
            table.as_ref().map_or_else(
                || "Unavailable".into(),
                |t| format!("0x{:06X} / {} bytes", t.version, t.size()),
            )
        };
        add(
            "PM table availability",
            "Capture availability".into(),
            describe(&before.pm_table),
            describe(&after.pm_table),
        );
    }
    let old_fw: BTreeMap<_, _> = before
        .firmware
        .iter()
        .flat_map(|f| &f.settings)
        .map(|s| (s.id, s))
        .collect();
    let new_fw: BTreeMap<_, _> = after
        .firmware
        .iter()
        .flat_map(|f| &f.settings)
        .map(|s| (s.id, s))
        .collect();
    let mut ids: Vec<_> = old_fw.keys().chain(new_fw.keys()).copied().collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let old = old_fw.get(&id);
        let new = new_fw.get(&id);
        let value = |s: Option<&&crate::aod::AodSetting>| {
            s.map_or_else(
                || "N/A".into(),
                |s| format!("{} · raw {}", s.display, s.raw),
            )
        };
        let name = new.or(old).map(|s| s.name.clone()).unwrap_or_default();
        add("Firmware settings", name, value(old), value(new));
    }
    Ok(rows)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum HelperCommand {
    RefreshMemory {},
    ApplyEpp { profile: EppProfile },
    Shutdown {},
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum HelperEvent {
    Snapshot {
        snapshot: Box<Snapshot>,
        #[serde(default)]
        memory_refreshed: bool,
    },
    Result {
        success: bool,
        message: String,
    },
}

pub struct Session {
    identity: Identity,
    smn: Option<smn::SmnReader>,
    rapl: Option<msr::RaplReader>,
    cpufreq: Option<cpufreq::CpuFreqReader>,
    memory: Option<MemConfig>,
    memory_error: Option<String>,
    memory_timestamp_ms: Option<u64>,
    usage: Option<(u64, u64)>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
impl Session {
    pub fn new() -> Self {
        let identity = Identity::detect();
        let reader = smn::SmnReader::new(identity.codename.is_some_and(|c| c.is_zen5()));
        let smn = if identity.codename.is_some() && reader.is_available() {
            Some(reader)
        } else {
            None
        };
        let mut rapl = msr::RaplReader::new().ok();
        if let Some(r) = rapl.as_mut() {
            let _ = r.read_package_power();
            let _ = r.read_core_power();
        }
        let mut session = Self {
            identity,
            smn,
            rapl,
            cpufreq: cpufreq::CpuFreqReader::new(),
            memory: None,
            memory_error: None,
            memory_timestamp_ms: None,
            usage: None,
        };
        session.refresh_memory();
        session
    }
    pub fn refresh_memory(&mut self) {
        let result = match (&self.smn, self.identity.memory_type()) {
            (_, MemType::Unknown) => {
                Err("DDR layout is unknown for this CPU; memory decoding is unavailable".into())
            }
            (None, _) => Err("Enable hardware access to read memory timings".into()),
            (Some(reader), kind) => mem::read_mem_config(reader, kind)
                .map_err(|e| e.to_string())
                .and_then(|m| {
                    if m.channels.is_empty() {
                        Err("No populated memory channels were reported".into())
                    } else {
                        Ok(m)
                    }
                }),
        };
        match result {
            Ok(memory) => {
                self.memory = Some(memory);
                self.memory_error = None;
                self.memory_timestamp_ms = Some(now_ms());
            }
            Err(e) => {
                self.memory = None;
                self.memory_timestamp_ms = None;
                self.memory_error = Some(e);
            }
        }
    }
    pub fn sample(&mut self) -> Snapshot {
        let pm_table = driver::read_pm_table(true).ok();
        let metrics = smu::read_metrics_with_pm(
            self.smn.as_ref(),
            self.rapl.as_mut(),
            self.cpufreq.as_ref(),
            pm_table.as_ref(),
        );
        let epp = EppManager::new().and_then(|m| m.read_all());
        let (epp_profiles, epp_error) = match epp {
            Ok(rows) => (
                rows.into_iter()
                    .map(|r| (r.cpu_num, r.profile.as_str().into()))
                    .collect(),
                None,
            ),
            Err(e) => (Vec::new(), Some(e.to_string())),
        };
        let cpu_usage_pct = fs::read_to_string("/proc/stat")
            .ok()
            .and_then(|s| usage_totals(&s))
            .and_then(|new| {
                let previous = self.usage.replace(new)?;
                let total = new.0.checked_sub(previous.0)?;
                let idle = new.1.checked_sub(previous.1)?;
                (total > 0 && idle <= total).then(|| 100.0 * (total - idle) as f64 / total as f64)
            });
        let (candidates, pm_table_version) = probe_candidates(pm_table.as_ref());
        Snapshot {
            schema_version: SNAPSHOT_VERSION,
            timestamp_ms: now_ms(),
            identity: self.identity.clone(),
            metrics,
            memory: self.memory.clone(),
            memory_error: self.memory_error.clone(),
            memory_timestamp_ms: self.memory_timestamp_ms,
            firmware: None,
            firmware_error: None,
            epp_profiles,
            epp_error,
            cpu_usage_pct,
            candidates,
            pm_table_version,
            pm_table,
        }
    }
}

fn usage_totals(text: &str) -> Option<(u64, u64)> {
    let fields: Vec<u64> = text
        .lines()
        .find(|l| l.starts_with("cpu "))?
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    if fields.len() < 4 {
        return None;
    }
    Some((
        fields.iter().sum(),
        fields[3] + fields.get(4).copied().unwrap_or(0),
    ))
}

fn probe_candidates(table: Option<&smu::PmTableData>) -> (Vec<Candidate>, Option<u32>) {
    let Some(table) = table else {
        return (Vec::new(), None);
    };
    if table.version != 0x620205 || table.size() != 2452 {
        return (Vec::new(), Some(table.version));
    }
    let mut offsets = vec![
        (0x00c, "Unassigned power candidate".into()),
        (0x068, "Unassigned power candidate".into()),
        (0x454, "Unassigned power candidate".into()),
    ];
    offsets.extend((0..16).map(|i| (0x574 + i * 4, format!("Sampled-clock candidate · core {i}"))));
    (
        offsets
            .into_iter()
            .map(|(offset, label)| Candidate {
                offset,
                label,
                value: table.read_f32(offset as usize).filter(|v| v.is_finite()),
            })
            .collect(),
        Some(table.version),
    )
}

/// Read only an already loaded helper's cached results after the same exact AML audit check.
pub fn cached_firmware() -> Result<AodCapture, String> {
    let read = |p| {
        fs::read_to_string(p)
            .map(|s| s.trim().to_string())
            .map_err(|e| e.to_string())
    };
    if read("/sys/class/dmi/id/board_name")? != aod::AUDITED_BOARD
        || read("/sys/class/dmi/id/bios_version")? != aod::AUDITED_BIOS
    {
        return Err("Firmware profile has not been audited for this board / BIOS".into());
    }
    let audited = fs::read_dir("/sys/firmware/acpi/tables")
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("SSDT"))
        .filter_map(|e| fs::read(e.path()).ok())
        .any(|bytes| format!("{:x}", Sha256::digest(&bytes)) == aod::AUDITED_HASH);
    if !audited {
        return Err("AOD AML differs from the audited profile".into());
    }
    let raw = fs::read_to_string("/proc/zentools_aod_probe").map_err(|_| {
        "No cached AOD helper capture; import a saved capture or run the audited probe".to_string()
    })?;
    let mut capture = AodCapture::parse(&raw).map_err(|e| e.to_string())?;
    capture.source =
        "Cached audited WMI getter results · firmware settings, not live voltages".into();
    capture.metadata_verified = true;
    Ok(capture)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> Snapshot {
        Snapshot {
            schema_version: SNAPSHOT_VERSION,
            timestamp_ms: 1,
            identity: Identity {
                model: "Test CPU".into(),
                codename: Some(CpuCodename::GraniteRidge),
                board: "Test board".into(),
                bios: "Test".into(),
                boot_id: "Test boot".into(),
                topology: None,
            },
            metrics: CpuMetrics::default(),
            memory: None,
            memory_error: None,
            memory_timestamp_ms: None,
            firmware: None,
            firmware_error: None,
            epp_profiles: Vec::new(),
            epp_error: None,
            cpu_usage_pct: None,
            candidates: Vec::new(),
            pm_table_version: None,
            pm_table: None,
        }
    }
    #[test]
    fn compares_registers_by_channel_and_preserves_lost_readings() {
        let mut before = snapshot();
        before.memory = Some(MemConfig {
            mem_type: MemType::DDR5,
            channels: vec![mem::MemChannel {
                channel_id: 1,
                dimm0_present: true,
                dimm1_present: false,
                timings: mem::MemTimings::default(),
                registers: BTreeMap::from([(0x50204, Some(123)), (0x50208, Some(456))]),
            }],
        });
        let mut after = before.clone();
        after.memory.as_mut().unwrap().channels[0]
            .registers
            .insert(0x50204, None);
        let rows = compare(&before, &after).unwrap();
        let row = rows.iter().find(|r| r.name.contains("00150204")).unwrap();
        assert!(row.changed);
        assert_eq!(row.after, "N/A");
        assert_eq!(rows.iter().filter(|r| r.changed).count(), 1);
        after.identity.board = "Different board".into();
        assert!(compare(&before, &after).is_err());
    }
    #[test]
    fn snapshot_roundtrip_preserves_unavailable_values() {
        let before = snapshot();
        let bytes = serde_json::to_vec(&before).unwrap();
        let after: Snapshot = serde_json::from_slice(&bytes).unwrap();
        assert!(after.metrics.package_power_w.is_none());
        assert!(after.memory.is_none());
        assert_eq!(after.schema_version, SNAPSHOT_VERSION);
    }
    #[test]
    fn detection_never_defaults_unknown_or_intel_to_ddr4() {
        assert_eq!(
            codename_from_cpuinfo("vendor_id: GenuineIntel\ncpu family:26\nmodel:68"),
            None
        );
        assert_eq!(
            codename_from_cpuinfo("vendor_id: AuthenticAMD\ncpu family:26\nmodel:68"),
            Some(CpuCodename::GraniteRidge)
        );
        assert_eq!(
            codename_from_cpuinfo("vendor_id: AuthenticAMD\ncpu family:26\nmodel:255"),
            None
        );
        let identity = Identity {
            model: "Test".into(),
            board: "Board".into(),
            bios: String::new(),
            boot_id: String::new(),
            topology: None,
            codename: None,
        };
        assert_eq!(identity.memory_type(), MemType::Unknown);
    }
    #[test]
    fn protocol_rejects_arbitrary_operations_and_arguments() {
        assert!(
            serde_json::from_str::<HelperCommand>(r#"{"command":"write_register","address":0}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<HelperCommand>(
                r#"{"command":"apply_epp","profile":"Overclock"}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<HelperCommand>(
                r#"{"command":"refresh_memory","path":"/tmp/anything"}"#
            )
            .is_err()
        );
    }
    #[test]
    fn usage_excludes_guest_double_counting() {
        assert_eq!(
            usage_totals("cpu  10 0 5 80 5 0 0 0 100 0\n"),
            Some((100, 85))
        );
        assert_eq!(usage_totals("cpu 1\n"), None);
    }
}
