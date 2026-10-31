import json
import os
import subprocess
import sys
import unittest

from lib import ROOT, read

ROUTE = os.path.join(ROOT, "skills", "intelli-route", "bin", "route")
FORMAL = ("statechart", "create-formal-verification-skill", "maintain-formal-verification-skill",
          "create-verification-skill")


def route(*a):
    p = subprocess.run([sys.executable, ROUTE, *a], capture_output=True, text=True, cwd=ROOT)
    return p.returncode, p.stdout, p.stderr


class When(unittest.TestCase):
    def test_when_lookup_finds_plugin_pstack_map(self):
        code, out, err = route("otherwise, for non-trivial engineering")
        self.assertEqual(code, 0, err)
        self.assertEqual(json.loads(out)["route"], "light")


class FormalSignal(unittest.TestCase):
    def emit(self, task):
        code, out, err = route("--signals", json.dumps({"kind": "build", "task": task}))
        self.assertEqual(code, 0, err)
        return json.loads(out)

    def test_formal_task_loads_statechart_and_verification_skills(self):
        e = self.emit("Add a TLA+ spec and model-check the ledger invariant with TLC")
        self.assertEqual(e["signal"], "formal-verification")
        self.assertEqual(e["loads"], list(FORMAL))
        self.assertIn("principle-boundary-discipline", e["principles"])
        self.assertIn("principle-encode-lessons-in-structure", e["principles"])

    def test_each_trigger_word_matches(self):
        for t in ("formal verification of the parser", "draw the statechart", "prove an invariant", "model check it"):
            self.assertEqual(self.emit(t)["signal"], "formal-verification", t)

    def test_plain_task_emits_no_formal_loads(self):
        e = self.emit("rename a css class")
        self.assertNotIn("signal", e)
        self.assertNotIn("loads", e)

    def test_manifest_declares_loads_for_formal_skills(self):
        m = json.loads(read(os.path.join(ROOT, "skills", "intelli-route", "manifest.json")))
        ids = {l["id"] for l in m["loads"]}
        for s in FORMAL:
            self.assertIn(s, ids)
        self.assertLessEqual(len(m["signals"][0]["loads"]), m["max_loads"])


class Docs(unittest.TestCase):
    def test_skill_and_readme_describe_task_signal(self):
        for f in (os.path.join("skills", "intelli-route", "SKILL.md"), "README.md",
                  os.path.join("skills", "intelli-route", "README.md")):
            self.assertIn("formal-verification", read(os.path.join(ROOT, f)), f)

    def test_skill_readme_names_real_tests(self):
        s = read(os.path.join(ROOT, "skills", "intelli-route", "README.md")).split("## Tests", 1)[1]
        self.assertNotIn("test-thin.sh", s)
        self.assertIn("test/test_route.py", s)

    def test_statechart_handoff_documents_tla2tools(self):
        self.assertIn("TLA2TOOLS", read(os.path.join(ROOT, "docs", "formal.md")))
        self.assertIn("TLA2TOOLS", read(os.path.join(ROOT, "skills", "intelli-route", "SKILL.md")))


if __name__ == "__main__":
    unittest.main()
