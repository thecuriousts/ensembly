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


if __name__ == "__main__":
    unittest.main()
