# Architecture

## Project Structure

```
src/
  main.rs           CLI definitions + busybox dispatch (~210 lines)
  lib.rs            Library root (pub mod epp, smu)
  epp.rs            EPP management (sysfs read/write)
  cmd/
    mod.rs          Command handler module declarations
    epp.rs          EPP command handlers + display
    smu.rs          SMU command handlers + display (debug, pm-table, info) — `monitor` delegates to tui.rs
    mem.rs          Memory timing command handler + display
    tui.rs          Ratatui dashboard for `smu monitor` (color-coded table, sparkline history, keyboard interaction)
  smu/
    mod.rs          Tiered metrics orchestrator (PM table + direct registers)
    types.rs        Shared types: CpuCodename, SmuError, CpuMetrics, etc.
    driver.rs       ryzen_smu kernel module sysfs interface
    pmtable.rs      PM table field maps + version-specific parsing
    mem.rs          UMC memory timing register reader
    smn.rs          SMN register reader (temperature, voltage via PCI config)
    msr.rs          MSR reader (RAPL power via /dev/cpu/N/msr)
    cpufreq.rs      Per-core frequency via Linux cpufreq sysfs (no root, no kernel module)
```

## Design Principles

**Library vs Binary separation**: `src/lib.rs` exposes `epp` and `smu` as library modules containing all data access logic. `src/main.rs` and `src/cmd/` are binary-only — they handle CLI parsing, command dispatch, and display formatting. The library has no dependency on `clap` or `comfy-table`.

**Busybox-style dispatch**: The single `zen` binary checks `argv[0]` to determine its personality. Symlinks `epp`, `smu`, and `mem` route directly to subsystem-specific CLI parsers. `just install` creates these symlinks automatically.

**Tiered data sources**: SMU monitoring uses a fallback chain:
1. **PM table** via ryzen_smu driver (most complete, requires kernel module)
2. **Direct registers** via SMN (temperature, voltage) and MSR (RAPL power)
3. Both sources merged when available (Hybrid mode)

This means basic monitoring works without the kernel module — just root access.

## Module Responsibilities

| Module | Reads from | Data |
|--------|-----------|------|
| `epp.rs` | `/sys/devices/system/cpu/*/cpufreq/energy_performance_preference` | EPP profile |
| `smu/driver.rs` | `/sys/kernel/ryzen_smu_drv/*` | SMU info, PM table binary |
| `smu/smn.rs` | `/sys/bus/pci/devices/0000:00:00.0/config` (PCI regs 0x60/0x64) | Temperature, SVI voltage |
| `smu/msr.rs` | `/dev/cpu/N/msr` | RAPL energy counters |
| `smu/cpufreq.rs` | `/sys/devices/system/cpu/cpuN/cpufreq/scaling_cur_freq` | Per-core frequency (APERF/MPERF-backed) |
| `smu/mem.rs` | SMN registers 0x50000-0x50260 | DDR4/DDR5 memory timings |
| `smu/pmtable.rs` | (parses binary data from driver.rs) | Version-specific field extraction |

## Error Handling

All library functions return `Result<T, SmuError>` or `Result<T, EppError>`. The binary layer converts these to `anyhow::Result` for uniform error display. Hardware-not-available errors are handled gracefully — the monitor shows whatever data is accessible.

## Platform Support

Linux-only for hardware access (`/dev/cpu/*/msr`, PCI config space, sysfs). The code compiles on all platforms but hardware reads return errors on non-Linux. All `#[cfg(target_os = "linux")]` blocks have matching `#[cfg(not(...))]` stubs.

## Qt frontend and shared capture API

The optional `gui/` workspace member uses CXX-Qt 0.10 and Qt Quick/QML. Root
`default-members` excludes it, and CLI dependencies are behind the default
`cli` feature; the GUI uses the library without terminal dependencies.

`monitor::Session` owns the readers, RAPL baseline, memory refresh cache, and
OS utilization deltas. `Snapshot` is a versioned serializable capture with CPU
identity, sample times, optional readings, raw UMC registers and PM-table data.
Telemetry parsing uses the same PM table retained for comparison. `aod` imports
and validates the audited 81-object getter protocol with explicit provenance.

The frontend worker coalesces owned presentation snapshots onto the QObject's
GUI thread. QML views bind to immutable arrays; no hardware I/O runs on the
GUI thread. The bounded history retains 120 samples. Baselines and imported
firmware data are owned independently of subsequent live samples.

Privileged access is a separate `zentools-helper stream` process, authorized by
`pkexec` only after an explicit user action. Its pipe protocol permits memory
refresh, the four known EPP preferences, and shutdown. It accepts no paths,
arbitrary addresses, firmware setters, or shell commands. EOF ends the session;
the GUI closes stdin, stops/reaps the child and joins its reader on shutdown.
Worker joins happen outside QObject methods to avoid their Rust-state lock.

See [gui.md](gui.md) for building, usage, and current limits.
