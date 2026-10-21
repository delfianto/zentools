# Read-only AOD research probe

This prototype queries AMD firmware memory settings through its existing WMI
getters. Tested on a Ryzen 9 9950X, MSI MPG X870E CARBON WIFI (MS-7E49), BIOS
1.AA3, with the AOD SSDT captured on 2026-10-21.

The runner requires the exact audited SSDT SHA-256. ACPI tables contain boot
addresses and initialized settings, so another boot or BIOS configuration can
change the hash even on the same BIOS version. A mismatch needs a new AML
review; there is no override flag.

## Decode the included capture

No privileges, hardware access, compiler, or kernel module required:

```bash
python3 tools/aod-probe/probe.py --decode tools/aod-probe/fixtures/ms7e49-1aa3.txt
python3 -m unittest discover -s tools/aod-probe -p 'test_*.py'
```

Python 3.12 or newer is required. To check all research tooling:

```bash
just lint-tools     # or: just lint-python / just lint-c
```

Python uses Ruff and basedpyright (warnings fail the check). The C helper uses
Clang-Format, strict Linux `checkpatch`, a kernel build with `W=1` and
`KCFLAGS=-Werror`, and Clang-Tidy's static analyzer. C checks require matching
kernel headers, LLVM/Clang, Clang-Tidy, Clang-Format, `make`, Perl and Python.
They build in a temporary directory and never load the helper or require root.
Use `KDIR=/path/to/kernel/build just lint-c` to select alternate headers. Kernel
header diagnostics are outside Clang-Tidy's project-header filter.

## Make another live capture

Requires matching running-kernel headers, `make`, the kernel's compiler/toolchain,
and root access. The runner uses `sudo -n` when invoked as a regular user.

```bash
sudo -v
python3 tools/aod-probe/probe.py
# Or select a new output directory:
python3 tools/aod-probe/probe.py --output /tmp/aod-new-capture
```

The script verifies board, BIOS and AML hash, builds the helper in the capture
directory, loads it, reads its cached response prefixes, and unloads it in a
`finally` block. It refuses to replace an already loaded helper. It does not
install files or change BIOS, memory, CPU, or power settings. Keep the capture
directory: it includes the AML, helper source/build, raw responses, metadata,
decoded JSON and a Markdown report. The included fixture is a saved capture,
not a live view of the current settings.

The helper is a separate GPL-2.0-only kernel module using Linux's exported WMI
API. It is a research tool, not part of the normal ZenTools build or install.

## Audited firmware path

`SSDT11` declares `AODE`, a 9403-byte SystemMemory region inside ACPI NVS. On
the inspected boot its address was `0x9948B018`. `/dev/mem` rejected the read.
The firmware itself provides this supported alternative:

```text
WMI GUID 95F20B61-3D44-41F8-87E1-03CBF94C4E8F
  WMAA(instance=0, method=4, object_id) -> AM04, enum getter
  WMAA(instance=0, method=6, object_id) -> AM06, linear getter
```

Both getters copy a value from `WCNS` into a response buffer. Object IDs,
indices and descriptions come from the firmware's OBIT/OBID/OBIE packages.
`objects.h` is the fixed compiled allowlist; `objects.json` preserves metadata.
Many firmware descriptions begin with "Set", but these queries use getter
methods 4 and 6. Method 5 calls AM05, writes command fields and triggers an SMI;
it is excluded from this helper. There are no writable module parameters or
proc attributes, and no arbitrary physical-memory access.

Raw response format, one line per object:

```text
object_id method acpi_status response_length word0 word1 word2 word3 word4
```

IDs, status and words are hexadecimal; method and length are decimal. `word0`
echoes the object ID; `word1` is the returned setting. A linear response is 20
bytes (ID, current, minimum, maximum, step). An enum response is 412 bytes; only
its first five words are retained, not the full option list. The decoder checks
the allowlist, method, status, length, ID echo, duplicates and completeness.

## Interpretation

These are **firmware-reported settings**, not electrical measurements. For
example, a voltage field reports a configured value rather than PMIC telemetry.
Zero in an override field can mean firmware control/Auto; it does not establish
that the physical voltage or power limit is zero. We preserve raw values for
all 81 objects and only decode selected fields with known encodings.

The captured 30/36/36/76 primaries, 884/480/390 refresh timings and Nitro 2/3/1
agree with independent UMC reads. ProcODT and RTT encodings are consistent with
the upstream dictionaries, but still need a BIOS or second-tool comparison.

Sources audited:

- [ZenStates AOD reader](https://github.com/irusanov/ZenStates-Core/blob/869b7e82572337e5a673e92230edc92bddf0e44b/Hardware/Aod/Aod.cs)
- [Electrical value dictionaries](https://github.com/irusanov/ZenStates-Core/blob/869b7e82572337e5a673e92230edc92bddf0e44b/Dictionaries/EncodedValueDictionaries.cs)
- Local AOD SSDT SHA-256: `219659fefb95ff23dded68aac16ff6af35ffa2863baf4999172250d7788d2f96`

For a new firmware revision, disassemble the table with ACPICA `iasl -d`, inspect
the getter dispatch and bodies, confirm object response layouts, and record a
new capture and supported profile. Do not generalize this allowlist based on
the WMI GUID alone.
