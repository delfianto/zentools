# Dependencies

## Runtime Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| [anyhow](https://crates.io/crates/anyhow) | 1.x | Error handling in the binary (flexible error types) |
| [byteorder](https://crates.io/crates/byteorder) | 1.x | Little-endian binary parsing (PM table, sysfs files) |
| [clap](https://crates.io/crates/clap) | 4.x | CLI argument parsing with derive macros |
| [comfy-table](https://crates.io/crates/comfy-table) | 7.x | Unicode table formatting for terminal output |
| [ctrlc](https://crates.io/crates/ctrlc) | 3.x | Signal handling for clean monitor loop exit (Ctrl+C) |
| [thiserror](https://crates.io/crates/thiserror) | 2.x | Derive macro for library error types (`SmuError`, `EppError`) |

## System Dependencies

| Requirement | Used by | Purpose |
|------------|---------|---------|
| Linux kernel | All SMU/mem features | sysfs, PCI config, MSR device files |
| Root access (sudo) | EPP write, SMU read, mem read | Hardware register access requires privileges |
| `ryzen_smu` kernel module | `zen smu info/pm-table/debug` | Exposes PM table and SMU info via sysfs |
| `msr` kernel module | `zen smu monitor` (RAPL power) | Exposes MSR device files at `/dev/cpu/N/msr` |
| AMD Ryzen CPU | Everything | Zen 2 or newer recommended |
| `amd-pstate` driver | `zen epp` | Active mode required for EPP support |

## Build Dependencies

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.85+ | Edition 2024 support |
| Cargo | 1.85+ | Package manager |
| [just](https://github.com/casey/just) | any | Task runner (optional, for `just check/install`) |

## Release Build Profile

```toml
[profile.release]
strip = true        # Strip debug symbols
opt-level = 3       # Maximum optimization
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit for best optimization
```

Produces a ~950 KB statically-linked binary.
