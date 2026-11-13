#!/usr/bin/env python3
"""Build the E5 chart page from summary.json with the vendored Minicharts engine.

    python3 -I scripts/e5/build_chart.py           write chart/index.html
    python3 -I scripts/e5/build_chart.py --check   exit 1 if the committed page differs

Offline page: fonts come from chart/fonts (bundled, SIL OFL), never from a font host.
The colour softening is adapted from Minicharts' build.py (MIT, April Zhu, 2026).
"""
import colorsys
import html as H
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
RUN = ROOT / "crates" / "evalbench-cli" / "treatments" / "e5-pulse"
CHART = RUN / "chart"
VENDOR = CHART / "vendor" / "minicharts"
FONT_LINKS = (
    '<link rel="preconnect" href="https://fonts.googleapis.com">\n'
    '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>\n'
    '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600'
    '&family=Gaegu:wght@400;700&family=Young+Serif&display=swap">'
)

SAT, LIFT = 0.8, 0.08
PIGMENTS = [(13, 0.56), (40, 0.74), (86, 0.34), (150, 0.28), (208, 0.3), (345, 0.42)]


def soften(m):
    r, g, b = (int(m.group(0)[i:i + 2], 16) / 255 for i in (1, 3, 5))
    h, l, s = colorsys.rgb_to_hls(r, g, b)
    if l > 0.3:
        l += (0.8 - l) * LIFT
    if 0.2 < l < 0.58:
        l += (0.62 - l) * 0.42
    if s > 0.04:
        d, cap = min(PIGMENTS, key=lambda a: min(abs(a[0] - h * 360), 360 - abs(a[0] - h * 360)))
        e = (d - h * 360 + 180) % 360 - 180
        h = ((h * 360 + e * 0.6) % 360) / 360
        s = min(cap, s * SAT)
    return "#" + "".join(f"{round(v * 255):02x}" for v in colorsys.hls_to_rgb(h, l, s))


def chart_js(s):
    jars = []
    for arm, x0 in (("off", 70), ("on", 318)):
        a = s["arms"][arm]
        for i, (score, fp, tok) in enumerate(zip(a["score"], a["first_pass"], a["median_tokens_passing"])):
            jars.append({"arm": arm, "x": x0 + i * 66, "r": i + 1, "score": score,
                         "fp": fp, "tok": tok, "n": s["n_tasks"]})
    data = json.dumps(jars, ensure_ascii=True, sort_keys=True)
    means = json.dumps({k: s["arms"][k]["score_mean"] for k in ("off", "on")}, sort_keys=True)
    return f"""
const JARS = {data}, MEANS = {means};
card({{
  id: 'e5pulse',
  name: 'Pulse on vs off',
  of: 'first-pass score per rerun, synthetic replies, to scale',
  note: 'One jar per rerun of the 12 frozen tasks; fill = first-pass score, 0 to 100. Synthetic: scripts/e5/synthetic_replies.py sets the gap with its P_WRONG rates. Not measured.',
  draw() {{
    const Y0 = 338, K = 2.3, Y = v => Y0 - 4 - v * K;
    PT('the shelf, with 0, 50 and 100 marked', () => {{
      const shelf = [[16, Y0], [504, Y0], [504, Y0 + 14], [16, Y0 + 14]];
      wash(shelf, '#c9a77a');
      lid(shelf, 1.1);
      [0, 50, 100].forEach(v => {{
        hair([36, Y(v)], [500, Y(v)]);
        TX(32, Y(v) + 5, String(v), {{anchor: 'end', size: 13.5, col: LN}});
      }});
      TX(136, Y0 + 40, 'pulse off', {{anchor: 'middle', size: 15, cap: true}});
      TX(384, Y0 + 40, 'pulse on', {{anchor: 'middle', size: 15, cap: true}});
      TX(136, Y0 + 60, 'mean ' + MEANS.off, {{anchor: 'middle', size: 14, col: LN}});
      TX(384, Y0 + 60, 'mean ' + MEANS.on, {{anchor: 'middle', size: 14, col: LN}});
    }});
    JARS.forEach(j => {{
      const cx = j.x, top = Y(j.score);
      const note = 'Pulse ' + j.arm + ', rerun ' + j.r + ': ' + j.fp + ' of ' + j.n + ' first-pass, score ' + j.score + ', median ' + j.tok + ' tokens per passing task. Synthetic.';
      PT(note, () => {{
        const glass = rrect(cx - 24, Y(100) - 14, 48, Y0 - Y(100) + 14, 9);
        wash(glass, '#e4eef0');
        const fill = [[cx - 20, Y0 - 4], [cx + 20, Y0 - 4], [cx + 20, top], [cx - 20, top]];
        wash(fill, j.arm === 'on' ? '#9fb3c8' : '#b8b2a7');
        lid(fill, 0.8);
        lid(glass, 1.2);
        const cap = rrect(cx - 27, Y(100) - 24, 54, 12, 4);
        wash(cap, '#b39a74');
        lid(cap, 1.0);
        TX(cx, top - 8, String(j.score), {{anchor: 'middle', size: 16}});
        TX(cx, Y0 + 26, 'r' + j.r, {{anchor: 'middle', size: 13.5, col: LN}});
      }}, {{lift: [0, -6]}});
    }});
  }}
}});
CARDS.forEach(c => c.sec = 'chart');
"""


def build():
    s = json.loads((RUN / "summary.json").read_text(encoding="utf-8"))
    user = re.sub(r"#[0-9a-fA-F]{6}\b", soften, chart_js(s))
    js = ("const LIT = {}, ALT = {};\n" + (VENDOR / "engine.js").read_text(encoding="utf-8")
          + "\n" + user + "\n" + (VENDOR / "run.js").read_text(encoding="utf-8"))
    page = (VENDOR / "page.html").read_text(encoding="utf-8")
    if FONT_LINKS not in page:
        raise SystemExit("page.html font links changed; update build_chart.py")
    page = page.replace(FONT_LINKS, '<link rel="stylesheet" href="fonts/fonts.css">')
    sub = ("<h2>E5 treatment run on the E1 evalbench</h2>"
           "<p>Synthetic replies. Not a model result and not a CV number.</p>")
    foot = ("Numbers from summary.json: %d frozen tasks, %d reruns per arm, dataset hash %s. "
            "Replies are written by scripts/e5/synthetic_replies.py. Fonts bundled (SIL OFL). "
            "Minicharts is used under the MIT licence; see vendor/minicharts/LICENSE and NOTICE.md."
            % (s["n_tasks"], s["reruns"], s["dataset_hash"][:12]))
    page = (page.replace("@@TITLE@@", "E5: pulse on vs off")
            .replace("@@SUB@@", sub)
            .replace("@@FOOT@@", H.escape(foot))
            .replace("@@JS@@", js))
    if "fonts.googleapis.com" in page or "fonts.gstatic.com" in page:
        raise SystemExit("a font host link is left in the page")
    return "<!doctype html>\n" + page


def main(argv):
    out = CHART / "index.html"
    page = build()
    if argv[1:] == ["--check"]:
        same = out.is_file() and out.read_text(encoding="utf-8") == page
        if not same:
            print("differs:", out.relative_to(ROOT))
        return 0 if same else 1
    if argv[1:]:
        print(__doc__, file=sys.stderr)
        return 2
    out.write_text(page, encoding="utf-8", newline="\n")
    print("wrote", out.relative_to(ROOT), len(page), "bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
