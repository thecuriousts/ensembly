#!/usr/bin/env python3
import math
import re
from datetime import date, datetime, timedelta, timezone

LEAF = "ensembl" + "y"
FIX_WORD = re.compile(r"\bfix\b", re.I)
REVERT_MSG = re.compile(r'^(?:Revert\b|revert:\s|This reverts commit\b)', re.I | re.M)
PR_URL = re.compile(r"https://github\.com/([^/]+)/([^/]+)/pull/(\d+)")
PR_HASH = re.compile(r"(?<![\w/])#(\d+)\b")
GREEN = {"success", "skipped", "neutral"}


def parse_day(value):
    if isinstance(value, date) and not isinstance(value, datetime):
        return value
    return date.fromisoformat(str(value)[:10])


def parse_ts(value):
    if value is None or value == "":
        return None
    if isinstance(value, datetime):
        ts = value
    else:
        text = str(value).replace("Z", "+00:00")
        ts = datetime.fromisoformat(text)
    if ts.tzinfo is None:
        ts = ts.replace(tzinfo=timezone.utc)
    return ts.astimezone(timezone.utc)


def window_bounds(since, until):
    start = datetime.combine(parse_day(since), datetime.min.time(), tzinfo=timezone.utc)
    end = datetime.combine(parse_day(until), datetime.min.time(), tzinfo=timezone.utc) + timedelta(days=1)
    if end <= start:
        raise ValueError("until must be on or after since")
    return start, end


def default_window(today=None):
    until = today or datetime.now(timezone.utc).date()
    return until - timedelta(days=30), until


def in_window(ts, start, end):
    return ts is not None and start <= ts < end


def is_public(repo):
    if not isinstance(repo, dict):
        return False
    private = repo.get("private")
    vis = str(repo.get("visibility") or "").lower()
    if private is True:
        return False
    if vis and vis != "public":
        return False
    return private is False or vis == "public"


def take_public(repos):
    kept = []
    for repo in repos or []:
        if not is_public(repo):
            continue
        vis = str(repo.get("visibility") or "").lower()
        if repo.get("private") is True or (vis and vis != "public"):
            raise AssertionError("non-public repository present after public filter")
        kept.append(repo)
    for repo in kept:
        vis = str(repo.get("visibility") or "public").lower()
        if repo.get("private") is True or vis != "public":
            raise AssertionError("non-public repository present after public filter")
    return kept


def display_repo(full_name):
    name = str(full_name or "").strip().strip("/")
    if not name:
        return "unknown"
    leaf = name.rsplit("/", 1)[-1]
    if leaf == LEAF:
        return name + "/"
    return name


def quantile(values, q):
    if not values:
        return None
    xs = sorted(values)
    if len(xs) == 1:
        return float(xs[0])
    pos = q * (len(xs) - 1)
    lo = int(math.floor(pos))
    hi = int(math.ceil(pos))
    if lo == hi:
        return float(xs[lo])
    return xs[lo] * (hi - pos) + xs[hi] * (pos - lo)


def fmt_duration(seconds):
    if seconds is None:
        return "—"
    if seconds < 3600:
        return f"{seconds / 60:.1f}m"
    if seconds < 86400:
        return f"{seconds / 3600:.1f}h"
    return f"{seconds / 86400:.1f}d"


def fmt_rate(passed, considered):
    if not considered:
        return "—"
    return f"{round(100 * passed / considered)}% ({passed}/{considered})"


def fmt_day(d):
    d = parse_day(d)
    return f"{d.day} {d.strftime('%b')} {d.year}"


def normalize_conclusion(value):
    return str(value or "").strip().lower()


def first_pass_from_runs(runs):
    if not runs:
        return None
    chosen = {}
    for run in runs:
        name = run.get("name") or ""
        raw = run.get("started_at") or run.get("startedAt")
        started = parse_ts(raw) if raw else None
        prev = chosen.get(name)
        if prev is None or (started is not None and (prev[0] is None or started < prev[0])):
            chosen[name] = (started, run)
    if not chosen:
        return None
    return all(normalize_conclusion(item[1].get("conclusion")) in GREEN for item in chosen.values())


