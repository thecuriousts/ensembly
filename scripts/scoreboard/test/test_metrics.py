import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from metrics import (
    compute,
    display_repo,
    first_pass_from_runs,
    is_public,
    quantile,
    take_public,
    window_bounds,
)

FIX = os.path.join(HERE, "fixtures")


def load(name):
    with open(os.path.join(FIX, name)) as handle:
        return json.load(handle)


class PublicFilter(unittest.TestCase):
    def test_requires_public_visibility(self):
        self.assertTrue(is_public({"full_name": "alice/alpha", "visibility": "public", "private": False}))
        self.assertTrue(is_public({"full_name": "alice/alpha", "visibility": "PUBLIC", "private": False}))
        self.assertFalse(is_public({"full_name": "alice/hidden", "visibility": "private", "private": True}))
        self.assertFalse(is_public({"full_name": "alice/hidden", "visibility": "internal", "private": False}))
        self.assertFalse(is_public({"full_name": "alice/hidden", "visibility": "public", "private": True}))

    def test_take_public_drops_private_and_asserts_rest(self):
        kept = take_public([
            {"full_name": "alice/alpha", "visibility": "public", "private": False},
            {"full_name": "alice/hidden", "visibility": "private", "private": True},
        ])
        self.assertEqual([repo["full_name"] for repo in kept], ["alice/alpha"])
        for repo in kept:
            self.assertNotEqual(str(repo.get("visibility")).lower(), "private")
            self.assertIsNot(repo.get("private"), True)


class FirstPass(unittest.TestCase):
    def test_all_green(self):
        self.assertTrue(first_pass_from_runs([
            {"name": "ci", "started_at": "2026-10-01T00:00:00Z", "conclusion": "success"},
            {"name": "lint", "started_at": "2026-10-01T00:00:01Z", "conclusion": "SUCCESS"},
        ]))

    def test_skipped_or_neutral_with_success_is_green(self):
        self.assertTrue(first_pass_from_runs([
            {"name": "ci", "started_at": "2026-10-01T00:00:00Z", "conclusion": "success"},
            {"name": "optional", "started_at": "2026-10-01T00:00:01Z", "conclusion": "skipped"},
            {"name": "notice", "started_at": "2026-10-01T00:00:02Z", "conclusion": "NEUTRAL"},
        ]))

    def test_rerun_does_not_hide_first_failure(self):
        self.assertFalse(first_pass_from_runs([
            {"name": "ci", "started_at": "2026-10-01T00:00:00Z", "conclusion": "failure"},
            {"name": "ci", "started_at": "2026-10-01T00:10:00Z", "conclusion": "success"},
        ]))

    def test_missing_check_run_is_unknown(self):
        self.assertIsNone(first_pass_from_runs([]))
        self.assertIsNone(first_pass_from_runs(None))


class Quantile(unittest.TestCase):
    def test_empty_and_single(self):
        self.assertIsNone(quantile([], 0.5))
        self.assertEqual(quantile([10], 0.9), 10)

    def test_linear_interpolation(self):
        xs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        self.assertEqual(quantile(xs, 0.5), 5.5)
        self.assertAlmostEqual(quantile(xs, 0.9), 9.1)


class Window(unittest.TestCase):
    def test_inclusive_until_date(self):
        start, end = window_bounds("2026-10-10", "2026-11-09")
        self.assertEqual(start.isoformat(), "2026-10-10T00:00:00+00:00")
        self.assertEqual(end.isoformat(), "2026-11-10T00:00:00+00:00")


