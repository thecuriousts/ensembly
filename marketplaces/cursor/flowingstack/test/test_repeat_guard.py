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


class Repeat(unittest.TestCase):
    def test_nudge_at_three_deny_at_five(self):
        with tempfile.TemporaryDirectory() as t:
            env = {"FS_HOOK_STATE_DIR": t, "FS_SESSION_ID": "g1"}
            for i in range(1, 3):
                code, data, err = call(env)
                self.assertEqual(code, 0, err)
                self.assertEqual(data.get("permission"), "allow")
                ctx = (data.get("hookSpecificOutput") or {}).get("additionalContext", "")
                self.assertNotIn("Repeat guard", ctx)
            code, data, err = call(env)
            self.assertEqual(code, 0, err)
            self.assertEqual(data.get("permission"), "allow")
            self.assertIn("Repeat guard", data.get("agent_message", ""))
            self.assertIn("Repeat guard", (data.get("hookSpecificOutput") or {}).get("additionalContext", ""))
            code, data, err = call(env)
            self.assertEqual(code, 0, err)
            self.assertEqual(data.get("permission"), "allow")
            code, data, err = call(env)
            self.assertEqual(code, 0, err)
            self.assertEqual(data.get("permission"), "deny")
            self.assertEqual((data.get("hookSpecificOutput") or {}).get("permissionDecision"), "deny")
            self.assertIn("5 identical consecutive calls", data.get("reason", ""))

    def test_different_call_resets_count(self):
        with tempfile.TemporaryDirectory() as t:
            env = {"FS_HOOK_STATE_DIR": t, "FS_SESSION_ID": "g2"}
            for _ in range(2):
                call(env, payload={"command": "ls"})
            code, data, err = call(env, payload={"command": "pwd"})
            self.assertEqual(code, 0, err)
            self.assertEqual(data.get("permission"), "allow")
            self.assertNotIn("Repeat guard", data.get("agent_message") or "")
            call(env, payload={"command": "pwd"})
            code, data, err = call(env, payload={"command": "pwd"})
            self.assertEqual(code, 0, err)
            self.assertEqual(data.get("permission"), "allow")
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
