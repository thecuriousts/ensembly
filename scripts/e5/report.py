#!/usr/bin/env python3
"""Summarise the E5 treatment runs into summary.json and the REPORT.md table.

    python3 -I scripts/e5/report.py DIR COMMIT           print summary.json and the table
    python3 -I scripts/e5/report.py DIR COMMIT --write   write DIR/summary.json and the table in DIR/REPORT.md
    python3 -I scripts/e5/report.py DIR COMMIT --check   exit 1 if they differ (the commit line is not compared)

DIR holds off-r1..r3 and on-r1..r3, each with traces.jsonl and score.json.
Synthetic replies: the numbers show the pipeline, not a pulse or model effect.
"""
import hashlib
import json
import os
import sys

ARMS = ("off", "on")
RERUNS = (1, 2, 3)
BEGIN, END = "<!-- e5-table:begin -->", "<!-- e5-table:end -->"
COMMIT_PREFIX = "Generated at commit "


def mean10(xs):
    """Mean with one decimal, as a string (no floats in the summary)."""
    q, r = divmod(10 * sum(xs) * 2 + len(xs), 2 * len(xs))
    return "%d.%d" % divmod(q, 10)


def pct10(a, b):
    return "%d.%d" % divmod((1000 * a * 2 + b) // (2 * b), 10)


def summarise(d):
    out = {"arms": {}, "reruns": len(RERUNS), "synthetic": True}
    hashes, models, tasks = set(), set(), set()
    for arm in ARMS:
        runs = []
        prompt_ids = set()
        for r in RERUNS:
            run = os.path.join(d, "%s-r%d" % (arm, r))
            s = json.load(open(os.path.join(run, "score.json"), encoding="utf-8"))
            for line in open(os.path.join(run, "traces.jsonl"), encoding="utf-8"):
                prompt_ids.add(json.loads(line)["prompt_id"])
            hashes.add(s["task_set_hash"])
            models.update(s["model_ids"])
            tasks.add(s["tasks"])
            runs.append(s)
        fp = [s["first_pass"] for s in runs]
        sc = [s["score"] for s in runs]
        n = sum(s["tasks"] for s in runs)
        out["arms"][arm] = {
            "first_pass": fp,
            "median_tokens_passing": [s["median_tokens_passing"] for s in runs],
            "pooled_first_pass": "%d/%d" % (sum(fp), n),
            "pooled_rate_pct": pct10(sum(fp), n),
            "prompt_ids": sorted(prompt_ids),
            "run_ids": [s["run_id"] for s in runs],
            "score": sc,
            "score_max": max(sc),
            "score_mean": mean10(sc),
            "score_min": min(sc),
        }
    assert len(hashes) == 1 and len(tasks) == 1, (hashes, tasks)
    out["dataset_hash"] = hashes.pop()
    out["model_ids"] = sorted(models)
    out["n_tasks"] = tasks.pop()
    return out


def table(s):
    rows = [
        "| Arm | Prompt id | Score per rerun | Min to max | Mean score | Pooled first pass | Median tokens per passing task |",
        "|---|---|---|---|---|---|---|",
    ]
    for arm in ARMS:
        a = s["arms"][arm]
        rows.append("| pulse %s | `%s` | %s | %d to %d | %s | %s (%s%%) | %s |" % (
            arm, ", ".join(a["prompt_ids"]), ", ".join(map(str, a["score"])),
            a["score_min"], a["score_max"], a["score_mean"], a["pooled_first_pass"],
            a["pooled_rate_pct"], ", ".join(map(str, a["median_tokens_passing"]))))
    rows += [
        "",
        "n = %d tasks per run, %d reruns per arm. Dataset hash `%s`. Model id %s. "
        "Synthetic replies (see `scripts/e5/synthetic_replies.py`): not a model result, not a CV number."
        % (s["n_tasks"], s["reruns"], s["dataset_hash"], ", ".join("`%s`" % m for m in s["model_ids"])),
    ]
    return "\n".join(rows)


def splice(md, tbl, commit):
    i, j = md.index(BEGIN), md.index(END)
    md = md[: i + len(BEGIN)] + "\n" + tbl + "\n" + md[j:]
    lines = [COMMIT_PREFIX + "`%s`." % commit if l.startswith(COMMIT_PREFIX) else l
             for l in md.split("\n")]
    return "\n".join(lines)


def strip_commit(md):
    return "\n".join(l for l in md.split("\n") if not l.startswith(COMMIT_PREFIX))


def main(argv):
    if len(argv) not in (3, 4) or argv[3:] not in ([], ["--write"], ["--check"]):
        print(__doc__, file=sys.stderr)
        return 2
    d, commit, mode = argv[1], argv[2], (argv[3] if len(argv) == 4 else "")
    s = summarise(d)
    body = json.dumps(s, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n"
    tbl = table(s)
    sp, rp = os.path.join(d, "summary.json"), os.path.join(d, "REPORT.md")
    if mode == "--write":
        open(sp, "w", encoding="utf-8", newline="\n").write(body)
        md = open(rp, encoding="utf-8").read()
        open(rp, "w", encoding="utf-8", newline="\n").write(splice(md, tbl, commit))
        return 0
    if mode == "--check":
        bad = 0
        if open(sp, encoding="utf-8").read() != body:
            print("differs: summary.json")
            bad = 1
        md = open(rp, encoding="utf-8").read()
        if strip_commit(splice(md, tbl, commit)) != strip_commit(md):
            print("differs: REPORT.md table")
            bad = 1
        return bad
    sys.stdout.write(body + "\n" + tbl + "\n")
    print("sha256(summary.json) " + hashlib.sha256(body.encode()).hexdigest())
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
