import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

from lib import ROOT, read, write

ROUTE = os.path.join(ROOT, "skills", "intelli-route", "bin", "route")
FORMAL = ("statechart", "create-formal-verification-skill", "maintain-formal-verification-skill",
          "create-verification-skill")


def route(*a, script=ROUTE, cwd=ROOT):
    p = subprocess.run([sys.executable, script, *a], capture_output=True, text=True, cwd=cwd)
    return p.returncode, p.stdout, p.stderr


class When(unittest.TestCase):
    def test_when_lookup_finds_plugin_pstack_map(self):
        code, out, err = route("otherwise, for non-trivial engineering")
        self.assertEqual(code, 0, err)
        body = json.loads(out)
        self.assertEqual(body["route"], "light")
        self.assertEqual(body["playbook"], "house")
        self.assertEqual(body["loads"], ["pstack-map"])

    def test_map_playbook_name_is_a_when(self):
        code, out, err = route("bug-fix")
        self.assertEqual(code, 0, err)
        body = json.loads(out)
        self.assertEqual(body["route"], "light")
        self.assertEqual(body["playbook"], "bug-fix")
        self.assertEqual(body["loads"], ["pstack-map"])
        self.assertEqual(body["next"], "pstack-map")

    def test_longest_map_playbook_token_wins(self):
        code, out, err = route("run autopilot-full overnight")
        self.assertEqual(code, 0, err)
        self.assertEqual(json.loads(out)["playbook"], "autopilot-full")

    def test_unknown_situation_is_missing_when(self):
        code, out, err = route("zzzz-not-a-playbook")
        self.assertEqual(code, 1)
        self.assertIn("missing when", err)
        self.assertEqual(out, "")

    def test_plugin_skills_tree_reads_sibling_map(self):
        with tempfile.TemporaryDirectory() as t:
            skill = os.path.join(t, "skills", "intelli-route")
            os.makedirs(os.path.join(skill, "bin"))
            shutil.copy(ROUTE, os.path.join(skill, "bin", "route"))
            shutil.copy(os.path.join(ROOT, "skills", "intelli-route", "manifest.json"),
                        os.path.join(skill, "manifest.json"))
            ref = os.path.join(t, "skills", "pstack-map", "references")
            os.makedirs(ref)
            write(os.path.join(ref, "map.md"),
                  "| Playbook | House |\n|---|---|\n| bug-fix | reproduce |\n| feature | shape |\n")
            script = os.path.join(skill, "bin", "route")
            code, out, err = route("please feature", script=script, cwd=t)
            self.assertEqual(code, 0, err)
            body = json.loads(out)
            self.assertEqual(body["playbook"], "feature")
            self.assertEqual(body["route"], "light")


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

    def test_situation_text_matching_signal_emits_formal_loads(self):
        code, out, err = route("model-check the ledger")
        self.assertEqual(code, 0, err)
        e = json.loads(out)
        self.assertEqual(e["signal"], "formal-verification")
        self.assertEqual(e["loads"], list(FORMAL))
        self.assertEqual(e["route"], "light")
        self.assertEqual(e["next"], "statechart")

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
        self.assertIn("bug-fix", s)
        self.assertIn("references/verify.md", read(os.path.join(ROOT, "skills", "intelli-route", "SKILL.md")))

    def test_statechart_handoff_documents_tla2tools(self):
        self.assertIn("TLA2TOOLS", read(os.path.join(ROOT, "docs", "formal.md")))
        self.assertIn("TLA2TOOLS", read(os.path.join(ROOT, "skills", "intelli-route", "SKILL.md")))


if __name__ == "__main__":
    unittest.main()
