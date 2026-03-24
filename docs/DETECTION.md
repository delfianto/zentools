# CPU Detection and Data Sources

## CPU Identification

The ryzen_smu kernel driver provides a numeric codename value at `/sys/kernel/ryzen_smu_drv/codename`. This maps to the `CpuCodename` enum in `src/smu/types.rs`:

| Value | Codename | Generation | Segment |
|-------|----------|------------|---------|
| 4 | Matisse | Zen 2 | Desktop |
| 12 | Vermeer | Zen 3 | Desktop |
| 20 | Raphael | Zen 4 | Desktop |
| 21 | Phoenix | Zen 4 | Mobile |
| 22 | Strix Point | Zen 5 | Mobile |
| 23 | Granite Ridge | Zen 5 | Desktop |
| 24 | Hawk Point | Zen 4 | Mobile |
| 25 | Storm Peak | Zen 4 | HEDT |

Full mapping: 25 codenames from Zen through Zen 5. See `CpuCodename::from_u32()`.

## Tiered Data Source Architecture

### Tier 1: Direct Hardware Registers (no kernel module needed)

**SMN (System Management Network)** — accessed via PCI config space on the host bridge (`0000:00:00.0`):
- Write target SMN address to PCI offset `0x60`
- Read result from PCI offset `0x64`
- Used for: Tctl temperature, CCD temperatures, SVI3 voltage telemetry
- Requires: root access to PCI config space

**MSR (Model-Specific Registers)** — accessed via `/dev/cpu/N/msr`:
- `0xC0010299`: RAPL Power Unit (energy unit exponent in bits 12:8)
- `0xC001029B`: Package Energy Status (32-bit counter, wraps)
- `0xC001029A`: Core Energy Status (unavailable on Zen 5 desktop)
- Power = energy_delta * energy_unit / time_delta
- Requires: root + `msr` kernel module (`modprobe msr`)

### Tier 2: ryzen_smu Kernel Driver

Reads from `/sys/kernel/ryzen_smu_drv/`:
- `version` — SMU firmware version (text)
- `codename` — CPU codename number (text)
- `pm_table` — raw PM table binary blob
- `pm_table_version` — 4-byte LE version identifier
- `pm_table_size` — 8-byte LE size in bytes

### Tier 3: Hybrid / Fallback

The orchestrator in `smu/mod.rs` (`read_metrics()`) tries the PM table first, then fills gaps with direct register reads. The `MetricsSource` enum tracks what was used: `PmTable`, `DirectRegisters`, or `Hybrid`.

## Memory Type Detection

Memory type (DDR4 vs DDR5) is determined from CPU generation, NOT from register probing. Register-based detection is unreliable — on Zen 5, DDR4 DIMM detection registers (`0x50000`/`0x50008`) still respond for DDR5 DIMMs.

| CPU Generation | Memory Type |
|---------------|-------------|
| Zen / Zen+ / Zen 2 / Zen 3 | DDR4 |
| Zen 4 (Raphael, Phoenix, HawkPoint, StormPeak) | DDR5 |
| Zen 5 (Granite Ridge, Strix Point) | DDR5 |

See `CpuCodename::is_ddr5()` in `types.rs`.

## Temperature Register Differences

| Register | Zen 1-4 | Zen 5 |
|----------|---------|-------|
| Tctl | `0x00059800` | `0x00059800` (same) |
| CCD temp base | `0x00059954` | `0x00059B08` |
| SVI core | varies | `0x00073010` |
| SVI SoC | varies | `0x00073014` |

Temperature formula: `(reg >> 21) * 125` millidegrees, subtract 49000 if bit 19 set.
CCD formula: `(reg & 0x7FF) * 125 - 49000` millidegrees, valid only if bit 11 set.
