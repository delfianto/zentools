# zentools

AMD Ryzen CPU management and monitoring tool for Linux.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

EPP power profile control, SMU telemetry monitoring, and DDR4/DDR5 memory timing readout in a single binary. Supports Zen 2 through Zen 5.

An optional **Qt 6 / QML desktop app** adds live graphs, per-core telemetry,
per-channel memory tables, firmware capture import, EPP profiles, and idle/load
snapshot comparisons. The interface runs as your normal user; hardware access
is authorized separately.

```bash
just run-gui             # build and launch (Qt 6 development packages + LLD)
just run-gui --demo      # preview with simulated telemetry; hardware writes disabled
just install-gui         # optional release build + user desktop launcher
```

See [the GUI guide](docs/gui.md) for dependencies and usage.

## Features

| Command | What it does | Requires |
|---------|-------------|----------|
| `zentools epp show` | Show current EPP power profile for all CPUs | sudo, amd-pstate driver |
| `zentools epp performance` | Set max performance profile | sudo |
| `zentools -p0` .. `zentools -p3` | Quick EPP level (0=perf, 1=bal-perf, 2=bal-power, 3=power) | sudo |
| `zentools smu info` | SMU version, codename, PM table info | sudo, ryzen_smu module |
| `zentools smu monitor` | Live CPU monitoring (temp, power, voltage, per-core) | sudo |
| `zentools smu debug` | Full diagnostic dump (driver, registers, PM table scan) | sudo |
| `zentools smu pm-table -f` | Read PM table (use `--raw` for hex dump) | sudo, ryzen_smu module |
| `zentools mem` | Show primary timings and subtimings for every detected channel | sudo, readable ryzen_smu metadata for DDR5 detection |
| `zentools mem --raw` | Include captured UMC registers and all refresh banks | sudo |

## Install

```bash
git clone https://github.com/delfianto/zentools
cd zentools

# Build + install to ~/.local/bin with symlinks
just install

# Or into /usr/local/bin (via sudo)
just install --system

# Or manually
cargo build --release
sudo cp target/release/zentools /usr/local/bin/
```

The `just install` command creates busybox-style symlinks so you can also run:
- `zen-epp show` instead of `zentools epp show`
- `zen-smu info` instead of `zentools smu info`
- `zen-mem` instead of `zentools mem`

### Prerequisites

