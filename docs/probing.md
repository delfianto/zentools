# Further probing on the Ryzen 9 9950X

Findings from 2026-10-21, MSI MPG X870E CARBON WIFI (MS-7E49), BIOS 1.AA3,
microcode `0x0B404038`, SMU 98.84.0, PM table `0x620205` (2452 bytes).

## Memory settings: a working firmware interface

The AOD ACPI SSDT contains both a firmware data region and callable getters.
This boot's declaration was:

```asl
OperationRegion (AODE, SystemMemory, 0x9948B018, 0x24BB)
```

The region is inside ACPI non-volatile storage. A bounded read through
`/dev/mem` failed with `EPERM`. Disassembling the SSDT with ACPICA `iasl`
revealed an existing supported getter path:

| WMI method | AML method | Behavior in the inspected firmware |
|------------|------------|-------------------------------------|
| 4 | AM04 | Copy an enum setting from WCNS into a response buffer |
| 6 | AM06 | Copy a linear setting from WCNS into a response buffer |
| 5 | AM05 | Write command fields and trigger an SMI; excluded from the probe |

WMI GUID: `95F20B61-3D44-41F8-87E1-03CBF94C4E8F`, instance 0. The firmware's
OBID/OBIE packages associate object IDs with names; OBIT supplies getter type,
WCNS index and response templates. Thus the actual firmware explains the
layout instead of requiring a frequency-pattern search in a memory dump.

A small read-only kernel helper queried the 81 inspected objects once through
Linux's WMI API. All returned successful ACPI status, the expected response
length and an ID echo. The helper cached response prefixes, exposed no setter
or arbitrary method parameters, and was unloaded after capture. A second
capture through the checked-in runner reproduced the results.

| Firmware setting | Returned value | Raw encoding |
|------------------|----------------|--------------|
| ProcODT pull-up P0 | 40 ohm | 28 |
| ProcODT pull-down P0 | 43.6 ohm | 15 |
| DRAM drive pull-up P0 | 48 ohm | 2 |
| DRAM drive pull-down P0 | 40 ohm | 1 |
| RTT nominal write/read | RZQ/7 (~34.3 ohm nominal) | 7 / 7 |
| RTT write/park | RZQ/3 (80 ohm nominal) | 3 / 3 |
| RTT park DQS | RZQ/4 (60 ohm nominal) | 4 |
| VDDIO setting | 1.350 V | 1350 mV |
| DIMM VDDQ setting | 1.350 V | 1350 mV |
| VPP setting | 1.850 V | 1850 mV |
| APU VDDIO setting | 1.340 V | 1340 mV |
| Nitro RX / TX / control | 2 / 3 / 1 | 2 / 3 / 1 |

