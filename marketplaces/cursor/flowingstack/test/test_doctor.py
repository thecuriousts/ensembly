import os
import re
import tempfile
import unittest

from lib import ROOT, read, run, write


def tree(d):
    out = {}
    for b, ds, fs in os.walk(d):
        for n in ds + fs:
            p = os.path.join(b, n)
            out[os.path.relpath(p, d)] = read(p) if os.path.isfile(p) else None
    return out


class Doctor(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.p = os.path.join(self.t.name, "proj")
        self.plug = os.path.join(self.t.name, "plugins")
        os.makedirs(self.p)
        os.makedirs(self.plug)

    def tearDown(self):
        self.t.cleanup()

    def doc(self, *a):
        return run("doctor", "--project", self.p, "--plugins-dir", self.plug, "--disk-percent", "33", *a)

    def full(self):
        for f, s in ((".cursor/rules/pstack-models.mdc", "m\n"), ("AGENTS.md", "hosts: laptop-1\n"),
                     (".github/workflows/ci.yml", "on: push\n"), (".cursor/hooks.json", "{}\n"),
                     (".cursor/skills/verify-app/SKILL.md", "v\n"), ("../plugins/pstack/SKILL.md", "p\n"),
                     ("../plugins/craft/SKILL.md", "c\n")):
            os.makedirs(os.path.dirname(os.path.join(self.p, f)), exist_ok=True)
            write(os.path.join(self.p, f), s)

    def test_empty_project_lists_every_missing_item(self):
        code, out = self.doc()
        self.assertEqual(code, 1)
        for k in ("model-rule", "agents-md", "ci", "verify-skill", "hooks", "aliases", "pstack", "craft"):
            self.assertRegex(out, rf"(?m)^missing {k}\b")

    def test_full_project_ok(self):
        self.full()
        code, out = self.doc()
        self.assertEqual(code, 0, out)
        self.assertNotIn("missing", out)

    def test_house_verify_skill_nested_or_verification_name_found(self):
        for f in (".agents/skills/ledger-verification/SKILL.md", ".cursor/skills/house/verify-ledger/SKILL.md"):
            with self.subTest(f=f):
                q = os.path.join(self.t.name, f.split("/")[1] + "-p")
                os.makedirs(os.path.dirname(os.path.join(q, f)), exist_ok=True)
                write(os.path.join(q, f), "v\n")
                out = run("doctor", "--project", q, "--plugins-dir", self.plug, "--disk-percent", "33")[1]
                self.assertRegex(out, r"(?m)^ok verify-skill$")

    def test_nested_plugin_cache_layout_found(self):
        d = os.path.join(self.plug, "pstack", "1ba91fc", "skills", "setup-pstack")
        os.makedirs(d)
        write(os.path.join(d, "SKILL.md"), "s\n")
        self.assertRegex(self.doc()[1], r"(?m)^ok pstack$")

    def test_cursor_cache_layout_found(self):
        a = os.path.join(self.plug, "acme-market", "id-7f3a", "0c1d2e3")
        os.makedirs(os.path.join(a, ".cursor-plugin"))
        write(os.path.join(a, ".cursor-plugin", "plugin.json"), '{"name": "pstack"}\n')
        b = os.path.join(self.plug, "acme-market", "id-9b2c", "4f5a6b7", "skills", "craft")
        os.makedirs(b)
        write(os.path.join(b, "SKILL.md"), "c\n")
        out = self.doc()[1]
        self.assertRegex(out, r"(?m)^ok pstack$")
        self.assertRegex(out, r"(?m)^ok craft$")

    def test_other_plugin_names_not_matched(self):
        a = os.path.join(self.plug, "acme-market", "id-1", "abc")
        os.makedirs(os.path.join(a, "skills", "craftsman"))
        write(os.path.join(a, "plugin.json"), '{"name": "pstack-lite"}\n')
        write(os.path.join(a, "skills", "craftsman", "SKILL.md"), "x\n")
        out = self.doc()[1]
        self.assertRegex(out, r"(?m)^missing pstack\b")
        self.assertRegex(out, r"(?m)^missing craft\b")

    def test_disk_at_70_fails(self):
        self.full()
        code, out = run("doctor", "--project", self.p, "--plugins-dir", self.plug, "--disk-percent", "70")
        self.assertEqual(code, 1)
        self.assertIn("fail disk", out)

    def test_ci_billing_failure_flagged(self):
        self.full()
        write(os.path.join(self.t.name, "ci.txt"), "conclusion: failure\nreason: billing\n")
        code, out = self.doc("--ci-status", os.path.join(self.t.name, "ci.txt"))
        self.assertEqual(code, 1)
        self.assertIn("fail ci-health", out)

    def test_init_never_writes(self):
        before = tree(self.t.name)
        code, out = self.doc("--init")
        self.assertEqual(tree(self.t.name), before)
        self.assertIn("/setup-pstack", out)
        self.assertFalse(os.path.exists(os.path.join(self.p, ".cursor", "rules", "pstack-models.mdc")))

    def test_init_on_full_project_never_writes(self):
        self.full()
        before = tree(self.t.name)
        self.doc("--init")
        self.assertEqual(tree(self.t.name), before)

    def test_source_has_no_write_path(self):
        src = read(os.path.join(ROOT, "scripts", "doctor"))
        self.assertIsNone(re.search(r"open\([^)]*['\"][wax]", src))
        for w in ("write(", "makedirs", "mkdir", "copy", "rename", "symlink", "subprocess"):
            self.assertNotIn(w, src)


class Skill(unittest.TestCase):
    def test_skill_layout(self):
        s = read(os.path.join(ROOT, "skills", "doctor", "SKILL.md"))
        for h in ("name: doctor", "owner:", "source:", "last-reviewed:", "## Flow", "## Contract", "## Checks", "## Notes", "/setup-pstack"):
            self.assertIn(h, s)


if __name__ == "__main__":
    unittest.main()
