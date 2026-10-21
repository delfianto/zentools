import re
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import override
from unittest.mock import patch

import probe


class AodCaptureTests(unittest.TestCase):
    text: str = ""

    @override
    def setUp(self) -> None:
        self.text = (Path(__file__).parent / "fixtures/ms7e49-1aa3.txt").read_text()

    def test_live_capture_agrees_with_independent_umc_readings(self) -> None:
        rows = {int(r["id"], 16): r for r in probe.decode(self.text)}
        self.assertEqual(len(rows), 81)
        # Independent UMC readout from the same boot: MCLK, CL, RCD, RP, RAS,
        # RFC1, RFC2, RFCsb, plus Nitro RX/TX/control.
        expected = {
            0x20001: 3000,
            0x20002: 30,
            0x2003B: 36,
            0x20006: 36,
            0x20005: 76,
            0x2003C: 884,
            0x2000B: 480,
            0x2003D: 390,
            0x2004D: 2,
            0x2004E: 3,
            0x2004F: 1,
        }
        for object_id, value in expected.items():
            self.assertEqual(rows[object_id]["raw"], value)
        self.assertEqual(rows[0x20051].get("decoded"), "40 ohm")
        self.assertEqual(rows[0x20052].get("decoded"), "43.6 ohm")
        self.assertEqual(rows[0x20053].get("decoded"), "48 ohm")
        self.assertEqual(rows[0x30001].get("decoded"), "1.350 V")

    def test_rejects_failed_truncated_and_mismatched_responses(self) -> None:
        for bad in [
            self.text.replace("6 00000000 20", "6 00000001 20", 1),
            self.text.replace("6 00000000 20", "6 00000000 4", 1),
            self.text.replace("20 00020059", "20 00020058", 1),
            self.text.replace("00020059 6", "00020059 5", 1),
            "\n".join(self.text.splitlines()[1:]),
            self.text + self.text.splitlines()[0] + "\n",
        ]:
            with self.subTest(bad=bad[:100]), self.assertRaises(ValueError):
                _ = probe.decode(bad)

    def test_compiled_allowlist_matches_audited_metadata(self) -> None:
        items = probe.load_objects()
        compiled = [
            (int(m[1], 16), int(m[2]))
            for m in re.finditer(
                r"\{\s*(0x[0-9a-f]+),\s*([0-9]+)\s*\}",
                (probe.HERE / "objects.h").read_text(),
            )
        ]
        self.assertEqual(compiled, [(item["id"], item["method"]) for item in items])
        self.assertTrue(all(method in (4, 6) for _, method in compiled))

    def test_unknown_electrical_codes_remain_unknown(self) -> None:
        self.assertEqual(probe.decoded_value(255, "proc_odt"), "unknown code 255")
        self.assertEqual(probe.decoded_value(255, "rtt"), "unknown code 255")

    def test_rejects_invalid_metadata_before_decoding(self) -> None:
        for content, error in [
            ("{}", TypeError),
            ("[3]", TypeError),
            ('[{"id": "invalid"}]', ValueError),
            (
                '[{"id": 1, "method": 5, "index": 0, "length": 20, "name": "x"}]',
                ValueError,
            ),
        ]:
            with self.subTest(content=content), TemporaryDirectory() as directory:
                root = Path(directory)
                _ = (root / "objects.json").write_text(content)
                with patch.object(probe, "HERE", root), self.assertRaises(error):
                    _ = probe.load_objects()


if __name__ == "__main__":
    _ = unittest.main()
