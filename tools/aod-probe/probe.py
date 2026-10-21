#!/usr/bin/env python3
"""Capture inspected AMD AOD getters, or decode a saved research capture.

This prototype is deliberately restricted to the audited MS-7E49 SSDT. It does
not call firmware setter method 5 or access physical memory through /dev/mem.
"""

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import tempfile
from collections.abc import Sequence
from pathlib import Path
from typing import Literal, NotRequired, TypedDict, cast

HERE = Path(__file__).resolve().parent
TABLE_SHA256 = "219659fefb95ff23dded68aac16ff6af35ffa2863baf4999172250d7788d2f96"
MODULE = "zentools_aod_probe"
BOARD = "MPG X870E CARBON WIFI (MS-7E49)"
BIOS = "1.AA3"
PROC_ODT = {
    0: "Hi-Z",
    1: "480",
    2: "240",
    3: "160",
    4: "120",
    5: "96",
    6: "80",
    7: "68.6",
    12: "60",
    13: "53.3",
    14: "48",
    15: "43.6",
    28: "40",
    29: "36.9",
    30: "34.3",
    31: "32",
    60: "30",
    61: "28.2",
    62: "26.7",
    63: "25.3",
}

# Object IDs are taken from this machine's OBID/OBIE declarations. Firmware
# names containing "Set" describe settings; the invoked methods are getters.
DISPLAY = {
    0x20051: ("ProcODT pull-up P0", "proc_odt"),
    0x20052: ("ProcODT pull-down P0", "proc_odt"),
    0x20053: ("DRAM drive pull-up P0", "dram_drive"),
    0x20054: ("DRAM drive pull-down P0", "dram_drive"),
    0x2003E: ("RTT nominal write", "rtt"),
    0x2003F: ("RTT nominal read", "rtt"),
    0x20023: ("RTT write", "rtt"),
    0x20024: ("RTT park", "rtt"),
    0x20040: ("RTT park DQS", "rtt"),
    0x30001: ("VDDIO setting", "mv"),
    0x30007: ("DIMM VDDQ setting", "mv"),
    0x30003: ("VPP setting", "mv"),
    0x30004: ("APU VDDIO setting", "mv"),
    0x2004D: ("Nitro RX", "raw"),
    0x2004E: ("Nitro TX", "raw"),
    0x2004F: ("Nitro control", "raw"),
}


class ObjectDefinition(TypedDict):
    id: int
    method: Literal[4, 6]
    index: int
    length: int
    name: str


class AodRecord(TypedDict):
    id: str
    firmware_name: str
    method: Literal[4, 6]
    length: int
    raw: int
    response_prefix: list[int]
    name: NotRequired[str]
    decoded: NotRequired[str]


class Arguments(argparse.Namespace):
    decode: Path | None = None
    output: Path | None = None


def load_objects() -> list[ObjectDefinition]:
    """Validate the JSON schema at the serialization boundary."""
    raw = cast(object, json.loads((HERE / "objects.json").read_text()))
    if not isinstance(raw, list):
        raise TypeError("expected an AOD object list")
    objects: list[ObjectDefinition] = []
    for entry in cast(list[object], raw):
        if not isinstance(entry, dict):
            raise TypeError("expected an AOD object definition")
        item = cast(dict[str, object], entry)
        object_id, method = item.get("id"), item.get("method")
        index, length, name = item.get("index"), item.get("length"), item.get("name")
        if (
            not isinstance(object_id, int)
            or not isinstance(method, int)
            or method not in (4, 6)
            or not isinstance(index, int)
            or not isinstance(length, int)
            or not isinstance(name, str)
        ):
            raise ValueError("invalid AOD object definition")
        objects.append(
            {
                "id": object_id,
                "method": method,
                "index": index,
                "length": length,
                "name": name,
            }
        )
    return objects


def decoded_value(value: int, kind: str) -> str:
    if kind == "proc_odt":
        text = PROC_ODT.get(value)
        return f"{text} ohm" if text and value else text or f"unknown code {value}"
    if kind == "dram_drive":
        return {0: "34 ohm", 1: "40 ohm", 2: "48 ohm"}.get(
            value, f"unknown code {value}"
        )
    if kind == "rtt":
        if value == 0:
            return "Off"
        if 1 <= value <= 7:
            return f"RZQ/{value} ({240 / value:g} ohm, nominal)"
        return f"unknown code {value}"
    if kind == "mv":
        return f"{value / 1000:.3f} V"
    return str(value)


