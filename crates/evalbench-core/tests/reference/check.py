"""Must-have vs evidence check. A posting lists must-haves, each with keywords.
A pack provides evidence items (claim + optional proof URL). A must-have is
'evidenced' only if some evidence item mentions one of its keywords AND has a proof link."""
import json, re, sys

def check(posting, pack):
    gaps = []
    for mh in posting["must_haves"]:
        kws = [k.lower() for k in mh["keywords"]]
        ok = any(e.get("proof") and any(k in e["claim"].lower() for k in kws) for e in pack["evidence"])
        if not ok:
            gaps.append(mh["name"])
    return gaps

if __name__ == "__main__":
    posting, pack = (json.load(open(p)) for p in sys.argv[1:3])
    g = check(posting, pack)
    print(json.dumps({"gaps": g}))
    sys.exit(1 if g else 0)
