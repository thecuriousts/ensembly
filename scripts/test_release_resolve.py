#!/usr/bin/env python3
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
RESOLVE = os.path.join(HERE, "release-resolve")


class ReleaseResolve(unittest.TestCase):
    def run_resolve(self, tag: str, repo: str | None = None) -> subprocess.CompletedProcess[str]:
        repo = repo or REPO
        return subprocess.run(
            [sys.executable, RESOLVE, tag, "--repo", repo],
            cwd=repo,
            capture_output=True,
            text=True,
        )

    def test_good_tag_passes(self):
        ver = subprocess.check_output(
            [
                sys.executable,
                "-c",
                "import tomllib; print(tomllib.load(open('crates/ensembly-kernel/Cargo.toml','rb'))['package']['version'])",
            ],
            cwd=REPO,
            text=True,
        ).strip()
        tag = f"ensembly-cli--v{ver}"
        proc = self.run_resolve(tag)
        self.assertEqual(proc.returncode, 0, proc.stderr + proc.stdout)
        self.assertIn(f"tag={tag}", proc.stdout)
        self.assertIn(f"version={ver}", proc.stdout)

    def test_flowingstack_tag_fails(self):
        proc = self.run_resolve("flowingstack--v0.1.0")
        self.assertNotEqual(proc.returncode, 0, proc.stdout)
        self.assertIn("ensembly-cli", proc.stderr + proc.stdout)

    def test_github_output_writes_file(self):
        ver = subprocess.check_output(
            [
                sys.executable,
                "-c",
                "import tomllib; print(tomllib.load(open('crates/ensembly-kernel/Cargo.toml','rb'))['package']['version'])",
            ],
            cwd=REPO,
            text=True,
        ).strip()
        tag = f"ensembly-cli--v{ver}"
        with tempfile.NamedTemporaryFile("w", delete=False) as handle:
            out_path = handle.name
        self.addCleanup(os.unlink, out_path)
        env = os.environ.copy()
        env["GITHUB_OUTPUT"] = out_path
        proc = subprocess.run(
            [sys.executable, RESOLVE, tag, "--github-output"],
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
        )
        self.assertEqual(proc.returncode, 0, proc.stderr + proc.stdout)
        with open(out_path, encoding="utf-8") as handle:
            body = handle.read()
        self.assertIn(f"tag={tag}\n", body)
        self.assertIn(f"version={ver}\n", body)

    def test_version_mismatch_fails(self):
        ver = subprocess.check_output(
            [
                sys.executable,
                "-c",
                "import tomllib; print(tomllib.load(open('crates/ensembly-kernel/Cargo.toml','rb'))['package']['version'])",
            ],
            cwd=REPO,
            text=True,
        ).strip()
        parts = ver.split(".")
        bumped = f"{parts[0]}.{parts[1]}.{int(parts[2]) + 1}"
        proc = self.run_resolve(f"ensembly-cli--v{bumped}")
        self.assertEqual(proc.returncode, 1, proc.stderr + proc.stdout)
        self.assertIn("!=", proc.stderr)


if __name__ == "__main__":
    unittest.main()