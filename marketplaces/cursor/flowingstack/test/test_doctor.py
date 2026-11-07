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

    def doc(self, *a, env=None):
        e = {"FLOWINGSTACK_DOCTOR": None}
        if env:
            e.update(env)
        return run("doctor", "--project", self.p, "--plugins-dir", self.plug, "--disk-percent", "33", *a, env=e)

    def kernel(self):
        for f, s in (("AGENTS.md", "operator notes\n"), (".github/workflows/ci.yml", "on: push\n"),
                     (".cursor/hooks.json", "{}\n"), (".cursor/skills/verify-app/SKILL.md", "v\n")):
            os.makedirs(os.path.dirname(os.path.join(self.p, f)), exist_ok=True)
            write(os.path.join(self.p, f), s)

    def user_checks(self, body):
        os.makedirs(os.path.join(self.p, ".flowingstack"), exist_ok=True)
        write(os.path.join(self.p, ".flowingstack", "doctor.tsv"),
              "name\trequired\tkind\targ\tfix\n" + body)

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
        for k in ("agents-md", "ci", "verify-skill", "hooks"):
            self.assertRegex(out, rf"(?m)^missing {k}\b")
        for k in ("model-rule", "aliases", "pstack", "craft"):
            self.assertRegex(out, rf"(?m)^optional {k}\b")
            self.assertNotRegex(out, rf"(?m)^missing {k}\b")

    def test_plugin_only_project_does_not_require_house_tools(self):
        self.kernel()
        code, out = self.doc()
        self.assertEqual(code, 0, out)
        self.assertNotRegex(out, r"(?m)^missing ")
        self.assertNotRegex(out, r"(?m)^fail ")
        for k in ("model-rule", "aliases", "pstack", "craft"):
            self.assertRegex(out, rf"(?m)^optional {k}\b")

    def test_missing_extension_file_is_fine(self):
        self.kernel()
        code, out = self.doc(env={"FLOWINGSTACK_DOCTOR": os.path.join(self.t.name, "no-such-doctor.tsv")})
        self.assertEqual(code, 0, out)
        self.assertRegex(out, r"(?m)^optional aliases\b")

    def test_added_check_is_visible(self):
        self.kernel()
        self.user_checks("team-notes\trequired\tfile\tdocs/team.md\tadd docs/team.md\n")
        code, out = self.doc()
        self.assertEqual(code, 1, out)
        self.assertRegex(out, r"(?m)^missing team-notes\b")
        self.assertRegex(out, r"(?m)^ok agents-md$")
        self.assertRegex(out, r"(?m)^optional pstack\b")

    def test_tweak_wins_over_shipped_check(self):
        self.kernel()
        self.user_checks("aliases\trequired\tregex\t\\blaptop-\\d\\b|\\bmac-mini\\b\tname host aliases\n")
        code, out = self.doc()
        self.assertEqual(code, 1, out)
        self.assertRegex(out, r"(?m)^missing aliases\b")
        self.assertRegex(out, r"(?m)^ok agents-md$")
        self.assertRegex(out, r"(?m)^optional pstack\b")
        self.assertRegex(out, r"(?m)^optional craft\b")

    def test_tweak_disk_threshold_wins(self):
        self.full()
        self.user_checks("disk\trequired\tdisk\t90\tfree disk\n")
        code, out = run("doctor", "--project", self.p, "--plugins-dir", self.plug, "--disk-percent", "70",
                        env={"FLOWINGSTACK_DOCTOR": None})
        self.assertEqual(code, 0, out)
        self.assertIn("ok disk 70%", out)

    def test_env_file_beats_local_file(self):
        self.kernel()
        self.user_checks("aliases\trequired\tregex\t\\bnever\\b\tname host aliases\n")
        env_file = os.path.join(self.t.name, "env-doctor.tsv")
        write(env_file, "name\trequired\tkind\targ\tfix\nteam-notes\trequired\tfile\tAGENTS.md\tadd notes\n")
        code, out = self.doc(env={"FLOWINGSTACK_DOCTOR": env_file})
        self.assertEqual(code, 0, out)
        self.assertRegex(out, r"(?m)^ok team-notes$")
        self.assertRegex(out, r"(?m)^optional aliases\b")

    def test_house_doctor_example_when_selected(self):
        self.kernel()
        code, out = self.doc(env={"FLOWINGSTACK_DOCTOR": os.path.join(ROOT, "examples", "house-doctor.tsv")})
        self.assertEqual(code, 1, out)
        self.assertRegex(out, r"(?m)^missing aliases\b")
        self.assertRegex(out, r"(?m)^missing team-notes\b")
        self.assertRegex(out, r"(?m)^ok ci$")
        self.assertRegex(out, r"(?m)^optional pstack\b")

    def test_bad_user_check_exits_2(self):
        self.kernel()
        self.user_checks("\trequired\tfile\tx\tfix\n")
        code, out = self.doc()
        self.assertEqual(code, 2, out)
        self.assertIn("missing name", out)

    def test_alias_regex_reads_host_alias_doc(self):
        self.kernel()
        os.makedirs(os.path.join(self.p, "docs"))
        write(os.path.join(self.p, "docs", "host-aliases.md"), "box mac-mini\n")
        self.user_checks("aliases\trequired\tregex\t\\blaptop-\\d\\b|\\bmac-mini\\b\tname host aliases\n")
        code, out = self.doc()
        self.assertEqual(code, 0, out)
        self.assertRegex(out, r"(?m)^ok aliases$")

    def test_shipped_optional_rows(self):
        rows = {}
        for line in read(os.path.join(ROOT, "doctor.tsv")).splitlines()[1:]:
            c = line.split("\t")
            rows[c[0]] = c[1]
        for name in ("model-rule", "aliases", "pstack", "craft"):
            self.assertEqual(rows[name], "optional", name)
        for name in ("agents-md", "ci", "verify-skill", "hooks", "disk"):
            self.assertEqual(rows[name], "required", name)

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

    def test_grill_rubric_or_verify_pack_counts(self):
        for rel in ("skills/grill-rubric/SKILL.md", "references/verify.md"):
            with self.subTest(rel):
                q = os.path.join(self.t.name, rel.split("/")[0] + "-v")
                os.makedirs(os.path.dirname(os.path.join(q, rel)), exist_ok=True)
                write(os.path.join(q, rel), "v\n")
                out = run("doctor", "--project", q, "--plugins-dir", os.path.join(self.t.name, "none"),
                          "--disk-percent", "33")[1]
                self.assertRegex(out, r"(?m)^ok verify-skill$")

    def test_house_verify_skills_in_plugin_cache_count(self):
        for name in ("verification-before-completion", "create-verification-skill", "maintain-verification-skill"):
            with self.subTest(name):
                d = os.path.join(self.plug, "cache", "id-1", "abc", "skills", name)
                os.makedirs(d, exist_ok=True)
                write(os.path.join(d, "SKILL.md"), "v\n")
                out = self.doc()[1]
                self.assertRegex(out, r"(?m)^ok verify-skill$")
                os.remove(os.path.join(d, "SKILL.md"))

    def test_flowingstack_tree_counts_as_verify(self):
        out = run("doctor", "--project", ROOT, "--plugins-dir", self.plug, "--disk-percent", "33")[1]
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
        self.assertRegex(out, r"(?m)^optional pstack\b")
        self.assertRegex(out, r"(?m)^optional craft\b")
        self.assertNotRegex(out, r"(?m)^ok pstack$")
        self.assertNotRegex(out, r"(?m)^ok craft$")

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
        self.assertIn("next: add AGENTS.md", out)
        self.assertNotIn("next: /setup-pstack", out)
        self.assertRegex(out, r"(?m)^optional model-rule\b")
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
        for h in ("name: doctor", "owner:", "source:", "last-reviewed:", "## Flow", "## Contract", "## Checks", "## Notes", "/setup-pstack",
                  "grill-rubric", "references/verify.md", "verification-before-completion",
                  "create-verification-skill", "maintain-verification-skill",
                  "FLOWINGSTACK_DOCTOR", ".flowingstack/doctor.tsv", "examples/house-doctor.tsv",
                  "optional"):
            self.assertIn(h, s)
        self.assertNotIn("exit 0 ⟺ no missing", s)


if __name__ == "__main__":
    unittest.main()
