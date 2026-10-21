# PM Table Reference

The PM (Power Management) table is a binary blob maintained by the SMU firmware. It contains real-time telemetry: temperatures, voltages, power, frequencies, per-core metrics, and PBO limits. The table layout changes between CPU generations and firmware versions.

## Known Versions

### Zen 2/3 (versions `0x2408xx`)

| Version | Size | CPUs |
|---------|------|------|
| `0x240903` | 0x514 | Matisse (3000 series) |
| `0x240802` | varies | Castle Peak, other Zen 2 |
| `0x240803` | varies | Zen 2/3 variants |

**Field map** (20 system fields + per-core): Source: `ryzen_smu/scripts/monitor_cpu.py`

| Offset | Field | Unit |
|--------|-------|------|
| `0x000` | PPT Limit | W |
| `0x004` | PPT Current | W |
| `0x008` | TDC Limit | A |
| `0x00C` | TDC Current | A |
| `0x010` | TjMax | C |
| `0x014` | Tctl | C |
| `0x020` | EDC Limit | A |
| `0x024` | EDC Current | A |
| `0x02C` | SVI2 Voltage | V |
| `0x060` | Core Power | W |
| `0x064` | SoC Power | W |
| `0x0A0` | Peak Voltage | V |
| `0x0B0` | SoC Voltage | V |
| `0x0B8` | SoC Current | A |
| `0x0C0` | FCLK | MHz |
| `0x0C4` | FCLK Average | MHz |
| `0x128` | UCLK | MHz |
| `0x138` | MCLK | MHz |
| `0x1F4` | cLDO_VDDP | V |
| `0x1F8` | cLDO_VDDG | V |

**Per-core offsets** (stride: 4 bytes per core):

| Base | Field |
|------|-------|
| `0x24C` | Core Power (W) |
| `0x26C` | Core Voltage (V) |
| `0x28C` | Core Temperature (C) |
| `0x30C` | Core Frequency (GHz, multiply by 1000 for MHz) |
| `0x32C` | Core Activity (%) |
| `0x36C` | Core Sleep (%) |
| `0x38C` | Core C0 (%) |
| `0x3AC` | Core CC1 (%) |
| `0x3CC` | Core CC6 (%) |

### Zen 4 (versions `0x4808xx` / `0x4809xx`)

| Version | Size | CPUs |
|---------|------|------|
| `0x480804` | varies | Raphael (7000 series) |
| `0x480805` | varies | Raphael |
| `0x480904` | varies | Raphael |

**Field map** (5 system fields + per-core): Source: `FrozenGalaxy/ryzen_smu_hwmon` (verified on 7950X3D)

| Offset | Field | Unit |
|--------|-------|------|
| `0x048` | Vcore | V |
| `0x0D8` | SoC Power | W |
| `0x0DC` | Package Power | W |
| `0x148` | VSOC | V |
| `0x454` | Tctl | C |

**Per-core offsets** (stride: 4 bytes, up to 16 cores):

| Base | Field |
|------|-------|
| `0x514` | Core Temperature (C) |
| `0x534` | CCD Temperature (C) |
| `0x554` | Core Power (W) |

Note: Offsets shifted dramatically from Zen 2/3 (Tctl moved from `0x014` to `0x454`).

### Zen 5 (versions `0x6201xx` / `0x6202xx` / `0x6211xx` / `0x6212xx`)

| Version | Size | CPUs |
|---------|------|------|
| `0x620105` | 1828 bytes | Granite Ridge |
| `0x620205` | 2452 bytes | Granite Ridge |
| `0x621101` | 1828 bytes | Granite Ridge (newer BIOS) |
| `0x621102` | 1828 bytes | Granite Ridge |
| `0x621201` | 2452 bytes | Granite Ridge |
| `0x621202` | 2452 bytes | Granite Ridge |

**Field map** (8 fields, partial): Source: `irusanov/ZenStates-Core PowerTable.cs`

| Offset | Field | Unit |
|--------|-------|------|
| `0x0E8` | VDD_MISC | V |
| `0x11C` | FCLK | MHz |
| `0x12C` | UCLK | MHz |
| `0x13C` | MCLK | MHz |
| `0x14C` | VDDCR_SOC | V |
| `0x40C` | CLDO_VDDG_IOD | V |
| `0x414` | CLDO_VDDG_CCD | V |
| `0x434` | CLDO_VDDP | V |

**Per-core field map (16-core family only: `0x620205`, `0x621201`, `0x621202`)**: Base offsets originate from a community draft in `hattedsquirrel/ryzen_monitor` issue #27 (comment by insunaa, 2025-08-06, against table version `0x620205`).

**Confirmed live** against a Ryzen 9 9950X (table version `0x620205`) by pinning single physical cores with `taskset`, diffing PM table reads taken before/after loading exactly one core, and cross-checking against `/proc/cpuinfo`, `scaling_cur_freq`, and `topology/core_id`:

