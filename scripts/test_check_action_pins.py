#!/usr/bin/env python3
import importlib.util
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
import importlib.machinery

_loader = importlib.machinery.SourceFileLoader(
    "check_action_pins", str(HERE / "check-action-pins")
)
_spec = importlib.util.spec_from_loader("check_action_pins", _loader)
_mod = importlib.util.module_from_spec(_spec)
_loader.exec_module(_mod)
USE_RE = _mod.USE_RE
collect_pins = _mod.collect_pins


class CheckActionPins(unittest.TestCase):
    def test_use_regex(self):
        sample = """
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
"""
        matches = list(USE_RE.finditer(sample))
        self.assertEqual(len(matches), 2)
        self.assertEqual(matches[0].group(1), "actions/checkout")
        self.assertEqual(matches[1].group(1), "actions/upload-artifact")

    def test_collect_pins_sorted(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            wf = root / ".github" / "workflows"
            wf.mkdir(parents=True)
            (wf / "a.yml").write_text(
                "- uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1\n",
                encoding="utf-8",
            )
            (wf / "z.yml").write_text(
                "- uses: dtolnay/rust-toolchain@d124f86f28ff42855784ff3319f48ac182c248f8\n",
                encoding="utf-8",
            )
            pins = collect_pins(wf, root)
            self.assertEqual(len(pins), 2)
            self.assertTrue(pins[0][0].endswith("a.yml"))


if __name__ == "__main__":
    unittest.main()