def decode(text: str) -> list[AodRecord]:
    """Check the helper protocol before interpreting any captured values."""
    objects = {item["id"]: item for item in load_objects()}
    records: dict[int, AodRecord] = {}
    for line in text.splitlines():
        parts = line.split()
        if len(parts) != 9:
            raise ValueError("expected nine columns in each AOD record")
        object_id = int(parts[0], 16)
        method, status, length = int(parts[1]), int(parts[2], 16), int(parts[3])
        words = [int(word, 16) for word in parts[4:]]
        obj = objects.get(object_id)
        if obj is None or object_id in records:
            raise ValueError(f"unknown or duplicate object {object_id:08x}")
        if method != obj["method"] or method not in (4, 6):
            raise ValueError(f"unexpected getter for {object_id:08x}")
        if status or length != obj["length"] or words[0] != object_id:
            raise ValueError(f"invalid firmware response for {object_id:08x}")
        record: AodRecord = {
            "id": f"0x{object_id:08X}",
            "firmware_name": obj["name"],
            "method": obj["method"],
            "length": length,
            "raw": words[1],
            "response_prefix": words,
        }
        if object_id in DISPLAY:
            name, kind = DISPLAY[object_id]
            record.update(name=name, decoded=decoded_value(words[1], kind))
        records[object_id] = record
    if records.keys() != objects.keys():
        raise ValueError("capture is missing AOD objects")
    return list(records.values())


def report(records: Sequence[AodRecord]) -> str:
    lines = [
        "AMD AOD firmware settings (captured values, not voltage measurements)",
        "",
        "| Setting | Value | Raw code |",
        "|---|---|---|",
    ]
    by_id = {int(row["id"], 16): row for row in records}
    for object_id, (name, kind) in DISPLAY.items():
        row = by_id[object_id]
        lines.append(f"| {name} | {decoded_value(row['raw'], kind)} | {row['raw']} |")
    return "\n".join(lines) + "\n"


def privileged(args: Sequence[str], *, capture_output: bool = False) -> bytes:
    prefix = [] if os.geteuid() == 0 else ["sudo", "-n"]
    result = subprocess.run(
        prefix + list(args),
        check=True,
        stdout=subprocess.PIPE if capture_output else None,
    )
    return result.stdout if capture_output else b""


def capture(output: Path) -> list[AodRecord]:
    output.mkdir(parents=True, exist_ok=False)
    board = Path("/sys/class/dmi/id/board_name").read_text().strip()
    bios = Path("/sys/class/dmi/id/bios_version").read_text().strip()
    if (board, bios) != (BOARD, BIOS):
        raise ValueError("this prototype only supports the audited MS-7E49 BIOS 1.AA3")
    if Path(f"/sys/module/{MODULE}").exists():
        raise ValueError(
            "probe module is already loaded; refusing to replace or unload it"
        )
    table = None
    for path in sorted(Path("/sys/firmware/acpi/tables").glob("SSDT*")):
        data = privileged(["cat", str(path)], capture_output=True)
        if hashlib.sha256(data).hexdigest() == TABLE_SHA256:
            table = path.name, data
            break
    if table is None:
        raise ValueError(
            "AOD AML differs from the audited capture; review its getters before use"
        )
    name, data = table
    if len(data) != int.from_bytes(data[4:8], "little") or sum(data) % 256:
        raise ValueError("invalid ACPI table size or checksum")
    _ = (output / "aod.aml").write_bytes(data)
    metadata = {
        "board": board,
        "bios": bios,
        "table": name,
        "table_sha256": TABLE_SHA256,
        "boot_id": Path("/proc/sys/kernel/random/boot_id").read_text().strip(),
        "kernel": platform.release(),
        "methods": [4, 6],
        "source": "AOD WCNS firmware settings; not live voltage telemetry",
    }
    build = output / "build"
    build.mkdir()
    for filename in ("zentools_aod_probe.c", "objects.h", "Makefile"):
        _ = shutil.copyfile(HERE / filename, build / filename)
    command = ["make", "-C", f"/lib/modules/{platform.release()}/build", f"M={build}"]
    if "clang" in Path("/proc/version").read_text().lower():
        command.append("LLVM=1")
    _ = subprocess.run(command + ["modules"], check=True)
    # Finish loading without a KeyboardInterrupt gap before the cleanup scope.
    try:
        _ = privileged(["insmod", str(build / f"{MODULE}.ko")])
        text = Path(f"/proc/{MODULE}").read_text()
        _ = (output / "aod-getters.txt").write_text(text)
    finally:
        if Path(f"/sys/module/{MODULE}").exists():
            _ = privileged(["rmmod", MODULE])
    records = decode(text)
    _ = (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    _ = (output / "settings.json").write_text(json.dumps(records, indent=2) + "\n")
    _ = (output / "settings.md").write_text(report(records))
    return records


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument(
        "--decode",
        type=Path,
        help="decode an existing raw capture without hardware access",
    )
    _ = parser.add_argument(
        "--output",
        type=Path,
        help="new directory for a live capture, AML, and build artifacts",
    )
    args = parser.parse_args(namespace=Arguments())
    if args.decode and args.output:
        parser.error("choose --decode or --output")
    try:
        if args.decode:
            records = decode(args.decode.read_text())
        else:
            output = (
                args.output.resolve()
                if args.output
                else Path(tempfile.mkdtemp(prefix="zentools-aod-parent-")) / "capture"
            )
            records = capture(output)
            print(f"Evidence: {output}\n")
        print(report(records), end="")
    except (TypeError, ValueError, OSError, subprocess.CalledProcessError) as exc:
        parser.exit(1, f"AOD probe: {exc}\n")


if __name__ == "__main__":
    main()
