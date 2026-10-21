# Working on ZenTools

ZenTools is a Linux CLI for AMD Ryzen EPP control, SMU telemetry, and DDR4/DDR5
memory timings. Read [README.md](README.md) for usage and
[docs/architecture.md](docs/architecture.md) for the design.

## Repository map

- `src/main.rs`: Clap definitions and multicall dispatch for `zentools`,
  `zen-epp`, `zen-smu`, and `zen-mem`.
- `src/cmd/`: CLI handlers, tables, and the Ratatui monitor.
- `src/epp.rs`: EPP profiles and sysfs access.
- `src/smu/`: hardware access, CPU identification, PM-table decoding, memory
  timings, and telemetry fallback logic; exported through `src/lib.rs`.
- `docs/`: architecture, detection, dependencies, register maps, and research
  findings. Keep every filename in this directory lowercase and update links
  when renaming files.
- `tools/aod-probe/`: a separate Python/C research tool with an optional,
  read-only WMI kernel helper, audited firmware metadata, and a saved fixture.
  It is outside the normal Cargo build and installation.

## Build and validation

Use the Rust version required by `Cargo.toml` (currently 1.98 or newer, edition
2024). Run commands from the repository root.

```bash
just build --debug       # cargo build; no UPX or installation
just format --apply      # rustfmt
just full-gate           # format check, Clippy with warnings denied, Rust tests
```

Equivalent Rust checks, when `just` is unavailable:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

For AOD probe or decoder changes, also run the offline fixture tests:

```bash
just lint-tools        # Ruff, basedpyright, kernel C checks and Clang analysis
python3 -m unittest discover -s tools/aod-probe -p 'test_*.py'
```

`just lint-python` and `just lint-c` run the language checks separately. Python
tooling targets Python 3.12 or newer. C checks need matching kernel headers,
LLVM/Clang, Clang-Tidy, Clang-Format, `make`, Perl, and Python. They build in a
temporary directory, require no sudo, and do not load the module. `KDIR` can
select a different kernel build directory.

Documentation-only changes need filename, link, and diff checks; do not run
hardware probes for them. Use regression tests with captured or synthetic input
for decoding changes. Compilation, ordinary tests, and offline fixture decoding
do not require sudo.

`just build` creates a native release build and requires UPX. `just install`
builds and installs the binary and symlinks; use it when installation is part
of the task. There is no configured `just test --integration` suite.

The `justfile` uses a shared baseline/layers/exceptions structure. Preserve its
baseline recipe bodies and `{{…}}` spacing; use knobs or scoped exceptions for
project-specific changes. Follow the actual recipes when README examples drift.

## Implementation conventions

- Keep register access and decoding in `src/smu/`, with presentation in
  `src/cmd/`. Keep Clap and display formatting out of the library modules.
- Use `SmuError` for library hardware errors and `anyhow::Result` at CLI
  boundaries. Missing readings must stay unavailable rather than becoming
  plausible zeros, disabled flags, or invented sensor values.
- Preserve Linux platform guards and useful errors/fallbacks on other systems.
- Keep DDR4 and DDR5 layouts explicit. Distinguish MHz, MT/s, clock cycles,
  encoded settings, active frequency, and effective frequency.
- Preserve unrelated work already in the checkout. Keep generated builds,
  Python caches, kernel modules, and bulk captures out of source control;
  retain small, purposeful fixtures used by regression tests.

## Hardware investigation

Use the existing session's authorization for live work. Perform stress tests,
privileged reads, module loading, or configuration changes only when relevant
to the requested task; ordinary validation should use offline inputs.

- Monitoring paths should remain read-only with respect to hardware settings.
  SMN reads require selecting an address through the PCI index register; this
  is distinct from writing a target register or sending a firmware setter.
- Identify CPU, BIOS/firmware, PM-table version, and size before interpreting
  data. Do not assume neighboring CPU generations or table versions share a
  layout.
- Keep raw bytes/registers, units, capture conditions, and source provenance
  alongside new findings. Render raw UMC values from the same readings used
  for decoding, including channel addresses and refresh banks.
- Validate candidate fields against independent sources and repeatable,
  controlled experiments. Correlation under load alone does not establish a
  field's name, unit, filtering window, or behavior at idle.
- Treat firmware settings, SPD profiles, trained controller timings, and live
  electrical measurements as distinct sources. Preserve raw unknown encodings.
- Honor the AOD probe's board, BIOS, AML-hash, object, and getter restrictions.
  Review changed firmware before adding a supported profile; do not weaken
  guards to make a probe run.
- Bound stress-test duration, isolate temporary configurations, and clean up
  processes and helpers created by the experiment. Preserve an existing
  workload or module owned by the user.
- Consult primary upstream implementations and record pinned revisions for
  imported register knowledge. Check licenses before incorporating source.

See [docs/memory.md](docs/memory.md), [docs/pmtable.md](docs/pmtable.md),
[docs/probing.md](docs/probing.md), and the
[AOD probe guide](tools/aod-probe/README.md) for current evidence and limits.
