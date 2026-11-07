import hashlib
import json
import os
import tempfile
import unittest

from lib import ROOT, read, run, write

HOUSE = "c9950759ac2d50828b308d6dd40445752ab028f52f9b1ca7fb0316b5249cda21"
CLEAN = {"FLOWINGSTACK_ROUTING": None}
HEADER = "task_type\tlead\tnote\tkeywords\n"


def route(*a, **k):
    env = {**CLEAN, **k.pop("env", {})}
    return run("route-rule", *a, env=env, **k)


class Defaults(unittest.TestCase):
    def test_no_user_rules_use_generic_roles(self):
        rows = {
            "fix the PR and release": ("engineering", "engineering"),
            "please review the grill": ("review", "review"),
            "update the readme docs": ("docs", "docs"),
        }
        for task, (t, lead) in rows.items():
            with self.subTest(task):
                code, out = route("--task", task)
                self.assertEqual(code, 0, out)
                self.assertIn(t, out)
                self.assertIn(f"route to {lead}; do not do it yourself", out)
        code, out = route("--list")
        self.assertEqual(code, 0, out)
        for lead in ("engineering", "review", "docs"):
            self.assertIn(lead, out)
        for house in ("Fleet Desk", "Chief of Staff", "Life Desk", "Director", "Originals"):
            self.assertNotIn(house, out)

    def test_shipped_table_has_generic_leads_only(self):
        rows = [l.split("\t") for l in read(os.path.join(ROOT, "routing.tsv")).splitlines()[1:] if l]
        leads = [r[1] for r in rows]
        self.assertEqual(leads, ["engineering", "review", "docs"])
        self.assertEqual(len(leads), len(set(leads)))

    def test_unknown_task(self):
        code, out = route("--task", "zzz qqq")
        self.assertEqual(code, 2)
        self.assertIn("no match: ask engineering", out)

    def test_json(self):
        code, out = route("--task", "update the readme docs", "--json")
        self.assertEqual(code, 0, out)
        self.assertEqual(json.loads(out)["lead"], "docs")

    def test_missing_extension_file_is_fine(self):
        code, out = route("--list", env={"FLOWINGSTACK_ROUTING": os.path.join(ROOT, "no-such-routing.tsv")})
        self.assertEqual(code, 0, out)
        self.assertIn("engineering", out)
        self.assertNotIn("Fleet Desk", out)

    def test_house_example_is_not_the_default(self):
        example = read(os.path.join(ROOT, "examples", "house-routing.tsv"))
        self.assertEqual(hashlib.sha256(example.encode()).hexdigest(), HOUSE)
        code, out = route("--task", "enskild audit")
        self.assertEqual(code, 2, out)
        self.assertIn("ask engineering", out)


class Overlay(unittest.TestCase):
    def write_rows(self, directory, body):
        os.makedirs(os.path.join(directory, ".flowingstack"), exist_ok=True)
        path = os.path.join(directory, ".flowingstack", "routing.tsv")
        write(path, HEADER + body)
        return path

    def test_added_rule_is_visible(self):
        with tempfile.TemporaryDirectory() as d:
            self.write_rows(d, "desk-demo\tadded-role\tn\tzzz\n")
            code, out = route("--task", "zzz", cwd=d)
            self.assertEqual(code, 0, out)
            self.assertIn("added-role", out)
            self.assertIn("route to added-role; do not do it yourself", out)
            code, listed = route("--list", cwd=d)
            self.assertEqual(code, 0, listed)
            self.assertIn("engineering", listed)
            self.assertIn("added-role", listed)

    def test_tweak_wins_over_shipped_role(self):
        with tempfile.TemporaryDirectory() as d:
            self.write_rows(d, "engineering\trelease-desk\tn\tcode,release,pr\n")
            code, out = route("--task", "fix the PR and release", cwd=d)
            self.assertEqual(code, 0, out)
            self.assertIn("release-desk", out)
            self.assertNotIn("route to engineering;", out)
            code, out = route("--task", "update the readme docs", cwd=d)
            self.assertEqual(code, 0, out)
            self.assertIn("route to docs; do not do it yourself", out)

    def test_env_file_beats_local_file(self):
        with tempfile.TemporaryDirectory() as d:
            self.write_rows(d, "zzz\tlocal-role\tn\tzzz\n")
            env_file = os.path.join(d, "env.tsv")
            write(env_file, HEADER + "docs\tmanuals\tn\tdocs,readme\n")
            code, out = route("--task", "zzz", cwd=d, env={"FLOWINGSTACK_ROUTING": env_file})
            self.assertEqual(code, 2, out)
            code, out = route("--task", "update the readme docs", cwd=d, env={"FLOWINGSTACK_ROUTING": env_file})
            self.assertEqual(code, 0, out)
            self.assertIn("manuals", out)

    def test_missing_env_file_skips_local_file(self):
        with tempfile.TemporaryDirectory() as d:
            self.write_rows(d, "zzz\tlocal-role\tn\tzzz\n")
            code, out = route("--task", "zzz", cwd=d, env={"FLOWINGSTACK_ROUTING": os.path.join(d, "missing.tsv")})
            self.assertEqual(code, 2, out)
            self.assertIn("ask engineering", out)

    def test_house_example_adds_rows_when_selected(self):
        env = {"FLOWINGSTACK_ROUTING": os.path.join(ROOT, "examples", "house-routing.tsv")}
        code, out = route("--task", "enskild audit", env=env)
        self.assertEqual(code, 0, out)
        self.assertIn("Enskild Desk", out)
        code, out = route("--task", "update the readme docs", env=env)
        self.assertEqual(code, 0, out)
        self.assertIn("route to docs; do not do it yourself", out)

    def test_explicit_table_skips_user_file(self):
        with tempfile.TemporaryDirectory() as d:
            user = os.path.join(d, "user.tsv")
            write(user, HEADER + "zzz\tadded-role\tn\tzzz\n")
            code, out = route("--table", "examples/house-routing.tsv", "--list", env={"FLOWINGSTACK_ROUTING": user})
            self.assertEqual(code, 0, out)
            self.assertIn("Fleet Desk", out)
            self.assertNotIn("added-role", out)
            self.assertNotIn("engineering\tengineering", out)

    def test_bad_user_table_exits_2(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "bad.tsv")
            write(path, HEADER + "life\t\tn\tfamily\n")
            code, out = route("--list", env={"FLOWINGSTACK_ROUTING": path})
            self.assertEqual(code, 2)
            self.assertIn("missing lead", out)
            self.assertEqual(len(out.strip().splitlines()), 1, out)


class Malformed(unittest.TestCase):
    def check(self, name, word):
        code, out = route("--table", f"test/fixtures/route-rule/{name}.tsv", "--list")
        self.assertEqual(code, 2)
        self.assertEqual(len(out.strip().splitlines()), 1, out)
        self.assertIn(word, out)

    def test_missing_lead(self):
        self.check("missing-lead", "missing lead")

    def test_duplicate_type(self):
        self.check("duplicate-type", "duplicate type")

    def test_path_outside_root(self):
        code, out = route("--table", "../x.tsv", "--list")
        self.assertEqual(code, 2)


if __name__ == "__main__":
    unittest.main()
