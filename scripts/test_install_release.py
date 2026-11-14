#!/usr/bin/env python3
"""Exercise install.sh release download + SHA256SUMS verification against a local HTTP server."""
from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import tarfile
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
INSTALL = REPO / "scripts" / "install.sh"
TAG = "ensembly-cli--v9.9.9"
VER = "9.9.9"
TARGET = "x86_64-unknown-linux-gnu"
ASSET = f"ensembly-{VER}-{TARGET}.tar.gz"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build_tarball_bytes(body: bytes) -> bytes:
    with tempfile.TemporaryDirectory() as t:
        root = Path(t)
        inner = root / f"ensembly-{VER}-{TARGET}"
        inner.mkdir()
        binary = inner / "ensembly"
        binary.write_bytes(body)
        binary.chmod(0o755)
        arc = root / ASSET
        with tarfile.open(arc, "w:gz") as tar:
            tar.add(inner, arcname=inner.name)
        return arc.read_bytes()


class ReleaseHandler(BaseHTTPRequestHandler):
    asset_body: bytes = b""
    sums_body: bytes | None = b""

    def log_message(self, _format, *_args):
        return

    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path == "/releases/latest":
            self.send_response(302)
            self.send_header("Location", f"/releases/tag/{TAG}")
            self.end_headers()
            return
        if path == f"/releases/tag/{TAG}":
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"release")
            return
        if path == f"/releases/download/{TAG}/{ASSET}":
            self.send_response(200)
            self.end_headers()
            self.wfile.write(ReleaseHandler.asset_body)
            return
        if path == f"/releases/download/{TAG}/SHA256SUMS":
            if ReleaseHandler.sums_body is None:
                self.send_response(404)
                self.end_headers()
                return
            self.send_response(200)
            self.end_headers()
            self.wfile.write(ReleaseHandler.sums_body)
            return
        self.send_response(404)
        self.end_headers()


class InstallRelease(unittest.TestCase):
    def invoke(self, asset: bytes, sums: bytes | None) -> subprocess.CompletedProcess[str]:
        ReleaseHandler.asset_body = asset
        ReleaseHandler.sums_body = sums
        server = ThreadingHTTPServer(("127.0.0.1", 0), ReleaseHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        port = server.server_address[1]
        root = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: shutil.rmtree(root, ignore_errors=True))
        try:
            prefix = root / "prefix"
            env = os.environ.copy()
            env["HOME"] = str(root / "home")
            Path(env["HOME"]).mkdir()
            env["ENSEMBLY_RELEASE_REPO"] = f"http://127.0.0.1:{port}"
            proc = subprocess.run(
                ["sh", str(INSTALL), "--harness", "cli", "--prefix", str(prefix)],
                cwd=REPO,
                env=env,
                capture_output=True,
                text=True,
            )
            proc.prefix = prefix  # type: ignore[attr-defined]
            return proc
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_good_sum_installs(self):
        body = b"#!/bin/sh\necho fake-ensembly\n"
        asset = build_tarball_bytes(body)
        sums = f"{sha256_bytes(asset)}  {ASSET}\n".encode()
        proc = self.invoke(asset, sums)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        installed = proc.prefix / "bin" / "ensembly"  # type: ignore[attr-defined]
        self.assertTrue(installed.is_file())
        self.assertGreater(installed.stat().st_size, 0)
        self.assertIn("release asset", proc.stdout)

    def test_tampered_asset_refuses(self):
        good = build_tarball_bytes(b"#!/bin/sh\necho good\n")
        bad = build_tarball_bytes(b"#!/bin/sh\necho bad\n")
        sums = f"{sha256_bytes(good)}  {ASSET}\n".encode()
        proc = self.invoke(bad, sums)
        self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
        combined = proc.stderr + proc.stdout
        self.assertTrue("FAILED" in combined or "checksum" in combined.lower())

    def test_missing_sums_refuses(self):
        asset = build_tarball_bytes(b"#!/bin/sh\necho ok\n")
        proc = self.invoke(asset, None)
        self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
        self.assertIn("SHA256SUMS", proc.stderr + proc.stdout)


if __name__ == "__main__":
    unittest.main()