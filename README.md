# ZenTools 🔧

> A friendly Rust tool for poking at your AMD Ryzen CPU's knobs and dials

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

**TL;DR:** Control your Ryzen CPU's power settings (EPP) and peek at its internal telemetry (SMU) from the command line. Works great on most Ryzen CPUs, though Zen 5 is still teaching us its secrets.

## What Does This Thing Do?

ZenTools combines two useful features for Ryzen owners:

### 1. **EPP Management** (Energy Performance Preference)

Tell your CPU whether you want it to go _fast fast fast_ or sip power like a responsible citizen:

- `performance` - MAXIMUM OVERDRIVE 🚀
- `balance-performance` - Quick but not crazy
- `balance-power` - Chill but responsive
- `power` - Laptop battery mode activated 🔋

### 2. **SMU Monitoring** (System Management Unit)

Read the secret telemetry data your CPU's firmware tracks:

- CPU voltages, frequencies, temperatures
- Power consumption
- Per-core stats
- Various mysterious numbers AMD doesn't officially document

Think of it like giving your CPU a full medical checkup, except some of the readings are in a language we're still learning to translate.

## Quick Start

```bash
# Check your CPU's current power profile
sudo zentools epp show

# Set it to maximum performance
sudo zentools epp performance

# Or use the old-school shorthand (backward compatible!)
sudo zentools -p 0    # 0=performance, 1=balanced-perf, 2=balanced-power, 3=power

# See what your SMU knows about your CPU
sudo zentools smu info

# Dump the raw telemetry table (for science!)
sudo zentools smu pm-table --force
```

Note: You need `sudo` because we're messing with system files. Your CPU is precious! 💎

## Installation

### Prerequisites

1. **A Ryzen CPU** (Zen 2 through Zen 5)
2. **The ryzen_smu kernel driver** (for SMU features)

    ```bash
    # Check if you have it
    ls /sys/kernel/ryzen_smu_drv

    # If not, get it from: https://github.com/amkillam/ryzen_smu
    ```

3. **Rust** (if building from source)
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```

### Build & Install

```bash
# Clone this repo
git clone https://github.com/yourusername/zentools
cd zentools

# Build it
cargo build --release

# Install it system-wide (optional)
sudo make install

# Or just run from the build directory
sudo ./target/release/zentools --help
```

## CPU Support Status

| Generation | Codename          | EPP Control | SMU Info | PM Table Parsing                  |
| ---------- | ----------------- | ----------- | -------- | --------------------------------- |
| Zen 2      | Matisse, Renoir   | ✅ Works    | ✅ Works | ✅ Mostly mapped                  |
| Zen 3      | Vermeer, Cezanne  | ✅ Works    | ✅ Works | ✅ Mostly mapped                  |
| Zen 4      | Raphael, Phoenix  | ✅ Works    | ✅ Works | ⚠️ Partially mapped               |
| **Zen 5**  | **Granite Ridge** | ✅ Works    | ✅ Works | ⚠️ **Needs reverse engineering!** |

### What's the Deal with Zen 5?

Your shiny new Ryzen 9000 series CPU (like the 9950X, 9900X, etc.) reports PM table version `0x620205`, which is... brand new!

**What works:**

- ✅ EPP control - works perfectly!
- ✅ SMU version detection
- ✅ Reading the raw PM table with `--force`

**What doesn't (yet):**

- ❌ Parsing the PM table into human-readable metrics
- ❌ Labeled fields (voltage, frequency, temp per core)

**Why?** AMD doesn't document this stuff, and the community hasn't fully reverse-engineered the Zen 5 table structure yet. The data is all there, we just don't know which byte means "Core 0 voltage" vs "CPU's current mood." 🤷

## Background & Credits

### The Origin Story

This started as a simple bash script called `eppcli` to toggle EPP settings because clicking through GUI tools is _so_ 2010s. Then it grew ambitions and decided to become a proper Rust project with SMU support.

### Standing on the Shoulders of Giants

The SMU functionality is heavily inspired by (and partially ported from) the amazing work done by:

**[ryzen_smu](https://github.com/amkillam/ryzen_smu)**

- Original author: Leonidas Kotsalidis ([@leogx9r](https://github.com/leogx9r))
- Current maintainer: [@amkillam](https://github.com/amkillam)
- License: GPL-2.0

The C code from their `monitor_cpu` tool was studied, understood, then reimplemented in Rust with the help of **Claude Sonnet 4.5** (yes, an AI helped port this). The original `monitor_cpu` tool failed on Zen 5 with "PM Table version is not currently supported," which kicked off this whole project.

### What's Actually New Here

- ✅ Complete Rust rewrite (no C code copied)
- ✅ Multi-module workspace architecture
- ✅ Combined EPP + SMU in one tool
- ✅ Better error messages
- ✅ `comfy-table` for pretty output
- ✅ Zen 5 codename detection (even if table parsing is WIP)

All the Rust code is original, but the _knowledge_ about how SMU works comes from the reverse engineering done by the ryzen_smu community. Much respect! 🙏

## Usage Examples

### EPP Control

```bash
# Show current settings
sudo zentools epp show

# Output:
# ╭─────────────────────────────────────╮
# │    AMD EPP Status (32 CPUs)        │
# ├─────────────────────────────────────┤
# │ balance-power   | CPUs: [0..31]    │
# │                 | Balanced for...  │
# ╰─────────────────────────────────────╯

# Set to performance mode
sudo zentools epp performance

