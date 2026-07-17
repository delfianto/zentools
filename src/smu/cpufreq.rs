//! Per-core CPU frequency via the Linux cpufreq sysfs interface.
//!
//! On modern kernels, `scaling_cur_freq` is backed by the APERF/MPERF hardware
//! counter ratio (actual cycles executed vs. reference cycles), not by reading
//! back a requested P-state — this is the same source tools like `btop` use,
//! and it works under hardware-autonomous P-state control (amd-pstate active
//! mode, intel_pstate/HWP) where there is no "requested frequency" to read back
//! at all. No root and no `ryzen_smu` driver required.

use std::collections::BTreeMap;
use std::fs;

const CPU_ROOT: &str = "/sys/devices/system/cpu";

/// Maps each physical core to the logical CPU used to represent its frequency
/// (the lowest-numbered SMT sibling of that core).
pub struct CpuFreqReader {
    core_to_cpu: BTreeMap<u32, u32>,
}

impl CpuFreqReader {
    /// Probe cpufreq availability and build the physical-core -> logical-CPU mapping.
    /// Returns `None` if cpufreq isn't usable at all (non-Linux, no cpufreq driver
    /// loaded, or a kernel too old to expose per-core topology) — callers should
    /// treat that as "no frequency data available" rather than an error.
    pub fn new() -> Option<Self> {
        let core_to_cpu = scan_core_topology()?;

        if core_to_cpu.is_empty() {
            return None;
        }

        let reader = Self { core_to_cpu };
        let any_readable = reader
            .core_to_cpu
            .keys()
            .any(|&core_id| reader.read_core_mhz(core_id).is_some());

        any_readable.then_some(reader)
    }

    /// Physical core ids known to this reader, in ascending order.
    pub fn core_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.core_to_cpu.keys().copied()
    }

    /// Read the current frequency (MHz) for a physical core, if available.
    pub fn read_core_mhz(&self, core_id: u32) -> Option<f64> {
        let cpu = *self.core_to_cpu.get(&core_id)?;
        let path = format!("{CPU_ROOT}/cpu{cpu}/cpufreq/scaling_cur_freq");
        let khz: u64 = fs::read_to_string(path).ok()?.trim().parse().ok()?;
        Some(khz as f64 / 1000.0)
    }
}

fn scan_core_topology() -> Option<BTreeMap<u32, u32>> {
    let entries = fs::read_dir(CPU_ROOT).ok()?;
    let mut core_to_cpu = BTreeMap::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(idx_str) = name.strip_prefix("cpu") else {
            continue;
        };
        let Ok(cpu_idx) = idx_str.parse::<u32>() else {
            continue;
        };

        let core_id_path = entry.path().join("topology/core_id");
        let Ok(core_id_str) = fs::read_to_string(&core_id_path) else {
            continue;
        };
        let Ok(core_id) = core_id_str.trim().parse::<u32>() else {
            continue;
        };

        // Keep the lowest-numbered logical CPU per physical core (first SMT sibling seen).
        core_to_cpu.entry(core_id).or_insert(cpu_idx);
    }

    Some(core_to_cpu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_returns_something_or_none_without_panicking() {
        // Environment-dependent: just verify it doesn't panic and, if Some,
        // is internally consistent.
        if let Some(reader) = CpuFreqReader::new() {
            assert!(!reader.core_to_cpu.is_empty());
        }
    }

    #[test]
    fn test_read_core_mhz_unknown_core_is_none() {
        let reader = CpuFreqReader {
            core_to_cpu: BTreeMap::new(),
        };
        assert_eq!(reader.read_core_mhz(0), None);
    }

    #[test]
    fn test_core_ids_ascending_order() {
        let mut map = BTreeMap::new();
        map.insert(3, 3);
        map.insert(1, 1);
        map.insert(2, 2);
        let reader = CpuFreqReader { core_to_cpu: map };
        let ids: Vec<u32> = reader.core_ids().collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }
}
