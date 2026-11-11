import hashlib
import json
from pathlib import Path
import os
import stat
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
    "FS_HOOK_STATE_DIR": None,
    "FS_SESSION_ID": None,
}


def hook(stdin, env=None):
    base = {**os.environ, "FLOWINGSTACK_ROOT": ROOT, "PYTHONDONTWRITEBYTECODE": "1"}
    for k, v in {**CLEAN, **(env or {})}.items():
        if v is None:
            base.pop(k, None)
        else:
            base[k] = v
    p = subprocess.run([sys.executable, os.path.join(ROOT, "hooks", "repeat-guard")],
                       capture_output=True, text=True, input=stdin, cwd=ROOT, env=base)
    return p.returncode, p.stdout, p.stderr


def call(env, tool="Shell", payload=None, session="g1"):
    body = {
        "session_id": session,
        "hook_event_name": "PreToolUse",
        "tool_name": tool,
        "tool_input": payload if payload is not None else {"command": "ls"},
    }
    code, out, err = hook(json.dumps(body), env)
    return code, json.loads(out), err


def no_allow(data):
    return "permission" not in data and (data.get("hookSpecificOutput") or {}).get("permissionDecision") not in ("allow", "deny")


class Repeat(unittest.TestCase):
    def test_nudge_at_three_deny_at_five(self):
        with tempfile.TemporaryDirectory() as t:
            env = {"FS_HOOK_STATE_DIR": t, "FS_SESSION_ID": "g1"}
            for i in range(1, 3):
                code, data, err = call(env)
                self.assertEqual(code, 0, err)
                self.assertEqual(data, {})
                self.assertTrue(no_allow(data))
            code, data, err = call(env)
            self.assertEqual(code, 0, err)
            self.assertTrue(no_allow(data))
            self.assertNotIn("permission", data)
            self.assertIsNone((data.get("hookSpecificOutput") or {}).get("permissionDecision"))
            self.assertIn("Repeat guard", data.get("agent_message", ""))
            self.assertIn("Repeat guard", (data.get("hookSpecificOutput") or {}).get("additionalContext", ""))
            code, data, err = call(env)
            self.assertEqual(code, 0, err)
            self.assertTrue(no_allow(data))
            code, data, err = call(env)
            self.assertEqual(code, 0, err)
            self.assertEqual(data.get("permission"), "deny")
            self.assertEqual((data.get("hookSpecificOutput") or {}).get("permissionDecision"), "deny")
            self.assertIn("5 identical consecutive calls", data.get("reason", ""))

    def test_first_call_returns_no_allow_decision(self):
        with tempfile.TemporaryDirectory() as t:
            code, data, err = call({"FS_HOOK_STATE_DIR": t, "FS_SESSION_ID": "first"})
            self.assertEqual(code, 0, err)
            self.assertEqual(data, {})
            self.assertNotIn("permission", data)
            self.assertIsNone((data.get("hookSpecificOutput") or {}).get("permissionDecision"))

    def test_different_call_resets_count(self):
        with tempfile.TemporaryDirectory() as t:
            env = {"FS_HOOK_STATE_DIR": t, "FS_SESSION_ID": "g2"}
            for _ in range(2):
                call(env, payload={"command": "ls"})
            code, data, err = call(env, payload={"command": "pwd"})
            self.assertEqual(code, 0, err)
            self.assertEqual(data, {})
            self.assertNotIn("Repeat guard", data.get("agent_message") or "")
            call(env, payload={"command": "pwd"})
            code, data, err = call(env, payload={"command": "pwd"})
            self.assertEqual(code, 0, err)
            self.assertTrue(no_allow(data))
            self.assertIn("Repeat guard", data.get("agent_message", ""))

    def test_deny_blocks_queue_row_in_temp_file(self):
        with tempfile.TemporaryDirectory() as t:
            queue = os.path.join(t, "desk.jsonl")
            write(queue, json.dumps({
                "id": "row-1",
                "from": "sender",
                "to": "desk",
                "ask": "work",
                "due": "",
                "urgent": False,
                "state": "taken",
                "result": "",
                "updated": "",
            }) + "\n")
            env = {
                "FS_HOOK_STATE_DIR": t,
                "FS_SESSION_ID": "g3",
                "FS_QUEUE_FILE": queue,
                "FS_QUEUE_ROW": "row-1",
            }
            for _ in range(4):
                call(env)
            code, data, err = call(env)
            self.assertEqual(code, 0, err)
            self.assertEqual(data.get("permission"), "deny")
            row = json.loads(Path(queue).read_text().splitlines()[0])
            self.assertEqual(row["id"], "row-1")
            self.assertEqual(row["state"], "blocked")
            self.assertIn("identical consecutive calls", row["result"])
            self.assertTrue(row["updated"])

    def test_deny_skips_null_queue_line_and_keeps_neighbor(self):
        with tempfile.TemporaryDirectory() as t:
            queue = os.path.join(t, "desk.jsonl")
            neighbor = {
                "id": "row-other",
                "from": "sender",
                "to": "desk",
                "ask": "other",
                "due": "",
                "urgent": False,
                "state": "queued",
                "result": "keep-me",
                "updated": "old",
            }
            target = {
                "id": "row-1",
                "from": "sender",
                "to": "desk",
                "ask": "work",
                "due": "",
                "urgent": False,
                "state": "taken",
                "result": "",
                "updated": "",
            }
            write(queue, json.dumps(neighbor) + "\nnull\n" + json.dumps(target) + "\n")
            env = {
                "FS_HOOK_STATE_DIR": t,
                "FS_SESSION_ID": "g4",
                "FS_QUEUE_FILE": queue,
                "FS_QUEUE_ROW": "row-1",
            }
            for _ in range(4):
                call(env)
            code, data, err = call(env)
            self.assertEqual(code, 0, err)
            self.assertEqual(data.get("permission"), "deny")
            lines = Path(queue).read_text().splitlines()
            self.assertEqual(len(lines), 3)
            self.assertEqual(json.loads(lines[0]), neighbor)
            self.assertEqual(lines[1], "null")
            row = json.loads(lines[2])
            self.assertEqual(row["id"], "row-1")
            self.assertEqual(row["state"], "blocked")
            self.assertIn("identical consecutive calls", row["result"])

    def test_session_id_cannot_escape_state_dir(self):
        with tempfile.TemporaryDirectory() as t:
            env = {"FS_HOOK_STATE_DIR": t}
            for raw in ("../escape", "/tmp/abs-session"):
                code, data, err = call(env, session=raw)
                self.assertEqual(code, 0, err)
                self.assertEqual(data, {})
                digest = hashlib.sha256(raw.encode()).hexdigest()
                inside = Path(t, f"{digest}.repeat.json")
                self.assertTrue(inside.is_file(), raw)
                self.assertFalse(Path(t, raw).exists())
            self.assertFalse((Path(t).parent / "escape.repeat.json").exists())
            self.assertFalse(Path("/tmp/abs-session.repeat.json").exists())

    def test_state_dir_uses_xdg_then_home_cache(self):
        with tempfile.TemporaryDirectory() as t:
            xdg = os.path.join(t, "xdg")
            home = os.path.join(t, "home")
            os.makedirs(home)
            code, data, err = call({
                "XDG_STATE_HOME": xdg,
                "HOME": home,
                "FS_SESSION_ID": "sxd",
            }, session="sxd")
            self.assertEqual(code, 0, err)
            dest = Path(xdg, "flowingstack-hooks")
            self.assertTrue(dest.is_dir())
            self.assertEqual(stat.S_IMODE(dest.stat().st_mode), 0o700)
            repeat = dest / "sxd.repeat.json"
            self.assertTrue(repeat.is_file())
            self.assertEqual(stat.S_IMODE(repeat.stat().st_mode), 0o600)
            self.assertFalse((Path(home) / ".cache" / "flowingstack-hooks").exists())
            code, data, err = call({
                "XDG_STATE_HOME": None,
                "HOME": home,
                "FS_SESSION_ID": "scache",
            }, session="scache")
            self.assertEqual(code, 0, err)
            cached = Path(home, ".cache", "flowingstack-hooks")
            self.assertTrue(cached.is_dir())
            self.assertEqual(stat.S_IMODE(cached.stat().st_mode), 0o700)
            self.assertTrue((cached / "scache.repeat.json").is_file())
            self.assertEqual(stat.S_IMODE((cached / "scache.repeat.json").stat().st_mode), 0o600)

    def test_deny_at_clamped_to_max(self):
        with tempfile.TemporaryDirectory() as t:
            env = {"FS_HOOK_STATE_DIR": t, "FS_SESSION_ID": "clamp", "FS_DENY_AT": "100"}
            for i in range(19):
                code, data, err = call(env, session="clamp")
                self.assertEqual(code, 0, err)
                self.assertNotEqual(data.get("permission"), "deny", i + 1)
            code, data, err = call(env, session="clamp")
            self.assertEqual(code, 0, err)
            self.assertEqual(data.get("permission"), "deny")
            self.assertIn("20 identical consecutive calls", data.get("reason", ""))
