import glob
import json
import os
import platform
import re
import subprocess

from rootpath import plugin_root

TOOLS = ("git", "python3", "claude", "cursor-agent")


def manifest(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except FileNotFoundError:
        return None
    except (ValueError, OSError):
        return False


def tool_version(name):
    try:
        p = subprocess.run([name, "--version"], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    m = re.search(r"\d+\.\d+(?:\.\d+)?", p.stdout + p.stderr)
    return m.group(0) if m else "present"


def claude_installs(name):
    d = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.join(os.path.expanduser("~"), ".claude")
    p = os.path.join(d, "plugins", "installed_plugins.json")
    if not os.path.isdir(d):
        return "absent", []
    m = manifest(p)
    if m is None:
        return "no-plugins", []
    if m is False or not isinstance(m.get("plugins"), dict):
        return "broken", []
    rows = []
    for key, entries in m["plugins"].items():
        if key.split("@")[0] != name:
            continue
        for e in entries if isinstance(entries, list) else []:
            ok = os.path.isdir(e.get("installPath", ""))
            rows.append((key, e.get("version", "?"), e.get("scope", "?"), ok))
    return "present", rows


def cursor_installs(name):
    d = os.path.join(os.path.expanduser("~"), ".cursor", "plugins")
    if not os.path.isdir(d):
        return "absent", []
    rows = []
    for p in sorted(glob.glob(os.path.join(d, "**", ".cursor-plugin", "plugin.json"), recursive=True)):
        m = manifest(p)
        if m and m.get("name") == name:
            rows.append((m.get("version", "?"),))
    return "present", rows


def semver(v):
    return tuple(int(x) for x in re.findall(r"\d+", v)[:3])


def latest_tag(name, remote, offline):
    if offline:
        return "skipped", None
    try:
        p = subprocess.run(["git", "ls-remote", "--tags", "--refs", remote, f"refs/tags/{name}--v*"],
                           capture_output=True, text=True, timeout=8,
                           env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
    except (OSError, subprocess.TimeoutExpired):
        return "offline", None
    if p.returncode != 0:
        return "offline", None
    tags = [l.split("refs/tags/")[1] for l in p.stdout.splitlines() if "refs/tags/" in l]
    vs = sorted((semver(t.split("--v", 1)[1]), t) for t in tags if t.startswith(f"{name}--v"))
    return ("none", None) if not vs else ("ok", vs[-1][1])


def run(offline=False):
    root = plugin_root()
    bad = []
    cur = manifest(os.path.join(root, ".cursor-plugin", "plugin.json"))
    cla = manifest(os.path.join(root, ".claude-plugin", "plugin.json"))
    if not cur:
        print("fail manifest cursor\tunreadable .cursor-plugin/plugin.json")
        return 2
    name, ver = cur.get("name", "?"), cur.get("version", "?")
    print(f"{name} {ver}")
    print(f"manifest cursor {ver}")
    if cla is None:
        print("optional manifest claude\tmissing .claude-plugin/plugin.json")
    elif cla is False or cla.get("version") != ver or cla.get("name") != name:
        print(f"fail manifest claude {(cla or {}).get('version', '?')}\tmust match the cursor manifest")
        bad.append("manifest")
    else:
        print(f"manifest claude {cla['version']}")
    print(f"platform {platform.system().lower()} {platform.machine()} python {platform.python_version()}")
    for t in TOOLS:
        v = tool_version(t)
        print(f"tool {t} {v}" if v else f"tool {t} missing")
    state, rows = claude_installs(name)
    if state == "broken":
        print("fail harness claude-code\tinstalled_plugins.json unreadable")
        bad.append("claude")
    elif not rows:
        print(f"harness claude-code {state} {name} not-installed")
    for key, v, scope, ok in rows:
        if ok:
            print(f"harness claude-code {key} {v} {scope}")
        else:
            print(f"fail harness claude-code {key} {v}\tinstall path missing, reinstall the plugin")
            bad.append(key)
    state, rows = cursor_installs(name)
    if not rows:
        print(f"harness cursor {state} {name} not-installed")
    for (v,) in rows:
        print(f"harness cursor {name} {v}")
    remote = os.environ.get("FLOWINGSTACK_DOCTOR_REMOTE") or cur.get("repository", "").rstrip("/")
    st, tag = latest_tag(name, remote, offline or os.environ.get("FLOWINGSTACK_OFFLINE") == "1")
    if st == "ok":
        newer = semver(tag.split("--v", 1)[1]) > semver(ver)
        print(f"latest {tag}" + ("\tupdate available" if newer else ""))
    else:
        print(f"latest {st}")
    return 1 if bad else 0
