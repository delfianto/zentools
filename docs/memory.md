# Memory Timing Reader

The `zen mem` command reads DDR4/DDR5 memory timings directly from AMD's Unified Memory Controller (UMC) registers via the SMN bus. The output includes primary, secondary, tertiary, refresh, and additional subtimings for every detected channel. `zen mem --raw` includes the captured register values with absolute SMN addresses, including all refresh banks; the display does not reread them.

These are the controller's programmed timings, rather than SPD/EXPO profile specifications or measured memory latency. Primary/secondary/tertiary and refresh timing values are in memory clock cycles; additional fields are shown as encoded by the UMC. `N/A` means unavailable. Power Down Enable is a configuration flag, not a measurement of time spent powered down.

## How It Works

UMC timing registers live in the SMN address space at `0x50000`-`0x50260` per channel. Each channel is offset by `channel_index << 20` (1 MB spacing). Registers are accessed through the PCI host bridge config space:

1. Write target SMN address to PCI config offset `0x60`
2. Read 32-bit result from PCI config offset `0x64`

The register transport requires root access to `/sys/bus/pci/devices/0000:00:00.0/config`. The current CLI also relies on `ryzen_smu` metadata to select DDR5 decoding; without readable driver metadata it falls back to DDR4. Independent CPU/memory-type detection remains follow-up work.

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
| `0x50210` | tFAW DDR4 [6:0], DDR5 [7:0] |
| `0x50214` | tCWL [5:0], tWTRS [12:8], tWTRL [22:16] |
| `0x50218` | tWR DDR4 [6:0], DDR5 [7:0] |
| `0x5021C` | tRCPage [31:20] |
| `0x50220` | tRDRDDD [3:0], tRDRDSD [11:8], tRDRDSC [19:16], tRDRDSCL DDR4 [27:24], DDR5 [29:24] |
| `0x50224` | tWRWRDD [3:0], tWRWRSD [11:8], tWRWRSC [19:16], tWRWRSCL [29:24] |
| `0x50228` | tWRRD [3:0], tRDWR DDR4 [12:8], DDR5 [13:8] |
| `0x50230` | tREFI [15:0] |
| `0x50234` | tMRD [5:0], tMOD [13:8], tMRDPDA [21:16], tMODPDA [29:24] |
| `0x50238` | tXS DDR4 [10:0]/DDR5 [11:0], tDLLK DDR4 [26:16]/DDR5 [27:16] |
| `0x50250` | tSTAG DDR4 [23:16]/DDR5 [26:16] |
| `0x50254` | tXP [5:0], tCPDED DDR4 [19:16]/DDR5 [20:16], tCKE [28:24] |
| `0x50258` | tRDDATAEN [6:0], tPHYWRL DDR4 [12:8]/DDR5 [15:8], tPHYRDL DDR4 [21:16]/DDR5 [23:16], tPHYWRD [26:24] |
| `0x50260`–`0x50264` (DDR4) | tRFC [10:0], tRFC2 [21:11], tRFC4 [31:22] |
| `0x50260`–`0x5026C` (DDR5) | tRFC [15:0], tRFC2 [31:16] |
| `0x502C0`–`0x502CC` (DDR5) | tRFCsb [10:0] |
| `0x50284` (DDR5) | Nitro RX data [9:8], TX data [5:4], control line [1:0] |

Refresh registers contain multiple DIMM/rank banks. The summary selects the first usable non-default bank. DDR4 skips `0x21060138` when another bank is populated; DDR5 skips `0x00C00138`. DDR5 has tRFCsb rather than DDR4's tRFC4. If several populated banks have different timings, inspect `--raw`; the summary does not enumerate separate timings per rank.

For example, the Ryzen 9 9950X inspected on 2026-10-21 returned `0x00C00138` at `0x50260` and `0x50264`, but `0x01E00374` at `0x50268` and `0x5026C`. The correct values are tRFC=884 and tRFC2=480. Applying the old DDR4 decoder to the first register incorrectly produced 312/0/3. `0x005A0186` at `0x502C8` provides tRFCsb=390. Both channels matched.

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
| GDM bit | 11 | 18 |
| Channels per DIMM | 1 | 2 (sub-channels) |
| Memory type detection | CPU generation (Zen 2/3 = DDR4) | CPU generation (Zen 4/5 = DDR5) |