These are **firmware-reported settings**, not physical voltage measurements or
proof of trained electrical behavior. The electrical encodings agree with the
[upstream dictionaries](https://github.com/irusanov/ZenStates-Core/blob/869b7e82572337e5a673e92230edc92bddf0e44b/Dictionaries/EncodedValueDictionaries.cs),
but need a BIOS or second-tool comparison before being treated as confirmed
electrical values.

Independent validation is available for the ordinary timing fields: the AOD
getters return MCLK 3000, primaries 30/36/36/76, and refresh timings 884/480/390,
matching direct UMC reads. Nitro also matches SMN `0x50284` = `0x00000231` on
both channels, so Nitro has been added to `zentools mem`.

Firmware PPT/TDC/EDC override getters return zero, while the SMU contains
nonzero limits and consumption. This illustrates why raw firmware override
values must not automatically become live sensor readings. Other settings
with raw zero are preserved without inferring actual electrical behavior.

The transport and saved fixture are in [tools/aod-probe](../tools/aod-probe/README.md).
It deliberately requires the exact audited AML hash; the main ZenTools binary
does not load a kernel module or automatically execute these methods.

## Per-core frequency candidate

LibreHardwareMonitor defines a frequency-array base at float index 349
(`0x574`) for table `0x621202`. The existing power, voltage and temperature
array bases match our `0x620205`, making this a useful candidate rather than
proof of identical layouts.

Pinned a single Python CPU worker to logical CPU 0 (physical core 0, CCD 0),
then to logical CPU 8 (physical core 8, CCD 1), with idle and recovery phases.
Captured 44 PM tables at one-second intervals. The sampler ran on CPU 15 to
avoid migrating onto the targeted cores. Independent active-frequency estimates
used APERF/MPERF deltas and the kernel's CPPC nominal frequency of 4300 MHz;
reading those counters itself slightly disturbs idle behavior.

| Loaded physical core | PM candidate mean | APERF/MPERF mean | cpufreq mean | C0 mean |
|----------------------|-------------------|-----------------|--------------|---------|
| 0 | 5580 MHz | 5611 MHz | 5610 MHz | 98.2% |
| 8 | 5375 MHz | 5418 MHz | 5415 MHz | 98.2% |

The targeted core's candidate agrees within ~1% under sustained load. Idle
values differ more: for example, initial core 8 means were 3670 MHz in the
candidate versus 4466 MHz from APERF/MPERF. Sampling windows and active/idle
semantics differ; this is not sufficient to call the PM value a wall-time
effective frequency or to replace the existing frequency source.

The old float-index-461 candidate remains disproven. No new frequency mapping
has been enabled in production.

Sources: [LibreHardwareMonitor CPU implementation](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/blob/617ab24e0c38a0df907cbe3506bf2f1a0d23d4a9/LibreHardwareMonitorLib/Hardware/Cpu/Amd17Cpu.cs),
[Linux APERF/MPERF implementation](https://github.com/torvalds/linux/blob/master/arch/x86/kernel/cpu/aperfmperf.c).

## Repeated power transitions

Ran two 12-second mprime 16K in-place AVX-512 load pulses, with 9-second idle
or recovery phases, retaining 51 PM-table and independent RAPL samples at
one-second intervals. No EPP, power-limit or BIOS changes were made.

The three candidates `0x00C`, `0x068` and `0x454` tracked package RAPL closely
in both repetitions. Means excluding the first three samples of each phase:

| Phase | RAPL | `0x00C` | `0x068` | `0x454` |
|-------|------|---------|---------|---------|
| Initial idle | 48.27 W | 48.28 | 48.28 | 48.32 |
| First load | 183.58 W | 183.61 | 183.61 | 183.48 |
| First recovery | 51.80 W | 51.81 | 51.81 | 51.56 |
| Second load | 179.93 W | 179.93 | 179.93 | 179.92 |
| Second recovery | 56.76 W | 56.73 | 56.76 | 56.24 |

LibreHardwareMonitor calls `0x00C` PPT Slow and `0x068` PPT Fast for its
`0x621202` map. At our one-second cadence they are nearly indistinguishable,
including transitions. The differing window of `0x454` is visible briefly on
load onset, but its exact meaning remains unknown. We have strengthened the
power interpretation without establishing every field's filtering semantics.
The candidates remain unassigned in production.

The pulses stopped cleanly, with zero errors/warnings reported; they were too
short to complete the FFT self-tests. This was a telemetry experiment.

## Next discriminating experiments

1. Compare the firmware electrical settings with the BIOS display or a
   ZenTimings report from the same configuration. Retain raw codes and source
   labels in the eventual UI.
2. Capture a second BIOS configuration with one deliberate memory-setting
   change, after saving the original configuration. Verify that the intended
   getter and UMC field change together; obtain a fresh AML audit if its hash
   changes.
3. Repeat the frequency experiment with fixed scalar and vector workloads,
   aligned counter windows, and different active duty cycles. Distinguish
   active clock, sampled clock and wall-time effective frequency.
4. Separate power-limit setters from consumption by a controlled limit
   experiment only when that configuration change is intended. The zero
   firmware overrides and differing PM schemas rule out wholesale map copying.

Local evidence: `/tmp/zentools-probing-20261021/`. `aod.dsl` is the disassembly,
`reproducible-capture/` includes decoded settings and raw getter prefixes,
`frequency/` contains the pinned-core experiment, and `power/transitions.csv`
contains the repeated power response. The checked-in AOD fixture preserves the
81 successful responses even after temporary files are removed.

## Integration into the application

The audited getter protocol is now decoded in Rust (`src/aod.rs`) and exposed
through `zentools firmware --capture <file-or-directory>` and the GUI's Memory
page. All 81 raw codes are retained; selected electrical encodings remain
explicitly provisional. The GUI never builds or loads the research module.
An already loaded helper's cached results can be read only after the existing
board/BIOS/AML checks succeed; imported captures are historical settings.

`zentools capture --output <file>` and `zentools compare <before> <after>` add
versioned idle/load comparison. Captures retain UMC registers and the exact
PM-table bytes used to parse telemetry. Full PM words are compared only with
matching table versions and lengths. The three power candidates and 16 clock
candidates appear under experimental PM fields for the exact audited schema;
they remain outside production sensor mappings.

See [gui.md](gui.md) for the Qt frontend and the capture workflow.
