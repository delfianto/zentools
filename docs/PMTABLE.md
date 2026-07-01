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

**Still unmapped**: PPT/TDC/EDC, per-core frequency, Tctl (not needed via PM table — the same value is available via the direct SMN register, see `DETECTION.md`). The 2452-byte table has ~613 potential f32 fields — only 14 are identified across the system-level and per-core maps. AMD does not publish PM table documentation for any generation.

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
