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

    def test_hub_names_engineering_and_desk_defaults(self):
        s = read(os.path.join(SKILLS, HUB, "SKILL.md"))
        for w in ("craft", "ub-mode", "Fleet Desk", "CRAP max 6", "proof"):
            self.assertIn(w, s)
        readme = read(os.path.join(ROOT, "README.md"))
        self.assertIn("Fleet Desk", readme)
        for w in ("capture-sweep", "capture-mail", "capture-calendar", "capture-chat", "capture-pr"):
            self.assertIn(w, readme)
        rule = read(os.path.join(SKILLS, "route-rule", "SKILL.md"))
        tomato = read(os.path.join(SKILLS, "tomato-loop", "SKILL.md"))
        task = read(os.path.join(SKILLS, "task-contract", "SKILL.md"))
        for w in ("craft", "ub-mode"):
            self.assertIn(w, tomato)
            self.assertIn(w, task)
        for w in ("capture-mail", "capture-calendar", "capture-chat", "capture-pr"):
            self.assertIn(w, rule)
            self.assertIn(w, tomato)


if __name__ == "__main__":
    unittest.main()
