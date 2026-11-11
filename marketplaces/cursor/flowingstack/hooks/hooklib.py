import hashlib
import json
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime

UNCLASSIFIED = {"missing-config", "unnamed-check", "unknown-tool", "unknown-error"}
MAX_CONT = 8
NUDGE_AT = 3
DENY_AT = 5


def here():
    return os.path.dirname(os.path.abspath(__file__))


def plugin_root():
    return os.environ.get("FLOWINGSTACK_ROOT") or os.path.dirname(here())


def repo_root(arg=None):
    return os.path.abspath(arg or os.environ.get("FS_REPO_ROOT") or os.getcwd())


def state_dir():
    return os.environ.get("FS_HOOK_STATE_DIR") or os.path.join(tempfile.gettempdir(), "flowingstack-hooks")


def session_id(payload=None):
    payload = payload or {}
    return os.environ.get("FS_SESSION_ID") or payload.get("session_id") or payload.get("conversation_id") or "default"


def read_stdin():
    raw = sys.stdin.read()
    if not raw.strip():
        return {}
    try:
        return json.loads(raw)
    except ValueError:
        return {}


def write_json(path, data):
    parent = os.path.dirname(path)
    if parent:
        os.makedirs(parent, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f)
        f.write("\n")


def load_json(path, default):
    if not os.path.isfile(path):
        return default
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (ValueError, OSError):
        return default


def head_sha(repo):
    p = subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"], capture_output=True, text=True)
    sha = p.stdout.strip()
    return sha if p.returncode == 0 and sha else None


def checks_path(explicit, repo, plugin):
    if explicit:
        return explicit
    env = os.environ.get("FS_CHECKS_FILE")
    if env:
        return env
    local = os.path.join(repo, ".flowingstack", "checks.tsv")
    if os.path.isfile(local):
        return local
    shipped = os.path.join(plugin, "hooks", "checks.tsv")
    if os.path.isfile(shipped):
        return shipped
    return None


def load_checks(path):
    if not path or not os.path.isfile(path):
        return None, "missing-config", "missing config: checks file not found"
    rows = []
    for n, line in enumerate(open(path, encoding="utf-8"), 1):
        if not line.strip() or line.startswith("#"):
            continue
        name, sep, cmd = line.rstrip("\n").partition("\t")
        name, cmd = name.strip(), cmd.strip()
        if n == 1 and name == "name" and cmd in ("command", "cmd"):
            continue
        rows.append((name, cmd))
    if not rows:
        return None, "missing-config", "missing config: checks file has no rows"
    return rows, None, None


def resolve_parts(command, repo, plugin):
    parts = shlex.split(command)
    if not parts:
        return parts
    head = parts[0]
    if os.path.isabs(head):
        return parts
    for base in (repo, plugin):
        cand = os.path.join(base, head)
        if os.path.isfile(cand):
            return [cand, *parts[1:]]
    return parts


def run_one(name, command, repo, plugin):
    if not name:
        return False, "unnamed-check", None, "no check named"
    if not command:
        return False, "unknown-tool", name, f"unknown tool: empty command for {name}"
    try:
        parts = resolve_parts(command, repo, plugin)
    except ValueError as e:
        return False, "unknown-error", name, f"unknown error: {e}"
    if not parts:
        return False, "unknown-tool", name, f"unknown tool: empty command for {name}"
    exe = parts[0]
    if not os.path.isfile(exe) and shutil.which(exe) is None:
        return False, "unknown-tool", name, f"unknown tool: {exe}"
    try:
        timeout = int(os.environ.get("FS_CHECK_TIMEOUT") or "120")
        p = subprocess.run(parts, cwd=repo, capture_output=True, text=True, timeout=timeout)
    except FileNotFoundError:
        return False, "unknown-tool", name, f"unknown tool: {exe}"
    except Exception as e:
        return False, "unknown-error", name, f"unknown error: {e}"
    if p.returncode != 0:
        detail = (p.stderr or p.stdout or "").strip()
        extra = f": {detail}" if detail else ""
        return False, "check-fail", name, f"check {name} failed{extra}"
    return True, "pass", name, None


def run_checks(path, repo, plugin):
    rows, kind, reason = load_checks(path)
    if rows is None:
        return {"ok": False, "classified": False, "kind": kind, "check": None, "reason": reason}
    for name, command in rows:
        ok, kind, check, reason = run_one(name, command, repo, plugin)
        if not ok:
            return {"ok": False, "classified": kind not in UNCLASSIFIED, "kind": kind, "check": check, "reason": reason}
    return {"ok": True, "classified": True, "kind": "pass", "check": None, "reason": None}


def marker_paths(session, marker=None, reviewer=None):
    base = state_dir()
    os.makedirs(base, exist_ok=True)
    return (
        marker or os.environ.get("FS_READY_MARKER") or os.path.join(base, f"{session}.ready.json"),
        reviewer or os.environ.get("FS_REVIEWER_REQUEST") or os.path.join(base, f"{session}.reviewer.txt"),
    )


def write_ready(repo, session, marker=None, reviewer=None):
    sha = head_sha(repo)
    marker, reviewer = marker_paths(session, marker, reviewer)
    payload = {
        "head": sha,
        "status": "ready" if sha else "no-head",
        "review": "A no-context reviewer must pass this exact sha. If HEAD moves, this marker is invalid.",
    }
    write_json(marker, payload)
    parent = os.path.dirname(reviewer)
    if parent:
        os.makedirs(parent, exist_ok=True)
    with open(reviewer, "w", encoding="utf-8") as f:
        f.write(
            "Review request: no-context reviewer\n"
            f"Exact sha: {sha or 'missing'}\n"
            "Do not invoke a cloud reviewer from this hook\n"
            "Pass only this sha; if HEAD moves, review again\n"
        )
    return marker, reviewer, sha


