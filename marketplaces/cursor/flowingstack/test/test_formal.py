import os
import re
import shutil
import tempfile
import unittest

from lib import ROOT, read, run, write

SPEC = os.path.join(ROOT, "formal", "flowingstack.tla")
CFG = os.path.join(ROOT, "formal", "flowingstack.cfg")
SKIP = 3


def rows(machine):
    out = []
    for l in read(os.path.join(ROOT, "formal", "machine.tsv")).splitlines():
        if l and not l.startswith("#"):
            c = l.split("\t")
            if c[0] == machine:
                out.append((c[1], c[2]))
    return sorted(set(out))


def spec_edges(text):
    body = re.search(r"Edges ==\s*\{(.*?)\}\n", text, re.S).group(1)
    return sorted(set(re.findall(r'<<"(\w+)",\s*"(\w+)">>', body)))


def chart_edges(text):
    return sorted({(a, b) for a, b in re.findall(r"^\s*(\w+) --> (\w+)", text, re.M)})


def mutated(old, new):
    d = tempfile.mkdtemp()
    shutil.copy(CFG, d)
    t = read(SPEC)
    assert old in t, old
    write(os.path.join(d, "flowingstack.tla"), t.replace(old, new))
    return d, os.path.join(d, "flowingstack.tla")


class Source(unittest.TestCase):
    def test_spec_edges_match_machine_tsv(self):
        self.assertEqual(spec_edges(read(SPEC)), rows("verified"))

    def test_formal_page_chart_matches_machine_tsv(self):
        page = read(os.path.join(ROOT, "docs", "formal.md"))
        chart = re.search(r"stateDiagram-v2\n(.*?)```", page, re.S).group(1)
        self.assertEqual([e for e in chart_edges(chart)], rows("verified"))

    def test_spec_states_round_cap_and_properties(self):
        t = read(SPEC)
        for name in ("RoundCap", "ShipFrozen", "AppendOnly", "EventuallyClosed"):
            self.assertIn(name, read(CFG))
            self.assertIn(name + " ==", t)


class Tlc(unittest.TestCase):
    def tlc(self, *a):
        code, out = run("formal-check", *a)
        if code == SKIP:
            self.skipTest(out.strip())
        return code, out

    def test_spec_passes_and_reports_states(self):
        code, out = self.tlc()
        self.assertEqual(code, 0, out)
        self.assertRegex(out, r"tlc ok states=\d+ distinct=\d+")

    def test_round_cap_four_fails(self):
        d, p = mutated("MaxRounds == 3", "MaxRounds == 4")
        try:
            code, out = self.tlc("--spec", p)
            self.assertEqual(code, 1, out)
            self.assertIn("RoundCap", out)
        finally:
            shutil.rmtree(d)

    def test_ship_without_frozen_fails(self):
        d, p = mutated("Ship == frozen /\\ ", "Ship == ")
        try:
            code, out = self.tlc("--spec", p)
            self.assertEqual(code, 1, out)
            self.assertIn("ShipFrozen", out)
        finally:
            shutil.rmtree(d)

    def test_missing_tools_skip_with_code_3_never_pass(self):
        code, out = run_env("formal-check", JAVA="/nonexistent/java", TLA2TOOLS="/nonexistent.jar", FLOWINGSTACK_OFFLINE="1",
                            XDG_CACHE_HOME="/nonexistent", FLOWINGSTACK_CACHE="/nonexistent")
        self.assertEqual(code, SKIP, out)
        self.assertIn("skip", out)


class Bend(unittest.TestCase):
    def test_ledger_laws_check(self):
        code, out = run("formal-check", "--bend")
        if code == SKIP:
            self.skipTest(out.strip())
        self.assertEqual(code, 0, out)
        self.assertIn("bend ok", out)

    def test_broken_close_fails(self):
        src = os.path.join(ROOT, "formal", "bend", "ledger.bend")
        d = tempfile.mkdtemp()
        try:
            p = os.path.join(d, "ledger.bend")
            write(p, read(src).replace("case Closed{run}: Closed{run}", "case Closed{run}: Opened{run}"))
            code, out = run("formal-check", "--bend", p)
            if code == SKIP:
                self.skipTest(out.strip())
            self.assertEqual(code, 1, out)
        finally:
            shutil.rmtree(d)

    def test_bend_missing_skips(self):
        code, out = run_env("formal-check", "--bend", BEND="/nonexistent/bend", HOME=tempfile.gettempdir() + "/nohome")
        self.assertEqual(code, SKIP, out)


def run_env(script, *a, **env):
    import subprocess
    import sys
    p = subprocess.run([sys.executable, os.path.join(ROOT, "scripts", script), *a], capture_output=True, text=True,
                       cwd=ROOT, env={**os.environ, "FLOWINGSTACK_ROOT": ROOT, "PATH": "/usr/bin:/bin", **env})
    return p.returncode, p.stdout + p.stderr


if __name__ == "__main__":
    unittest.main()