| Offset (base) | Field | Stride | Unit | Status |
|---------------|-------|--------|------|--------|
| `0x4B4` | Core Power | 4 bytes x 16 cores | W | **Confirmed** — clean single-core delta (e.g. 0.7 W idle -> 18.1 W loaded), other cores unaffected |
| `0x4F4` | Core Voltage | 4 bytes x 16 cores | V | **Confirmed** — rises with load on the targeted core only (e.g. 0.84 V -> 1.32 V) |
| `0x534` | Core Temperature | 4 bytes x 16 cores | C | **Confirmed** — targeted core jumps ~40C, others roughly flat |
| `0x5F4` | Core C0 residency | 4 bytes x 16 cores | % | **Confirmed** — idle ~8%, loaded ~100% on the targeted core |
| `0x634` | Core CC1 residency | 4 bytes x 16 cores | % | **Confirmed** — inversely tracks C0 as expected |
| `0x674` | Core CC6 residency | 4 bytes x 16 cores | % | **Confirmed** — inversely tracks C0 as expected |

Sanity check: C0 + CC1 + CC6 sums to ~100% independently in both idle and loaded snapshots, which is a strong cross-validation these three offsets are correct together, not just individually plausible.

**Disproven**: the draft's frequency offset (float element 461) reads a flat ~0.645 constant regardless of real core frequency (verified against a core that went from 614 MHz idle to 5.4 GHz boost with zero change at that offset). It has been deliberately left unmapped rather than shipped as a plausible-but-wrong value — there is currently no known per-core frequency offset for this table version.

The 8-core family (`0x620105`, `0x621101`, `0x621102`) has a rougher, internally inconsistent community draft (duplicate/conflicting offset assignments) and was not ported — it needs fresh reverse-engineering rather than blind porting.

**Still unmapped**: PPT/TDC/EDC, per-core frequency, Tctl (not needed via PM table — the same value is available via the direct SMN register, see `detection.md`). The 2452-byte table has ~613 potential f32 fields — only 14 are identified across the system-level and per-core maps. AMD does not publish PM table documentation for any generation.

## Reverse Engineering

To help map Zen 5 offsets:

```bash
# Capture at idle
sudo zen smu pm-table -f --raw > idle.txt

# Run a load (e.g., stress-ng --cpu $(nproc) --timeout 30s)

# Capture under load
sudo zen smu pm-table -f --raw > load.txt

# Compare
diff idle.txt load.txt
```

Look for f32 values that correlate with known sensor readings (temperature from `sensors`, power from RAPL, frequency from `/proc/cpuinfo`).

## Live mprime comparison, 2026-10-21

Ryzen 9 9950X, SMU 98.84.0, PM table `0x620205`, 2452 bytes. Ran `mprime -t` for 75 seconds in a temporary working directory, with a fixed 16K in-place AVX-512 FFT, 16 workers and two threads per worker (32 threads total). EPP was unchanged. Collected 12 one-second PM-table samples per phase, with `/proc/stat` utilization, k10temp Tctl, and independent package RAPL energy deltas. Recovery sampling began about eight seconds after the workload stopped. The desktop remained running; “idle” means unloaded by mprime, not a fully quiescent machine.

| Metric | Idle | Load | Recovery |
|--------|------|------|----------|
| CPU busy (`/proc/stat`) | 7.76% | 99.99% | 8.32% |
| Package power (RAPL energy / elapsed time) | 67.76 W | 196.72 W | 70.08 W |
| Mean Tctl (k10temp) | 58.78 C | 83.15 C | 58.42 C |
| Maximum sampled Tctl | 61.63 C | 84.50 C | 61.38 C |
| Sum of 16 core powers (`0x4B4` base) | 27.31 W | 142.03 W | 25.27 W |
| Mean core C0 (`0x5F4` base) | 13.89% | 100.00% | 14.92% |
| Mean core CC1 (`0x634` base) | 48.07% | 0.00% | 45.24% |
| Mean core CC6 (`0x674` base) | 38.04% | 0.00% | 39.84% |

The C-state averages sum to ~100% in all three phases. This supports the existing per-core residency mappings. OS logical-CPU busy time and SMU physical-core C0 are different measurements and need not match at idle.

Every captured UMC register and decoded memory timing was identical across all three phases, on both channels. Memory remained DDR5-6000 (MCLK 3000 MHz), tCL/tRCDRD/tRP/tRAS 30/36/36/76, tRFC 884, tRFC2 480, tRFCsb 390, tREFI 11677, and tPHYRDL 36. See [memory.md](memory.md) for the corrected refresh decoding and additional subtimings.

Several previously unnamed f32 fields tracked package power. These remain **unassigned candidates**, not new production mappings:

| Offset | Idle mean | Load mean | Recovery mean |
|--------|-----------|-----------|---------------|
| `0x00C` | 69.30 | 196.84 | 68.20 |
| `0x068` | 69.38 | 197.33 | 68.10 |
| `0x454` | 70.12 | 196.67 | 67.92 |

The experiment cannot distinguish package power, PPT, filtered power or other related measurements among these candidates. Repeat with a different workload and controlled power limits before naming them. In particular, `0x454` must not inherit the Zen 4 “Tctl” label: on this table it reached ~197 while independent Tctl was ~83 C.

The earlier disproven per-core frequency candidate at float index 461 remains unmapped. This run did not establish a replacement frequency offset.

Raw evidence for this local run is in `/tmp/zentools-mprime-20261021/`: 36 binary PM tables, per-phase memory/info/EPP/sensor captures, independent samples in JSON, `pm-diff.csv` covering every finite f32 offset, `summary.json`, and the temporary mprime configuration. The workload stopped cleanly at its time limit; workers reported zero errors and warnings. This brief experiment establishes load-response behavior, not long-term stability.
