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

**Unmapped**: PPT/TDC/EDC, Tctl, per-core power/frequency/temperature. The 2452-byte table has ~613 potential f32 fields — only 8 are identified. AMD does not publish PM table documentation for any generation.

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