# Or use the numeric shorthand (backward compatible)
sudo zentools -p 0    # performance
sudo zentools -p 1    # balance-performance
sudo zentools -p 2    # balance-power
sudo zentools -p 3    # power
```

### SMU Monitoring

```bash
# Get basic info
sudo zentools smu info

# Output:
# ╭──────────────────────────────────────╮
# │   AMD Ryzen SMU Information          │
# ├──────────────────────────────────────┤
# │ SMU Version      | SMU v98.82.0      │
# │ Codename         | Granite Ridge (Zen 5) │
# │ Driver Version   | 0.1.7             │
# │ PM Table Version | 0x620205          │
# │ PM Table Size    | 2452              │
# ╰──────────────────────────────────────╯

# Verbose mode shows file paths
sudo zentools smu info --verbose

# Read the PM table (Zen 5 requires --force)
sudo zentools smu pm-table --force

# Continuous monitoring (updates every second)
sudo zentools smu pm-table --force --update 1

# Raw hex dump for reverse engineering
sudo zentools smu pm-table --force --raw > pm_table_dump.txt
```

## Troubleshooting

### "SMU driver not found"

You need the ryzen_smu kernel module:

```bash
# Install from: https://github.com/amkillam/ryzen_smu
cd /tmp
git clone https://github.com/amkillam/ryzen_smu
cd ryzen_smu
make
sudo make install
sudo modprobe ryzen_smu

# Make it load on boot
echo "ryzen_smu" | sudo tee /etc/modules-load.d/ryzen_smu.conf
```

### "Permission denied"

Use `sudo`. We're reading from/writing to system files that need root access.

### "PM Table version is not supported"

If you have Zen 5, use `--force`:

```bash
sudo zentools smu pm-table --force
```

This bypasses the version check and reads the raw data. It just won't parse it into labeled fields yet.

### EPP not working

Make sure your CPU's governor is set to use EPP:

```bash
# Check current governor
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor

# Should be 'powersave' or 'performance' (schedutil also works)
# If it's 'userspace', EPP won't work

# Set to powersave
echo powersave | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

## Contributing

### Help Reverse Engineer Zen 5!

If you have a Ryzen 9000 series CPU, you can help map the PM table structure:

```bash
# Capture at idle
sudo zentools smu pm-table --force --raw > zen5_idle.txt
sensors > sensors_idle.txt

# Run a stress test
stress-ng --cpu $(nproc) --timeout 30s

# Capture under load
sudo zentools smu pm-table --force --raw > zen5_load.txt
sensors > sensors_load.txt

# Compare the dumps
diff zen5_idle.txt zen5_load.txt

# Share your findings in a GitHub issue!
```

Look for values that change in predictable ways:

- Frequencies should go up under load
- Temperatures should increase
- Power consumption should rise
- Voltages might increase

If you can correlate PM table offsets with actual sensor readings, that's pure gold! 🏆

## FAQ

### Is this safe?

EPP control is totally safe—you're just setting power management hints the kernel already supports. SMU reading is read-only (we're just peeking, not poking). That said:

**⚠️ EXTREMELY IMPORTANT DISCLAIMER THAT YOU SHOULD DEFINITELY READ ⚠️**

This software is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF:

- MERCHANTABILITY (it might work, might not)
- FITNESS FOR A PARTICULAR PURPOSE (we have no idea what your purpose is)
- NONINFRINGEMENT (we didn't copy anyone's code, pinky swear)
- YOUR CPU NOT CATCHING FIRE (probably won't, but who knows)
- YOUR COMPUTER GAINING SENTIENCE (if it does, that's on you)
- THE CONTINUED STRUCTURAL INTEGRITY OF SPACETIME (we're 90% sure we didn't break physics)

In no event shall the authors be liable for any claim, damages, or other liability, whether in an action of contract, tort, or otherwise, arising from, out of, or in connection with the software or the use or other dealings in the software, including but not limited to:

- Spontaneous CPU combustion 🔥
- Unexpected time travel incidents ⏰
- Your electricity bill 💸
- Existential dread from seeing raw CPU telemetry 😱
- The heat death of the universe (not our fault) 🌌

**USE AT YOUR OWN RISK.** We tested this on our machines and they're fine, but your mileage may vary. If you break something, you get to keep both pieces! 🧩

### Will this work on [insert CPU here]?

Check the [CPU Support Status](#cpu-support-status) table above. Generally:

- Ryzen 2000-9000 series (desktop): Yes
- Ryzen 3000-7000 series (laptop): Probably
- Threadripper: Maybe? (untested)
- EPYC: Probably not (different SMU interface)

Try it and let us know!

## License

GPL-3.0-or-later. See [LICENSE](LICENSE) for legalese.
**Short version:** Use it, modify it, share it. Just keep it open source when you distribute it. 🎉

## Links

- **This project:** https://github.com/yourusername/zentools
- **ryzen_smu driver:** https://github.com/amkillam/ryzen_smu
- **AMD Ryzen subreddit:** https://reddit.com/r/Amd (not affiliated, just helpful)

## Acknowledgments

Huge thanks to:

- The original author of ryzen_smu team for reverse engineering AMD's SMU interface
- The Linux kernel developers for the cpufreq/EPP infrastructure
- The Rust community for making systems programming fun
- Claude Sonnet 4.5 for helping port C to Rust (AI pair programming is wild)
- Coffee ☕ (the real MVP)

---

Made with 🦀 and ❤️ in [Zed](https://zed.dev).