- AMD Ryzen CPU (Zen 2 or newer)
- Rust 1.98+ (edition 2024)
- Linux with root access
- `amd-pstate` driver in active mode (for EPP)
- [`ryzen_smu`](https://github.com/amkillam/ryzen_smu) kernel module (for SMU features, optional for basic monitoring)

```bash
# Load ryzen_smu
sudo modprobe ryzen_smu

# Load msr module (for RAPL power monitoring without ryzen_smu)
sudo modprobe msr
```

## CPU Support

| Generation | Desktop | PM Table | Memory Timings |
|-----------|---------|----------|----------------|
| Zen 2 | Matisse (3000) | Full (20 fields + per-core) | DDR4 |
| Zen 3 | Vermeer (5000) | Full (shared with Zen 2) | DDR4 |
| Zen 4 | Raphael (7000) | Partial (5 fields + per-core) | DDR5 |
| Zen 5 | Granite Ridge (9000) | Partial (8 fields, clocks + voltages) | DDR5 |

EPP control and direct register monitoring (temperature, RAPL power) work on all generations. PM table field mapping varies — see [docs/pmtable.md](docs/pmtable.md) for the full offset reference.

## Quick Examples

```bash
# Set performance mode
sudo zen -p0

# Live monitoring
sudo zen smu monitor

# Memory timings
sudo zen mem

# Per-channel subtimings and refresh-bank register values
sudo zen mem --raw

# Full diagnostic
sudo zen smu debug

# Check what data sources are available
sudo zen smu check
```

## Documentation

| Document | Contents |
|----------|----------|
| [docs/architecture.md](docs/architecture.md) | Project structure, module design, separation of concerns |
| [docs/detection.md](docs/detection.md) | CPU identification, tiered data sources, register addresses |
| [docs/pmtable.md](docs/pmtable.md) | PM table versions, field offset maps, reverse engineering guide |
| [docs/memory.md](docs/memory.md) | UMC register map, DDR4/DDR5 differences, timing parameters |
| [docs/probing.md](docs/probing.md) | AOD firmware getters, electrical settings, frequency and power experiments |
| [docs/gui.md](docs/gui.md) | Desktop app setup, EPP profiles, capture comparison and Qt architecture |
| [docs/dependencies.md](docs/dependencies.md) | Crate dependencies, system requirements, build profile |

## Development

```bash
just check    # Run lint + compile check + all tests
just test     # Run tests only
just lint     # Clippy with warnings as errors
just fmt      # Format code
just build    # Release build
just install  # Build + install + create symlinks
```

## Helping with Zen 5

AMD does not publish PM table documentation. Only 8 of ~613 potential fields are mapped for Zen 5. If you have a Ryzen 9000 series CPU:

```bash
# Capture idle
sudo zen smu pm-table -f --raw > idle.txt

# Run a stress test, then capture
sudo zen smu pm-table -f --raw > load.txt

# Compare
diff idle.txt load.txt
```

Correlate changing offsets with known values from `sensors`, `zen smu debug`, or `zen mem`. See [docs/pmtable.md](docs/pmtable.md) for details.

## Credits

This project builds on the work of several open-source projects:

**[ryzen_smu](https://github.com/amkillam/ryzen_smu)** by [@leogx9r](https://github.com/leogx9r) and [@amkillam](https://github.com/amkillam) (GPL-2.0)
The kernel module that exposes the SMU interface. PM table access, SMU mailbox addresses, and codename mappings all originate from this project. The `monitor_cpu.py` script provided the Zen 2/3 PM table offset map.

**[ZenStates-Core](https://github.com/irusanov/ZenStates-Core)** by [@irusanov](https://github.com/irusanov) (MIT)
The 8 confirmed Zen 5 PM table offsets (FCLK, UCLK, MCLK, VDDCR_SOC, VDD_MISC, CLDO_VDDP, CLDO_VDDG_IOD, CLDO_VDDG_CCD) come from the `PowerTable.cs` reverse engineering work.

**[ZenTimings](https://github.com/irusanov/ZenTimings)** by [@irusanov](https://github.com/irusanov) (MIT)
The UMC register map for DDR4/DDR5 memory timing readout. Register addresses, bit field layouts, and DDR4 vs DDR5 differences are all derived from this Windows tool — `zen mem` is essentially the Linux port.

**[ryzen_smu_hwmon](https://github.com/FrozenGalaxy/ryzen_smu_hwmon)** by [@FrozenGalaxy](https://github.com/FrozenGalaxy)
Zen 4 (Raphael/7950X3D) PM table offsets for temperature, power, and voltage.

**[zenpower5](https://github.com/mattkeenan/zenpower5)** by [@mattkeenan](https://github.com/mattkeenan)
Zen 5 temperature register addresses and RAPL power monitoring approach.

**[kylon/ryzen_smu](https://github.com/kylon/ryzen_smu)** — Additional Zen 5 PM table version/size mappings.

**[Death4two/RyzenSMUDebug-LINUX](https://github.com/Death4two/RyzenSMUDebug-LINUX)** — Granite Ridge SMU command references.

## Disclaimer

This software talks directly to your CPU's hardware registers. It reads PCI config space, MSR registers, and pokes around in memory controller internals that AMD pretends don't exist.

**EPP control** is safe — it writes to the same sysfs interface your desktop environment uses. **SMU monitoring** is read-only — we look but don't touch. **Memory timing readout** is also read-only.

That said:

THIS SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY ARISING FROM THE USE OF THIS SOFTWARE, INCLUDING BUT NOT LIMITED TO:

- Your CPU spontaneously deciding it's had enough
- Incorrect readings leading to incorrect conclusions leading to incorrect BIOS settings leading to an expensive paperweight
- The existential dread of discovering your "DDR5-6000" is actually running at 4800 because you forgot to enable EXPO
- Kernel panics from loading sketchy out-of-tree kernel modules (looking at you, ryzen_smu)
- Your cat walking across the keyboard while `zen smu monitor` is running with sudo

If your silicon catches fire, you get to keep both pieces. We tested this on exactly one (1) AMD Ryzen 9 9950X and it's still alive. Your results may vary. Probably fine. No promises.

## License

MIT. See [LICENSE](LICENSE).

## Origin

Started as a bash script called `eppcli` for toggling EPP settings. Grew into a Rust project when the author wanted SMU monitoring on a Zen 5 system and discovered that AMD's idea of documentation is "buy the chip and figure it out yourself."

Built with [Claude Code](https://claude.ai/claude-code).
