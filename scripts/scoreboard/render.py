#!/usr/bin/env python3
from metrics import display_repo, fmt_day


THEME = (
    '%%{init: {"theme": "base", "themeVariables": {'
    '"primaryColor": "#1e3a5f", "primaryBorderColor": "#93c5fd", '
    '"primaryTextColor": "#ffffff", "secondaryColor": "#1e3a5f", '
    '"tertiaryColor": "#1e3a5f", "lineColor": "#93c5fd", '
    '"fontFamily": "Inter"}}}%%'
)
DARK = "fill:#1e3a5f,stroke:#93c5fd,color:#ffffff"


def _esc(text):
    return str(text).replace('"', "#quot;")


def render(result):
    total = result["total"]
    since = fmt_day(result["since"])
    until = fmt_day(result["until"])
    lines = [
        "# Public engineering scoreboard",
        "",
        "```mermaid",
        THEME,
        "flowchart TB",
        f'  title["Public repo scoreboard {since} to {until} UTC"]',
        "  title --> pillars",
        "  subgraph pillars[Three measures]",
        "    direction LR",
        "    subgraph correct[Correct]",
        f'      fp["First-pass CI {_esc(total["first_pass_rate"])}"]',
        "    end",
        "    subgraph effective[Effective]",
        f'      mg["PRs merged {total["merged"]}"]',
        f'      rv["Reverts and follow-up fixes {total["reverts_and_fixes"]}"]',
        "    end",
        "    subgraph efficient[Efficient]",
        f'      md["Open to merge median {total["median"]}"]',
        f'      p9["Open to merge p90 {total["p90"]}"]',
        "    end",
        "  end",
        f"  classDef correct {DARK}",
        f"  classDef effective {DARK}",
        f"  classDef efficient {DARK}",
        f"  classDef frame {DARK}",
        "  class fp correct",
        "  class mg,rv effective",
        "  class md,p9 efficient",
        "  class title,pillars frame",
        "```",
        "",
        "## Definitions",
        "",
        "Window timestamps are UTC. `--since` and `--until` are inclusive calendar dates: a timestamp counts when `since 00:00 UTC <= t < until+1 day 00:00 UTC`. The default window is today UTC and the date 30 days earlier.",
        "",
        "Owners are `p10ns11y` and `thecuriousts`. A repository is included only when `visibility` is public and `private` is false. That check runs before any name is kept. Repositories that fail it are dropped and never written down.",
        "",
        "- **PRs merged:** pull requests whose `merged_at` falls in the window.",
        "- **CI first-pass rate:** share of those merged pull requests whose first CI attempt on the first head commit was all green. The first head commit is the earliest pull-request commit that has at least one check run. The first attempt is the earliest `started_at` for each check name on that commit. Those conclusions count as green when they are `success`, `skipped`, or `neutral`. Any other conclusion fails the first pass. A merged pull request with no check run on any commit is omitted from the numerator and the denominator.",
        "- **Reverts and follow-up fixes:** default-branch commits in the window whose message starts with `Revert`, `revert:`, or `This reverts commit`, plus merged pull requests titled or labelled `fix` that cite an earlier pull request (`#N` or a github.com pull URL) whose `merged_at` is earlier by at most 7 days.",
        "- **Open-to-merge time:** median and 90th percentile of `merged_at - created_at` for merged pull requests in the window. Percentiles use linear interpolation at `q * (n - 1)`.",
        "",
        "## Results",
        "",
        "| Repository | PRs merged | First-pass CI | Reverts and follow-up fixes | Open to merge median | Open to merge p90 |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for row in result["rows"]:
        lines.append(
            "| {repo} | {merged} | {rate} | {fixes} | {median} | {p90} |".format(
                repo=display_repo(row["repo"]),
                merged=row["merged"],
                rate=row["first_pass_rate"],
                fixes=row["reverts_and_fixes"],
                median=row["median"],
                p90=row["p90"],
            )
        )
    lines.append(
        "| **Total** | {merged} | {rate} | {fixes} | {median} | {p90} |".format(
            merged=total["merged"],
            rate=total["first_pass_rate"],
            fixes=total["reverts_and_fixes"],
            median=total["median"],
            p90=total["p90"],
        )
    )
    scanned = result["public_repo_count"]
    shown = result["active_repo_count"]
    fixes = total["reverts_and_fixes"]
    fix_phrase = "1 revert and follow-up fix" if fixes == 1 else f"{fixes} reverts and follow-up fixes"
    lines.extend([
        "",
        f"Scanned {scanned} public repositories; {shown} had a merged pull request or a revert or follow-up fix in the window and appear above.",
        "",
        f"Over {since} to {until} UTC the public record shows {total['merged']} merged pull requests. First-pass CI was {total['first_pass_rate']}. Open-to-merge time was {total['median']} at the median and {total['p90']} at p90, with {fix_phrase}.",
        "",
        "Rebuild with `python3 scripts/scoreboard/scoreboard --since "
        f"{result['since'].isoformat()} --until {result['until'].isoformat()} "
        "--output docs/scoreboard.md`.",
        "",
    ])
    return "\n".join(lines)
