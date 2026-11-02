import json
import os
import unittest

from lib import ROOT, read, run

ROWS = {
    "fix the PR and release": ("pro-engineering", "Fleet Desk"),
    "family energy plan": ("life", "Life Desk"),
    "AI-flow audit for a client": ("co-enskild", "Enskild Desk"),
    "edit the brand video": ("creative", "Director"),
    "draft an X post": ("x-content", "Originals"),
    "answer hiring mail for an interview": ("job-hunt", "Kanithanj Desk"),
    "irreversible cross-domain decision": ("cross-domain", "Chief of Staff"),
}


class Rows(unittest.TestCase):
    def test_each_row_routes(self):
        for task, (t, lead) in ROWS.items():
            with self.subTest(task):
                code, out = run("route-rule", "--task", task)
                self.assertEqual(code, 0, out)
                self.assertIn(t, out)
                self.assertIn(f"route to {lead}; do not do it yourself", out)

    def test_unknown_task(self):
        code, out = run("route-rule", "--task", "zzz qqq")
        self.assertEqual(code, 2)
        self.assertIn("no match: ask Chief of Staff", out)

    def test_json(self):
        code, out = run("route-rule", "--task", "draft an X post", "--json")
        self.assertEqual(code, 0, out)
        self.assertEqual(json.loads(out)["lead"], "Originals")

    def test_list(self):
        code, out = run("route-rule", "--list")
        self.assertEqual(code, 0, out)
        for _, lead in ROWS.values():
            self.assertIn(lead, out)

    def test_every_lead_once(self):
        rows = [l.split("\t") for l in read(os.path.join(ROOT, "routing.tsv")).splitlines()[1:] if l]
        leads = [r[1] for r in rows]
        self.assertEqual(len(leads), len(set(leads)))
        self.assertEqual(set(leads), {l for _, l in ROWS.values()})


class Malformed(unittest.TestCase):
    def check(self, name, word):
        code, out = run("route-rule", "--table", f"test/fixtures/route-rule/{name}.tsv", "--list")
        self.assertEqual(code, 2)
        self.assertEqual(len(out.strip().splitlines()), 1, out)
        self.assertIn(word, out)

    def test_missing_lead(self):
        self.check("missing-lead", "missing lead")

    def test_duplicate_type(self):
        self.check("duplicate-type", "duplicate type")

    def test_path_outside_root(self):
        code, out = run("route-rule", "--table", "../x.tsv", "--list")
        self.assertEqual(code, 2)


if __name__ == "__main__":
    unittest.main()