class MixedSnapshot(unittest.TestCase):
    def setUp(self):
        self.result = compute(load("mixed.json"), "2026-10-01", "2026-10-31")

    def test_private_repo_never_appears(self):
        names = [row["repo"] for row in self.result["rows"]] + [self.result["total"]["repo"]]
        blob = repr(self.result)
        self.assertNotIn("hidden", blob)
        self.assertNotIn("alice/hidden", names)
        self.assertEqual(self.result["public_repo_count"], 3)

    def test_merged_counts(self):
        by_repo = {row["repo"]: row for row in self.result["rows"]}
        self.assertEqual(by_repo["alice/alpha"]["merged"], 3)
        self.assertEqual(by_repo["alice/beta"]["merged"], 1)
        self.assertEqual(self.result["total"]["merged"], 4)
        self.assertNotIn("alice/idle", by_repo)

    def test_first_pass_skips_missing_and_counts_rerun(self):
        by_repo = {row["repo"]: row for row in self.result["rows"]}
        self.assertEqual(by_repo["alice/alpha"]["first_pass"], 2)
        self.assertEqual(by_repo["alice/alpha"]["first_pass_considered"], 3)
        self.assertEqual(by_repo["alice/alpha"]["first_pass_rate"], "67% (2/3)")
        self.assertEqual(by_repo["alice/beta"]["first_pass_rate"], "—")
        self.assertEqual(self.result["total"]["first_pass_rate"], "67% (2/3)")

    def test_followup_within_seven_days_only(self):
        by_repo = {row["repo"]: row for row in self.result["rows"]}
        self.assertEqual(by_repo["alice/alpha"]["reverts_and_fixes"], 1)
        self.assertEqual(by_repo["alice/beta"]["reverts_and_fixes"], 1)
        self.assertEqual(self.result["total"]["reverts_and_fixes"], 2)

    def test_open_to_merge_times(self):
        by_repo = {row["repo"]: row for row in self.result["rows"]}
        self.assertEqual(by_repo["alice/alpha"]["median"], "2.0h")
        self.assertEqual(by_repo["alice/beta"]["median"], "2.0d")
        self.assertEqual(by_repo["alice/beta"]["p90"], "2.0d")


class EmptySnapshot(unittest.TestCase):
    def test_no_prs(self):
        result = compute(load("empty.json"), "2026-10-01", "2026-10-31")
        self.assertEqual(result["public_repo_count"], 0)
        self.assertEqual(result["rows"], [])
        self.assertEqual(result["total"]["merged"], 0)
        self.assertEqual(result["total"]["first_pass_rate"], "—")
        self.assertEqual(result["total"]["median"], "—")
        self.assertEqual(result["total"]["p90"], "—")
        self.assertEqual(result["total"]["reverts_and_fixes"], 0)


class MissingCheck(unittest.TestCase):
    def test_missing_check_excluded_from_rate(self):
        result = compute(load("missing_check.json"), "2026-10-01", "2026-10-31")
        self.assertEqual(result["total"]["merged"], 1)
        self.assertEqual(result["total"]["first_pass_considered"], 0)
        self.assertEqual(result["total"]["first_pass_rate"], "—")


class RevertSnapshot(unittest.TestCase):
    def test_revert_commit_counts(self):
        result = compute(load("revert.json"), "2026-10-01", "2026-10-31")
        self.assertEqual(result["total"]["merged"], 1)
        self.assertEqual(result["total"]["reverts_and_fixes"], 1)
        self.assertEqual(result["total"]["first_pass_rate"], "100% (1/1)")


class PrivateOnly(unittest.TestCase):
    def test_private_snapshot_is_empty_without_names(self):
        result = compute(load("private_only.json"), "2026-10-01", "2026-10-31")
        self.assertEqual(result["public_repo_count"], 0)
        self.assertEqual(result["total"]["merged"], 0)
        self.assertNotIn("hidden", repr(result))


class DisplayName(unittest.TestCase):
    def test_special_leaf_gets_trailing_slash(self):
        full = "thecuriousts/" + ("ensembl" + "y")
        self.assertEqual(display_repo(full), full + "/")
        self.assertEqual(display_repo("alice/alpha"), "alice/alpha")


if __name__ == "__main__":
    unittest.main()
