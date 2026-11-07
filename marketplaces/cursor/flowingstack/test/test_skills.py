import os
import re
import unittest

from lib import ROOT, read

SKILLS = os.path.join(ROOT, "skills")
HUB = "flowingstack"


def front(name):
    s = read(os.path.join(SKILLS, name, "SKILL.md"))
    m = re.match(r"---\n(.*?)\n---\n", s, re.S)
    return dict(re.findall(r"^(\w[\w-]*):\s*(.*)$", m.group(1), re.M)) if m else {}


class Skills(unittest.TestCase):
    def test_every_skill_has_name_and_description(self):
        for d in sorted(os.listdir(SKILLS)):
            with self.subTest(d):
                f = front(d)
                self.assertEqual(f.get("name"), d)
                self.assertTrue(f.get("description"))

    def test_hub_routes_to_every_skill(self):
        s = read(os.path.join(SKILLS, HUB, "SKILL.md"))
        for d in sorted(set(os.listdir(SKILLS)) - {HUB}):
            with self.subTest(d):
                self.assertIn(f"`{d}`", s)

    def test_hub_default_path_is_the_kernel(self):
        s = read(os.path.join(SKILLS, HUB, "SKILL.md"))
        for w in ("task-contract", "grill-rubric", "run-ledger", "eval-freeze", "doctor", "intelli-route", "route-rule"):
            self.assertIn(f"`{w}`", s)
        self.assertIn("references/verify.md", s)
        for role in ("engineering", "review", "docs"):
            self.assertIn(role, s)
        for banned in ("Fleet Desk", "Chief of Staff", "loads craft then ub-mode"):
            self.assertNotIn(banned, s)
        self.assertIn("craft", s)
        self.assertIn("ub-mode", s)
        self.assertIn("when installed", s)
        readme = read(os.path.join(ROOT, "README.md"))
        self.assertNotIn("Fleet Desk", readme)
        self.assertNotIn("Chief of Staff", readme)
        self.assertIn("examples/house-routing.tsv", readme)
        self.assertIn("FLOWINGSTACK_ROUTING", readme)
        self.assertIn("FLOWINGSTACK_DOCTOR", readme)
        for w in ("capture-sweep", "capture-mail", "capture-calendar", "capture-chat", "capture-pr"):
            self.assertIn(w, readme)
            self.assertIn(f"`{w}`", s)
        rule = read(os.path.join(SKILLS, "route-rule", "SKILL.md"))
        task = read(os.path.join(SKILLS, "task-contract", "SKILL.md"))
        self.assertNotIn("Chief of Staff", rule)
        self.assertNotIn("Fleet Desk", rule)
        self.assertIn("FLOWINGSTACK_ROUTING", rule)
        self.assertIn(".flowingstack/routing.tsv", rule)
        self.assertIn("examples/house-routing.tsv", rule)
        self.assertIn("references/verify.md", task)
        self.assertNotIn("loads craft then ub-mode", task)
        self.assertIn("when installed", task)
        for w in ("capture-mail", "capture-calendar", "capture-chat", "capture-pr"):
            self.assertNotIn(w, rule)


if __name__ == "__main__":
    unittest.main()