def verify_marker(path, repo):
    data = load_json(path, {})
    sha = head_sha(repo)
    return bool(data.get("head") and sha and data.get("head") == sha and data.get("status") == "ready")


def route_unknown(kind):
    plugin = plugin_root()
    script = os.path.join(plugin, "scripts", "route-rule")
    task = f"unknown unclassified {kind}"
    env = {**os.environ, "FLOWINGSTACK_ROOT": plugin, "PYTHONDONTWRITEBYTECODE": "1"}
    try:
        p = subprocess.run([sys.executable, script, "--task", task, "--json"],
                           capture_output=True, text=True, cwd=plugin, env=env)
        if p.returncode == 0:
            data = json.loads(p.stdout)
            bits = [data.get("rule") or f"route to {data.get('lead')}; do not do it yourself"]
            if data.get("note"):
                bits.append(data["note"])
            return "Do not guess. Route this unknown through flowingstack. " + " ".join(bits)
    except (OSError, ValueError):
        pass
    return "Do not guess. Route this unknown through flowingstack."


def continuation_message(result):
    reason = result.get("reason") or "unclassified failure"
    if result.get("classified"):
        return reason
    return f"{reason}. {route_unknown(result.get('kind') or 'unknown-error')}"


def state_path(session):
    os.makedirs(state_dir(), exist_ok=True)
    return os.path.join(state_dir(), f"{session}.json")


def handle_stop(payload, checks=None, repo=None, marker=None, reviewer=None):
    if payload.get("status") == "aborted":
        return {}
    session = session_id(payload)
    repo = repo_root(repo)
    plugin = plugin_root()
    path = checks_path(checks, repo, plugin)
    result = run_checks(path, repo, plugin)
    result["head"] = head_sha(repo)
    if result["ok"]:
        m, r, sha = write_ready(repo, session, marker, reviewer)
        result["marker"] = m
        result["reviewer"] = r
        result["head"] = sha
        st = load_json(state_path(session), {"continuations": 0, "result": None})
        st["result"] = "ready"
        write_json(state_path(session), st)
        return {}
    st = load_json(state_path(session), {"continuations": 0, "result": None})
    st["continuations"] = int(st.get("continuations") or 0) + 1
    limit = int(os.environ.get("FS_MAX_CONTINUATIONS") or MAX_CONT)
    msg = continuation_message(result)
    if st["continuations"] > limit:
        st["result"] = "failed-gate"
        write_json(state_path(session), st)
        return {}
    write_json(state_path(session), st)
    return {"decision": "block", "reason": msg, "followup_message": msg}


def norm_input(value):
    if isinstance(value, str):
        try:
            value = json.loads(value)
        except ValueError:
            return " ".join(value.split())
    if isinstance(value, (dict, list)):
        return json.dumps(value, sort_keys=True, separators=(",", ":"))
    return " ".join(str(value).split())


def call_hash(payload):
    name = payload.get("tool_name") or payload.get("tool") or ""
    raw = payload.get("tool_input")
    if raw is None:
        raw = payload.get("input")
    if raw is None:
        raw = payload.get("arguments")
    if raw is None:
        raw = {}
    return hashlib.sha256(f"{name}\n{norm_input(raw)}".encode()).hexdigest()


def now_iso():
    return datetime.now().astimezone().isoformat(timespec="seconds")


def block_queue(reason):
    path = os.environ.get("FS_QUEUE_FILE")
    row_id = os.environ.get("FS_QUEUE_ROW")
    if not path or not row_id or not os.path.isfile(path):
        return
    lines = []
    for line in open(path, encoding="utf-8"):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except ValueError:
            lines.append(line.rstrip("\n"))
            continue
        if row.get("id") == row_id:
            row["state"] = "blocked"
            row["result"] = reason
            row["updated"] = now_iso()
        lines.append(json.dumps(row, separators=(",", ":")))
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + ("\n" if lines else ""))


def handle_pretool(payload):
    session = session_id(payload)
    path = os.path.join(state_dir(), f"{session}.repeat.json")
    h = call_hash(payload)
    st = load_json(path, {"hash": None, "count": 0})
    if st.get("hash") == h:
        st["count"] = int(st.get("count") or 0) + 1
    else:
        st = {"hash": h, "count": 1}
    write_json(path, st)
    nudge_at = int(os.environ.get("FS_NUDGE_AT") or NUDGE_AT)
    deny_at = int(os.environ.get("FS_DENY_AT") or DENY_AT)
    n = st["count"]
    if n >= deny_at:
        reason = f"Repeat guard: {n} identical consecutive calls. Stopping."
        block_queue(reason)
        return {
            "permission": "deny",
            "decision": "block",
            "reason": reason,
            "agent_message": reason,
            "user_message": reason,
            "continue": False,
            "stopReason": reason,
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            },
        }
    if n >= nudge_at:
        msg = f"Repeat guard: identical call {n} times. Change the input or stop looping."
        return {
            "permission": "allow",
            "agent_message": msg,
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow",
                "additionalContext": msg,
            },
        }
    return {
        "permission": "allow",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
        },
    }
