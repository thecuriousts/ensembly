#!/usr/bin/env python3
import hashlib
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / "scripts/verify-release-checksums"


class VerifyReleaseChecksums(unittest.TestCase):
    def test_round_trip(self):
        with tempfile.TemporaryDirectory() as t:
            root = Path(t)
            payload = root / "ensembly-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
            payload.write_bytes(b"demo")
            digest = hashlib.sha256(payload.read_bytes()).hexdigest()
            (root / "SHA256SUMS").write_text(f"{digest}  {payload.name}\n")
            out = subprocess.run([str(SCRIPT), str(root)], capture_output=True, text=True)
            self.assertEqual(out.returncode, 0, out.stderr + out.stdout)


if __name__ == "__main__":
    unittest.main()