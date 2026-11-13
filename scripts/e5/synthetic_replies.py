#!/usr/bin/env python3
"""Write the synthetic replies for the E5 pulse on/off treatment run.

These replies are made up by this script, not returned by a model. A seeded
generator answers each frozen E1 task correctly or makes one kind of mistake.
The chance of a mistake is a parameter per arm (P_WRONG below), so any gap between
the arms is put there by this file. The run exercises the pipeline (same tasks,
same model id, same prompt plus the pulse pack, three reruns, spread, hashes); it
measures nothing about pulse or about any model. Not a CV number.

    python3 -I scripts/e5/synthetic_replies.py           write the replies
    python3 -I scripts/e5/synthetic_replies.py --check   exit 1 if the committed files differ
"""
import json
import os
import random
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CORE = os.path.join(ROOT, "crates", "evalbench-core", "fixtures")
OUT = os.path.join(ROOT, "crates", "evalbench-cli", "treatments", "e5-pulse")
MODEL_ID = "synthetic-replay-model"  # the E1 baseline's id: same model id in both arms
PULSE_HEADER = "\nPulse (canonical pulse-pack-v1 from earlier sessions; context only):\n"
# Synthetic parameters, chosen before the run and not tuned afterwards.
P_WRONG = {"off": 0.40, "on": 0.25}
RERUNS = (1, 2, 3)
PRICE_IN, PRICE_OUT = 3, 15  # synthetic micro-USD per token


def read(*parts):
    with open(os.path.join(*parts), encoding="utf-8") as f:
        return f.read()


def render(template, posting, pack):
    """The E1 render: each marker replaced once, in one pass."""
    head, rest = template.split("{{posting}}", 1)
    mid, tail = rest.split("{{pack}}", 1)
    return head + posting + mid + pack + tail


def tokens(text):
    return (len(text.encode("utf-8")) + 3) // 4


def answer(rng, expected, names, p_wrong):
    if rng.random() >= p_wrong:
        return json.dumps({"gaps": expected}, separators=(",", ":"), ensure_ascii=False)
    kinds = ["prose"]
    if expected:
        kinds.append("drop")
    extra = [n for n in names if n not in expected]
    if extra:
        kinds.append("extra")
    kind = rng.choice(kinds)
    if kind == "drop":
        gaps = expected[1:]
    elif kind == "extra":
        gaps = expected + [extra[0]]
    else:
        return "Gaps: " + (", ".join(expected) or "none")
    return json.dumps({"gaps": gaps}, separators=(",", ":"), ensure_ascii=False)


def build():
    template = read(CORE, "prompts", "gaps-v1.txt")
    pulse = read(CORE, "pulse", "e5-pulse.json")
    tasks = sorted(os.listdir(os.path.join(CORE, "tasks")))
    files = {}
    for arm in ("off", "on"):
        for r in RERUNS:
            rng = random.Random(f"e5-{arm}-r{r}")
            lines = []
            for t in tasks:
                posting = read(CORE, "tasks", t, "posting.json")
                pack = read(CORE, "tasks", t, "pack.json")
                expected = json.loads(read(CORE, "tasks", t, "expected.json"))["gaps"]
                names = [m["name"] for m in json.loads(posting)["must_haves"]]
                prompt = render(template, posting, pack)
                if arm == "on":
                    prompt += PULSE_HEADER + pulse
                text = answer(rng, expected, names, P_WRONG[arm])
                tin, tout = tokens(prompt), tokens(text) + 2
                lines.append(json.dumps({
                    "task_id": t, "model_id": MODEL_ID, "text": text,
                    "tokens_in": tin, "tokens_out": tout,
                    "cost_micro_usd": tin * PRICE_IN + tout * PRICE_OUT,
                }, separators=(",", ":"), ensure_ascii=False) + "\n")
            files[os.path.join(OUT, f"{arm}-r{r}", "replies.jsonl")] = "".join(lines)
    return files


def main(argv):
    check = argv[1:] == ["--check"]
    if argv[1:] and not check:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    bad = 0
    for path, body in build().items():
        if check:
            try:
                same = read(path) == body
            except OSError:
                same = False
            if not same:
                print("differs:", os.path.relpath(path, ROOT))
                bad = 1
        else:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8", newline="\n") as f:
                f.write(body)
    return bad


if __name__ == "__main__":
    sys.exit(main(sys.argv))