## Load comparison

UMC timings normally remain stable when CPU load changes at a fixed memory configuration. SMU PM tables contain dynamic telemetry; use those for idle/load/core-residency investigation. A configured power-down bit does not prove the memory is currently powered down.

Capture several samples in each phase: idle, sustained load, then recovery after stopping the load. Keep EPP, BIOS/PBO settings, worker count, FFT size, and AVX mode fixed. Use a small in-place FFT for an initial CPU-focused comparison, then repeat separately with a larger memory workload. A short run validates telemetry behavior, not long-term hardware stability. See the installed mprime `hlp/CommandLine.html` and `hlp/Undoc.html` for `-t`, `MinTortureFFT`, `MaxTortureFFT`, and `TortureMem`.

Run this from the repository after building, and call `capture idle`, `capture load` while mprime is running, then `capture recovery` after stopping it:

```bash
cargo build
capture_dir=$(mktemp -d /tmp/zentools-capture.XXXXXX)
capture() {
    local phase="$1"
    mkdir "$capture_dir/$phase" || return
    sudo target/debug/zentools smu info > "$capture_dir/$phase/info.txt" || return
    sudo target/debug/zentools epp show > "$capture_dir/$phase/epp.txt" || return
    sudo target/debug/zentools mem --raw > "$capture_dir/$phase/memory.txt" || return
    for i in $(seq 1 10); do
        date --iso-8601=ns > "$capture_dir/$phase/time-$i.txt"
        sudo target/debug/zentools smu pm-table -f --raw > "$capture_dir/$phase/pm-$i.txt" || return
        cat /proc/stat > "$capture_dir/$phase/cpu-$i.txt"
        sensors > "$capture_dir/$phase/sensors-$i.txt"
        sleep 1
    done
}
capture idle
```

The full hex dump preserves every PM-table byte. Its “Notable Float Values” section currently shows only the first 64 floats, so it misses most per-core data. Compare all aligned 4-byte offsets, record raw integers alongside f32 interpretations, and reject version/size mismatches. A changing float is only a candidate: require repeatable correlation with independent temperature, RAPL power, CPU utilization, or pinned-core experiments before assigning a name.

## Remaining work

- Detect memory type independently of SMU metadata; avoid silent DDR4 fallback on unsupported/absent metadata.
- Decode per-rank refresh banks individually when populated values differ.
- Expose additional DDR5 fields (their register definitions are available upstream). Nitro RX/TX/control is now shown and agrees with the inspected firmware getters (2/3/1).
- Integrate the validated AOD getter transport for electrical settings such as ProcODT, RTT and drive strengths. A board/firmware-specific, read-only research probe is available in [tools/aod-probe](../tools/aod-probe/README.md); see [probing.md](probing.md) for findings and interpretation limits.
- Obtain a DDR4 hardware capture. DDR4 changes are checked against the upstream definitions and unit fixtures; live validation here used DDR5 on a 9950X.

## Register Sources

- UMC timing register layout: [irusanov/ZenTimings](https://github.com/irusanov/ZenTimings)
- UMC register definitions: [DDR4 dictionary](https://github.com/irusanov/ZenStates-Core/blob/869b7e82572337e5a673e92230edc92bddf0e44b/Dictionaries/DDR4Dictionary.cs), [DDR5 dictionary](https://github.com/irusanov/ZenStates-Core/blob/869b7e82572337e5a673e92230edc92bddf0e44b/Dictionaries/DDR5Dictionary.cs), and [DDR5 refresh-bank handling](https://github.com/irusanov/ZenStates-Core/blob/869b7e82572337e5a673e92230edc92bddf0e44b/Hardware/DRAM/Ddr5Timings.cs), checked 2026-10-21
- DDR5 differences verified on AMD Ryzen 9 9950X (Granite Ridge)