def cited_prs(title, body, default_repo):
    text = f"{title or ''}\n{body or ''}"
    found = []
    seen = set()
    for match in PR_URL.finditer(text):
        repo = f"{match.group(1)}/{match.group(2)}"
        number = int(match.group(3))
        key = (repo, number)
        if key not in seen:
            seen.add(key)
            found.append(key)
    for match in PR_HASH.finditer(text):
        key = (default_repo, int(match.group(1)))
        if key not in seen:
            seen.add(key)
            found.append(key)
    return found


def labelled_fix(labels):
    return any(str(label).strip().lower() == "fix" for label in labels or [])


def is_followup(pull, lookup):
    title = pull.get("title") or ""
    if not (FIX_WORD.search(title) or labelled_fix(pull.get("labels"))):
        return False
    merged = parse_ts(pull.get("merged_at"))
    if merged is None:
        return False
    repo = pull.get("repo")
    number = pull.get("number")
    for cited_repo, cited_number in cited_prs(title, pull.get("body"), repo):
        if cited_repo == repo and cited_number == number:
            continue
        earlier = lookup.get((cited_repo, cited_number))
        if not earlier:
            continue
        earlier_merged = parse_ts(earlier.get("merged_at"))
        if earlier_merged is None:
            continue
        delta = merged - earlier_merged
        if timedelta(0) < delta <= timedelta(days=7):
            return True
    return False


def is_revert_message(message):
    return bool(REVERT_MSG.search(message or ""))


def empty_row(repo):
    return {
        "repo": repo,
        "merged": 0,
        "first_pass": 0,
        "first_pass_considered": 0,
        "reverts_and_fixes": 0,
        "open_seconds": [],
    }


def summarize_row(row):
    times = row["open_seconds"]
    return {
        "repo": row["repo"],
        "merged": row["merged"],
        "first_pass": row["first_pass"],
        "first_pass_considered": row["first_pass_considered"],
        "first_pass_rate": fmt_rate(row["first_pass"], row["first_pass_considered"]),
        "reverts_and_fixes": row["reverts_and_fixes"],
        "median_seconds": quantile(times, 0.5),
        "p90_seconds": quantile(times, 0.9),
        "median": fmt_duration(quantile(times, 0.5)),
        "p90": fmt_duration(quantile(times, 0.9)),
    }


def compute(snapshot, since, until):
    start, end = window_bounds(since, until)
    public_repos = take_public(snapshot.get("repos"))
    public = {repo["full_name"] for repo in public_repos}
    lookup = {}
    for pull in snapshot.get("pulls") or []:
        repo = pull.get("repo")
        if repo not in public:
            continue
        lookup[(repo, pull.get("number"))] = pull
    window_pulls = []
    for pull in lookup.values():
        if in_window(parse_ts(pull.get("merged_at")), start, end):
            window_pulls.append(pull)
    window_pulls.sort(key=lambda item: (item.get("repo") or "", item.get("number") or 0))

    rows = {}
    for repo in sorted(public):
        rows[repo] = empty_row(repo)

    for pull in window_pulls:
        row = rows[pull["repo"]]
        row["merged"] += 1
        created = parse_ts(pull.get("created_at"))
        merged = parse_ts(pull.get("merged_at"))
        if created and merged and merged >= created:
            row["open_seconds"].append((merged - created).total_seconds())
        verdict = first_pass_from_runs(pull.get("check_runs"))
        if verdict is not None:
            row["first_pass_considered"] += 1
            if verdict:
                row["first_pass"] += 1
        if is_followup(pull, lookup):
            row["reverts_and_fixes"] += 1

    for commit in snapshot.get("commits") or []:
        repo = commit.get("repo")
        if repo not in public:
            continue
        if not in_window(parse_ts(commit.get("committed_at")), start, end):
            continue
        if is_revert_message(commit.get("message")):
            rows[repo]["reverts_and_fixes"] += 1

    active = []
    totals = empty_row("total")
    for repo in sorted(public):
        row = rows[repo]
        totals["merged"] += row["merged"]
        totals["first_pass"] += row["first_pass"]
        totals["first_pass_considered"] += row["first_pass_considered"]
        totals["reverts_and_fixes"] += row["reverts_and_fixes"]
        totals["open_seconds"].extend(row["open_seconds"])
        if row["merged"] or row["reverts_and_fixes"]:
            active.append(summarize_row(row))

    return {
        "since": parse_day(since),
        "until": parse_day(until),
        "public_repo_count": len(public),
        "active_repo_count": len(active),
        "rows": active,
        "total": summarize_row(totals),
    }
