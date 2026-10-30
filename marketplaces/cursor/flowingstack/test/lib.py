import os
import subprocess
import sys

ROOT = os.environ.get("FLOWINGSTACK_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIX = os.path.join(ROOT, "test", "fixtures")


def run(script, *args, stdin=None, cwd=None):
    p = subprocess.run([sys.executable, os.path.join(ROOT, "scripts", script), *args],
                       capture_output=True, text=True, input=stdin, cwd=cwd or ROOT,
                       env={**os.environ, "FLOWINGSTACK_ROOT": ROOT})
    return p.returncode, p.stdout + p.stderr
