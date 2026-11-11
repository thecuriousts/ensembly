import json
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest

from lib import ROOT, write

CLEAN = {
    "FLOWINGSTACK_ROUTING": None,
    "FS_CHECKS_FILE": None,
    "FS_QUEUE_FILE": None,
    "FS_QUEUE_ROW": None,
    "FS_READY_MARKER": None,
    "FS_REVIEWER_REQUEST": None,
}


def hook(name, *args, stdin=None, cwd=None, env=None):
    base = {**os.environ, "FLOWINGSTACK_ROOT": ROOT, "PYTHONDONTWRITEBYTECODE": "1"}
    for k, v in {**CLEAN, **(env or {})}.items():
        if v is None:
            base.pop(k, None)
        else:
            base[k] = v
    p = subprocess.run([sys.executable, os.path.join(ROOT, "hooks", name), *args],
                       capture_output=True, text=True, input=stdin, cwd=cwd or ROOT, env=base)
    return p.returncode, p.stdout, p.stderr


def git_repo(path):
    subprocess.run(["git", "init"], cwd=path, check=True, capture_output=True)
    subprocess.run(["git", "-C", path, "config", "user.email", "dev@example.com"], check=True, capture_output=True)
    subprocess.run(["git", "-C", path, "config", "user.name", "dev"], check=True, capture_output=True)
    write(os.path.join(path, "a.txt"), "a\n")
    subprocess.run(["git", "-C", path, "-c", "commit.gpgsign=false", "add", "a.txt"], check=True, capture_output=True)
    subprocess.run(["git", "-C", path, "-c", "commit.gpgsign=false", "commit", "-m", "init"],
                   check=True, capture_output=True)


def stop(env, session="s1"):
    return hook("stop", stdin=json.dumps({"session_id": session, "hook_event_name": "Stop"}), env=env)


class FailBlock(unittest.TestCase):
    def test_named_check_fail_blocks_with_reason(self):
        with tempfile.TemporaryDirectory() as t:
            checks = os.path.join(t, "checks.tsv")
            write(checks, "name\tcommand\ntests\tpython3 -c \"import sys; sys.stderr.write('boom-tests'); sys.exit(1)\"\n")
            code, out, err = stop({
                "FS_HOOK_STATE_DIR": t,
                "FS_CHECKS_FILE": checks,
                "FS_REPO_ROOT": t,
                "FS_SESSION_ID": "s1",
            })
            self.assertEqual(code, 0, err)
            data = json.loads(out)
            self.assertEqual(data.get("decision"), "block")
            self.assertIn("tests", data.get("reason", ""))
            self.assertIn("boom-tests", data.get("reason", ""))
            self.assertEqual(data.get("followup_message"), data.get("reason"))
            self.assertNotIn("eva-emptiness", data.get("reason", ""))


class RetryBound(unittest.TestCase):
    def test_eight_continuations_then_allow_stop(self):
        with tempfile.TemporaryDirectory() as t:
            checks = os.path.join(t, "checks.tsv")
            write(checks, "name\tcommand\ntests\tpython3 -c \"raise SystemExit(1)\"\n")
            env = {
                "FS_HOOK_STATE_DIR": t,
                "FS_CHECKS_FILE": checks,
                "FS_REPO_ROOT": t,
                "FS_SESSION_ID": "bound",
            }
            for i in range(8):
                code, out, err = stop(env, session="bound")
                self.assertEqual(code, 0, err)
                data = json.loads(out)
                self.assertEqual(data.get("decision"), "block", f"call {i + 1}: {out}")
            code, out, err = stop(env, session="bound")
            self.assertEqual(code, 0, err)
            data = json.loads(out)
            self.assertNotIn("decision", data)
            self.assertFalse(data.get("followup_message"))
            state = json.loads(Path(t, "bound.json").read_text())
            self.assertEqual(state["result"], "failed-gate")
            self.assertEqual(state["continuations"], 9)


class PassMarker(unittest.TestCase):
    def test_pass_writes_marker_with_head_sha(self):
        with tempfile.TemporaryDirectory() as t:
            git_repo(t)
            sha = subprocess.check_output(["git", "-C", t, "rev-parse", "HEAD"], text=True).strip()
            checks = os.path.join(t, "checks.tsv")
            write(checks, "name\tcommand\nok\tpython3 -c \"raise SystemExit(0)\"\n")
            marker = os.path.join(t, "ready.json")
            reviewer = os.path.join(t, "review.txt")
            code, out, err = hook("done-gate", "--checks", checks, "--repo", t, "--session", "ok",
                                  "--state-dir", t, "--marker", marker, "--reviewer", reviewer)
            self.assertEqual(code, 0, err + out)
            data = json.loads(out)
            self.assertTrue(data["ok"])
            self.assertEqual(data["head"], sha)
            saved = json.loads(Path(marker).read_text())
            self.assertEqual(saved["head"], sha)
            self.assertEqual(saved["status"], "ready")
            self.assertIn(sha, Path(reviewer).read_text())
            self.assertIn("no-context reviewer", Path(reviewer).read_text())
            code, out, err = hook("done-gate", "--verify-marker", marker, "--repo", t)
            self.assertEqual(code, 0, err + out)
            self.assertTrue(json.loads(out)["ok"])

    def test_moved_head_invalidates_marker(self):
        with tempfile.TemporaryDirectory() as t:
            git_repo(t)
            checks = os.path.join(t, "checks.tsv")
            write(checks, "name\tcommand\nok\tpython3 -c \"raise SystemExit(0)\"\n")
            marker = os.path.join(t, "ready.json")
            code, out, err = hook("done-gate", "--checks", checks, "--repo", t, "--session", "ok",
                                  "--state-dir", t, "--marker", marker, "--reviewer", os.path.join(t, "review.txt"))
            self.assertEqual(code, 0, err + out)
            write(os.path.join(t, "b.txt"), "b\n")
            subprocess.run(["git", "-C", t, "-c", "commit.gpgsign=false", "add", "b.txt"], check=True, capture_output=True)
            subprocess.run(["git", "-C", t, "-c", "commit.gpgsign=false", "commit", "-m", "next"],
                           check=True, capture_output=True)
            code, out, err = hook("done-gate", "--verify-marker", marker, "--repo", t)
            self.assertEqual(code, 1, err + out)
            self.assertFalse(json.loads(out)["ok"])
            self.assertNotEqual(json.loads(out)["head"], json.loads(Path(marker).read_text())["head"])


class Unclassified(unittest.TestCase):
    def test_missing_config_routes_through_flowingstack(self):
        with tempfile.TemporaryDirectory() as t:
            env = {
                "FS_HOOK_STATE_DIR": t,
                "FS_CHECKS_FILE": os.path.join(t, "missing-checks.tsv"),
                "FS_REPO_ROOT": t,
                "FS_SESSION_ID": "u1",
                "FLOWINGSTACK_ROUTING": os.path.join(ROOT, "examples", "house-routing.tsv"),
            }
            code, out, err = stop(env, session="u1")
            self.assertEqual(code, 0, err)
            data = json.loads(out)
            self.assertEqual(data.get("decision"), "block")
            msg = data.get("reason", "")
            self.assertIn("missing config", msg)
            self.assertIn("Do not guess", msg)
            self.assertIn("flowingstack", msg)
            self.assertIn("eva-emptiness", msg)
            self.assertIn("odysseus-navigator", msg)
            self.assertEqual(data.get("followup_message"), msg)
