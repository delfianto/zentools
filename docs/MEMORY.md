# Memory Timing Reader

The `zen mem` command reads DDR4/DDR5 memory timings directly from AMD's Unified Memory Controller (UMC) registers via the SMN bus. This is the Linux equivalent of [ZenTimings](https://github.com/irusanov/ZenTimings).

## How It Works

UMC timing registers live in the SMN address space at `0x50000`-`0x50260` per channel. Each channel is offset by `channel_index << 20` (1 MB spacing). Registers are accessed through the PCI host bridge config space:

1. Write target SMN address to PCI config offset `0x60`
2. Read 32-bit result from PCI config offset `0x64`

No kernel module needed — just root access to `/sys/bus/pci/devices/0000:00:00.0/config`.

## Register Map

### Configuration (`0x50200`)

| Bits | DDR4 | DDR5 |
|------|------|------|
| [6:0] | Ratio (freq = ratio/3 * 200) | — |
| [15:0] | — | MCLK in MHz (freq = MCLK * 2) |
| [10] | Cmd2T | — |
| [11] | GDM | — |
| [17] | — | Cmd2T |
| [18] | — | GDM |

### Timing Registers

| Address | Fields (bit ranges) |
|---------|-------------------|
| `0x50204` | tCL [5:0], tRAS [14:8], tRCDRD [21:16], tRCDWR [29:24] |
| `0x50208` | tRC [7:0], tRP [21:16] |
| `0x5020C` | tRRDS [4:0], tRRDL [12:8], tRTP [28:24] |
| `0x50210` | tFAW [7:0] |
| `0x50214` | tCWL [5:0], tWTRS [12:8], tWTRL [22:16] |
| `0x50218` | tWR [7:0] |
| `0x50220` | tRDRDDD [3:0], tRDRDSD [11:8], tRDRDSC [19:16], tRDRDSCL [29:24] |
| `0x50224` | tWRWRDD [3:0], tWRWRSD [11:8], tWRWRSC [19:16], tWRWRSCL [29:24] |
| `0x50228` | tWRRD [3:0], tRDWR [13:8] |
| `0x50230` | tREFI [15:0] |
| `0x50260` | tRFC [10:0], tRFC2 [21:11], tRFC4 [31:22] |

### Channel and DIMM Detection

| Register | Purpose |
|----------|---------|
| `0x50DF0` | Channel enabled (bit 19: 0=enabled, 1=disabled) |
| `0x50000` | DDR4 DIMM slot 0 present (bit 0) |
| `0x50008` | DDR4 DIMM slot 1 present (bit 0) |
| `0x50020` | DDR5 DIMM slot 0 present (bit 0) |
| `0x50028` | DDR5 DIMM slot 1 present (bit 0) |

Note: On Zen 5 with DDR5, the DDR4 detection addresses (`0x50000`/`0x50008`) still report DIMM presence correctly. The DDR5-specific addresses may not use bit 0 for presence on all platforms. The code tries DDR4 addresses first as a reliable fallback.

## DDR4 vs DDR5

| Aspect | DDR4 | DDR5 |
|--------|------|------|
| Frequency encoding | Ratio in bits [6:0] | MCLK in MHz in bits [15:0] |
| Cmd2T bit | 10 | 17 |
| GDM bit | 18 | 18 |
| Channels per DIMM | 1 | 2 (sub-channels) |
| Memory type detection | CPU generation (Zen 2/3 = DDR4) | CPU generation (Zen 4/5 = DDR5) |

## Register Sources

- UMC timing register layout: [irusanov/ZenTimings](https://github.com/irusanov/ZenTimings)
- UMC register definitions: [irusanov/ZenStates-Core](https://github.com/irusanov/ZenStates-Core)
- DDR5 differences verified on AMD Ryzen 9 9950X (Granite Ridge)
