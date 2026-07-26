// pr-takeover-trim — critical PR review + necessity trim for takeover handoff.
// Read-only. Produces a scratch report; does not edit code or post to GitHub.
//
// Run:
//   /workflow pr-takeover-trim {"pr":"2","branch":"issue-1/hitl-hootl-runtime-core","base":"master"}
// Defaults match ensembly PR #2 if args omitted partially.

let meta = #{
    name: "pr-takeover-trim",
    description: "Take over an abandoned agent PR: law-lock, parallel critical lenses, adversarial verify cuts, production-grade trim report",
    when_to_use: "Abandoned or agent-authored PR needs critical review, necessity cuts, and a takeover decision packet (no auto-fix)",
    phases: [
        #{ title: "LawLock", detail: "inventory PR files + formal invariants + seeded review findings" },
        #{ title: "Lenses", detail: "parallel: invariants, necessity, correctness, tests, dogfood" },
        #{ title: "Verify", detail: "adversarial skeptics on cut/fix claims" },
        #{ title: "Synthesize", detail: "one takeover + trim report" },
    ],
};

// ── Schemas ──────────────────────────────────────────────────────────────

let inventory_schema = #{
    "type": "object",
    "required": ["pr_files", "invariants", "seeded_findings", "module_map", "notes"],
    "properties": #{
        "pr_files": #{
            "type": "array",
            "maxItems": 40,
            "items": #{ "type": "string" },
        },
        "invariants": #{
            "type": "array",
            "maxItems": 12,
            "items": #{ "type": "string" },
        },
        "seeded_findings": #{
            "type": "array",
            "maxItems": 12,
            "items": #{
                "type": "object",
                "required": ["id", "severity", "file", "issue"],
                "properties": #{
                    "id": #{ "type": "string" },
                    "severity": #{ "type": "string" },
                    "file": #{ "type": "string" },
                    "issue": #{ "type": "string" },
                },
            },
        },
        "module_map": #{
            "type": "array",
            "maxItems": 20,
            "items": #{
                "type": "object",
                "required": ["file", "formal_object", "role"],
                "properties": #{
                    "file": #{ "type": "string" },
                    "formal_object": #{ "type": "string" },
                    "role": #{ "type": "string" },
                },
            },
        },
        "notes": #{ "type": "string" },
    },
};

let findings_schema = #{
    "type": "object",
    "required": ["findings"],
    "properties": #{
        "findings": #{
            "type": "array",
            "maxItems": 8,
            "items": #{
                "type": "object",
                "required": ["file", "issue", "tier", "severity", "why"],
                "properties": #{
                    "file": #{ "type": "string" },
                    "issue": #{ "type": "string" },
                    "tier": #{ "type": "string" },
                    "severity": #{ "type": "string" },
                    "why": #{ "type": "string" },
                },
            },
        },
    },
};

let verdict_schema = #{
    "type": "object",
    "required": ["real", "reason", "evidence", "tier_after"],
    "properties": #{
        "real": #{ "type": "boolean" },
        "reason": #{ "type": "string" },
        "evidence": #{ "type": "string" },
        "tier_after": #{ "type": "string" },
    },
};

let report_schema = #{
    "type": "object",
    "required": ["title", "executive", "confirmed_cuts", "confirmed_fixes", "must_keep", "do_not_touch", "first_steps", "merge_bar", "markdown"],
    "properties": #{
        "title": #{ "type": "string" },
        "executive": #{ "type": "string" },
        "confirmed_cuts": #{
            "type": "array",
            "maxItems": 16,
            "items": #{ "type": "string" },
        },
        "confirmed_fixes": #{
            "type": "array",
            "maxItems": 16,
            "items": #{ "type": "string" },
        },
        "must_keep": #{
            "type": "array",
            "maxItems": 16,
            "items": #{ "type": "string" },
        },
        "do_not_touch": #{
            "type": "array",
            "maxItems": 12,
            "items": #{ "type": "string" },
        },
        "first_steps": #{
            "type": "array",
            "maxItems": 8,
            "items": #{ "type": "string" },
        },
        "merge_bar": #{ "type": "string" },
        "markdown": #{ "type": "string" },
    },
};

// ── Args (defaults for ensembly PR #2) ───────────────────────────────────

let pr = "2";
let branch = "issue-1/hitl-hootl-runtime-core";
let base = "master";
let repo = "thecuriousts/ensembly";

if args != () {
    if args.pr != () { pr = args.pr; }
    if args.branch != () { branch = args.branch; }
    if args.base != () { base = args.base; }
    if args.repo != () { repo = args.repo; }
}

log("pr-takeover-trim: PR #" + pr + " " + branch + " vs " + base);

// ── Seeded review findings (operator review on PR #2; re-verified later) ─

let seed_block = "";
seed_block += "SEEDED PRIOR REVIEW (operator; re-verify against code, do not rubber-stamp):\n";
seed_block += "1. P0 critical_path.rs next_hootl_digital — after CP path scan, falls back to any open digital HOOTL by earliest start; weakens 'agents claim only via CP'. Prefer None/NoWork or slack≈0 only.\n";
seed_block += "2. P1 life_state recompute — hitl_surfaces increments every recompute while gate open; edge-trigger on regime enter only.\n";
seed_block += "3. P1 TickReport.triggers_emitted — copies messages_drained; rename or real emit counter.\n";
seed_block += "4. Auth id dual-namespace: approve uses action id (pay-rent) not auth-pay-rent from pendingAuth; document loud / accept both.\n";
seed_block += "5. Dual SoT risk: graph node status vs approvals Snapshot updated separately in apply_manual.\n";
seed_block += "6. Same-tick claim+complete is dry-run thrash clearance; separate simulate vs execute before real HOOTL.\n";
seed_block += "7. Stub/weak test smell (e.g. fingerprint_independent_hashmap_order); store save_life_state format smell.\n";

// ── Shared law pointers (cold agents must open these files) ──────────────

let law_block = "";
law_block += "LAW FILES (read with read_file before judging):\n";
law_block += "- arch-design/formal_problem_definition.AppGenMathPhyLang.md\n";
law_block += "- docs/DECISIONS.md (section Issue #1 HITL/HOOTL runtime core)\n";
law_block += "- docs/arch-design/coming-next.md (SN-0 remaining)\n";
law_block += "- AGENTS.md (production bar; refuse prototype theater)\n";
law_block += "- fixtures/issue-1-runtime.json\n";
law_block += "PR: https://github.com/" + repo + "/pull/" + pr + "\n";
law_block += "BRANCH: " + branch + " BASE: " + base + "\n";
law_block += "Tools: gh pr view/diff, git diff " + base + "..." + branch + ", read_file, grep. Do NOT answer from memory.\n";

// ── NECESSITY POLICY (operator choice: cut-until-essential + production-grade) ─

let necessity_policy = "";
necessity_policy += "NECESSITY POLICY — Cut until it cannot be missed, AND remain production-grade:\n";
necessity_policy += "- Default bias: CUT or DEFER. Keep only what is load-bearing for formal iron-peak dogfood:\n";
necessity_policy += "  S + G + CP prioritization + HITL wait-only-for-permission + HOOTL claim-via-CP + typed MsgBus + durable T1 + real CLI runtime * path.\n";
necessity_policy += "- Production-grade: do NOT recommend keeping prototype theater, fake metrics, stub tests as product surface, or half-wired APIs.\n";
necessity_policy += "- Production-grade also: do NOT gut so hard that remaining code is a toy. If a surface stays, it must be honest, tested, and dogfoodable.\n";
necessity_policy += "- Prefer cut-or-simplify over keep-and-polish for anything not required for load→tick→approve→claim→complete truth.\n";
necessity_policy += "- Formal listed PERT/MonteCarlo as Must: keep a real uncertainty measure if dogfood uses it; cut sample bloat, unused fields, or dead API if not load-bearing.\n";
necessity_policy += "- Explicitly do-not-expand: game layer, Eve remote bridge, multi-agent conflict at scale, RL/adaptive policies, continuous dashboards (SN-0).\n";
necessity_policy += "Tiers for each finding: must_keep | keep_but_fix | defer | cut_or_simplify | rewrite_later.\n";
necessity_policy += "Severity: P0 | P1 | P2 | note.\n";

// ══════════════════════════════════════════════════════════════════════════
phase("LawLock");
// ══════════════════════════════════════════════════════════════════════════

let map_prompt = "";
map_prompt += "You are the law-lock mapper for an abandoned agent PR takeover.\n";
map_prompt += law_block;
map_prompt += seed_block;
map_prompt += "TASK:\n";
map_prompt += "1) List every file changed on this PR/branch vs base (gh or git). pr_files must be exact paths.\n";
map_prompt += "2) Read formal law + DECISIONS Issue #1; extract up to 12 concrete invariants as short strings.\n";
map_prompt += "3) Re-state seeded findings with severity and file (rephrase from seed; ids S1..S7).\n";
map_prompt += "4) module_map: map each major new/changed Rust module to formal_object (S|G|CP|P|MsgBus|Trigger|Agent|HITL|CLI|Docs|Other) and one-line role.\n";
map_prompt += "5) notes: one paragraph on how the agent-authored work was structured (commits, bulk modules, docs vs code).\n";
map_prompt += "Empty arrays only after you actually ran git/gh and opened law files.\n";

let inv = agent(map_prompt, #{
    label: "law-lock-map",
    capability_mode: "read-only",
    output_schema: inventory_schema,
});

// Hard fail only when the agent slot itself dies. Partial/canned output falls
// back to known PR #2 defaults so later phases still run (and smoke can walk them).
if inv == () || !inv.success {
    complete(#{
        summary: "LawLock agent failed — cannot proceed without inventory",
        error: true,
    });
}

let pr_files = inv.output.pr_files;
let invariants = inv.output.invariants;
let seeded_findings = inv.output.seeded_findings;
let module_map = inv.output.module_map;
let inv_notes_raw = inv.output.notes;

if pr_files == () || pr_files.len() == 0 {
    log("LawLock: pr_files empty — using PR #2 default file list");
    pr_files = [
        "crates/peram-kernel/src/critical_path.rs",
        "crates/peram-kernel/src/runtime.rs",
        "crates/peram-kernel/src/graph.rs",
        "crates/peram-kernel/src/agent.rs",
        "crates/peram-kernel/src/msg_bus.rs",
        "crates/peram-kernel/src/trigger.rs",
        "crates/peram-kernel/src/life_state.rs",
        "crates/peram-kernel/src/main.rs",
        "crates/peram-kernel/src/store.rs",
        "crates/peram-kernel/src/lib.rs",
        "fixtures/issue-1-runtime.json",
        "docs/DECISIONS.md",
        "docs/MAP.md",
        "docs/PLAYBOOK.md",
        "README.md",
        "AGENTS.md",
        "arch-design/formal_problem_definition.AppGenMathPhyLang.md",
    ];
}
if invariants == () || invariants.len() == 0 {
    log("LawLock: invariants empty — using formal iron-peak defaults");
    invariants = [
        "Agents claim digital HOOTL work only via critical path / CP engine",
        "Human waits only for permission (AuthGate + PhysicalBeacon)",
        "Runtime owns global truth S+G; agents do not own prioritization",
        "Game layer excluded from Issue #1 core",
        "Typed MsgBus only — no free-form agent chatter as coordination",
        "Durable life_state in T1 SQLite; dogfood via runtime CLI",
    ];
}
if seeded_findings == () || seeded_findings.len() == 0 {
    seeded_findings = [
        #{ id: "S1", severity: "P0", file: "crates/peram-kernel/src/critical_path.rs", issue: "next_hootl_digital falls back off CP" },
        #{ id: "S2", severity: "P1", file: "crates/peram-kernel/src/life_state.rs", issue: "hitl_surfaces inflates every recompute" },
        #{ id: "S3", severity: "P1", file: "crates/peram-kernel/src/runtime.rs", issue: "triggers_emitted mislabeled as messages_drained" },
    ];
}
if module_map == () { module_map = []; }
if inv_notes_raw == () { inv_notes_raw = ""; }

let pr_files_json = json_encode(pr_files);
let invariants_json = json_encode(invariants);
let seeded_json = json_encode(seeded_findings);
log("LawLock ok: " + pr_files.len().to_string() + " files, "
    + invariants.len().to_string() + " invariants");

// ══════════════════════════════════════════════════════════════════════════
phase("Lenses");
// ══════════════════════════════════════════════════════════════════════════

let shared_tail = "";
shared_tail += law_block;
shared_tail += necessity_policy;
shared_tail += "PR_FILES (scope — only cite these or files they import that define the behavior):\n";
shared_tail += pr_files_json + "\n";
shared_tail += "INVARIANTS:\n" + invariants_json + "\n";
shared_tail += "SEEDED:\n" + seeded_json + "\n";
shared_tail += "OUTPUT: at most 8 findings. Each: file, issue (concrete), tier, severity, why (1-2 sentences).\n";
shared_tail += "An empty findings array is valid ONLY after reading the relevant source files with tools.\n";

let jobs = [];

// 1) Invariant fidelity
let p1 = "";
p1 += "LENS: Invariant fidelity. Does shipped code enforce formal HITL/HOOTL law?\n";
p1 += "Focus: next_hootl_digital / claim-via-CP, AuthGate+PhysicalBeacon wait-only-for-permission, agents refuse Auth/Physical, MsgBus typed ManualCmd|Trigger|AgentReport, no free-form chatter as coordination.\n";
p1 += "Re-check seeded P0 and related. Prefer keep_but_fix or cut_or_simplify over vague notes.\n";
p1 += shared_tail;
jobs.push(#{
    prompt: p1,
    label: "lens:invariants",
    capability_mode: "read-only",
    output_schema: findings_schema,
});

// 2) Necessity / bloat
let p2 = "";
p2 += "LENS: Necessity and bloat. Cut until it cannot be missed; production-grade remainder only.\n";
p2 += "For each bulk surface (critical_path MonteCarlo, msg_bus, trigger set, CLI surface, docs sprawl, extra fields/metrics): decide cut_or_simplify vs must_keep with evidence.\n";
p2 += "Flag unused public APIs, duplicate counters, over-documented promises, sample-heavy paths not used by dogfood.\n";
p2 += "Do not recommend deleting S/G/CP/HITL core; do recommend stripping non-load-bearing mass.\n";
p2 += shared_tail;
jobs.push(#{
    prompt: p2,
    label: "lens:necessity",
    capability_mode: "read-only",
    output_schema: findings_schema,
});

// 3) Correctness / dual SoT
let p3 = "";
p3 += "LENS: Correctness and dual sources of truth.\n";
p3 += "Hunt: graph status vs approvals snapshot desync, metric inflation (hitl_surfaces), mislabeled JSON fields, auth id dual-namespace footguns, silent wrong regime after approve/deny/claim/complete.\n";
p3 += "Re-check seeded P1s. tier keep_but_fix for real bugs; cut_or_simplify for lying fields that should die.\n";
p3 += shared_tail;
jobs.push(#{
    prompt: p3,
    label: "lens:correctness",
    capability_mode: "read-only",
    output_schema: findings_schema,
});

// 4) Test truth
let p4 = "";
p4 += "LENS: Test truth vs theater.\n";
p4 += "Read #[cfg(test)] modules in changed files. Flag stub tests, assertions that cannot fail meaningfully, missing coverage for claim-via-CP, approve id, tick HOOTL path, fixture load.\n";
p4 += "Production bar: tests must exercise shipped dogfood path. Prefer cut_or_simplify for worthless tests; keep_but_fix for missing real tests.\n";
p4 += shared_tail;
jobs.push(#{
    prompt: p4,
    label: "lens:tests",
    capability_mode: "read-only",
    output_schema: findings_schema,
});

// 5) Dogfood / operator path
let p5 = "";
p5 += "LENS: Dogfood and operator truth.\n";
p5 += "Trace CLI runtime load|status|tick|approve|deny|claim|complete and fixture fixtures/issue-1-runtime.json against docs (README, MAP, PLAYBOOK).\n";
p5 += "Flag docs that overclaim, JSON+RUNTIME_OK parse traps, approve-id confusion, anything that makes cold dogfood fail or feel like prototype theater.\n";
p5 += shared_tail;
jobs.push(#{
    prompt: p5,
    label: "lens:dogfood",
    capability_mode: "read-only",
    output_schema: findings_schema,
});

let lens_results = parallel(jobs);

let all_findings = [];
let lens_i = 0;
for r in lens_results {
    if r != () && r.success && r.output.findings != () {
        for f in r.output.findings {
            // scope guard: drop findings whose file is empty
            if f.file != () && f.file != "" && f.issue != () && f.issue != "" {
                all_findings.push(f);
            }
        }
    } else {
        log("lens slot " + lens_i.to_string() + " failed or empty");
    }
    lens_i += 1;
}

log("Lenses produced " + all_findings.len().to_string() + " raw findings");

// Prefer verifying cut/fix tiers; always try to include seeded-related keep_but_fix
let to_verify = [];
for f in all_findings {
    let tier = f.tier;
    if tier == "cut_or_simplify" || tier == "keep_but_fix" || tier == "rewrite_later" {
        if to_verify.len() < 12 {
            to_verify.push(f);
        }
    }
}
// If under-filled, add remaining findings up to 12
if to_verify.len() < 12 {
    for f in all_findings {
        if to_verify.len() >= 12 { break; }
        let already = false;
        for t in to_verify {
            if t.file == f.file && t.issue == f.issue { already = true; }
        }
        if !already {
            to_verify.push(f);
        }
    }
}

if to_verify.len() == 0 {
    log("No findings to verify — synthesizing empty-trim report");
}

// ══════════════════════════════════════════════════════════════════════════
phase("Verify");
// ══════════════════════════════════════════════════════════════════════════

let confirmed = [];
let rejected = [];

if to_verify.len() > 0 {
    let vjobs = [];
    let vi = 0;
    for f in to_verify {
        let vp = "";
        vp += "Adversarially VERIFY this PR-takeover finding. Default real=false unless you independently read the code and confirm.\n";
        vp += "FILE: " + f.file + "\n";
        vp += "CLAIMED ISSUE: " + f.issue + "\n";
        vp += "CLAIMED TIER: " + f.tier + " SEVERITY: " + f.severity + "\n";
        vp += "CLAIMED WHY: " + f.why + "\n";
        vp += law_block;
        vp += necessity_policy;
        vp += "Rules:\n";
        vp += "- Use read_file/grep on the actual file. Quote symbols/behavior in evidence.\n";
        vp += "- real=true only with concrete evidence. If the claim is taste without load-bearing impact, real=false.\n";
        vp += "- If real, set tier_after to the correct tier under cut-until-essential + production-grade policy.\n";
        vp += "- If the claim overstates, real=false and explain.\n";
        vjobs.push(#{
            prompt: vp,
            label: "verify:" + vi.to_string(),
            capability_mode: "read-only",
            output_schema: verdict_schema,
        });
        vi += 1;
    }

    let verdicts = parallel(vjobs);
    let i = 0;
    for v in verdicts {
        if i >= to_verify.len() { break; }
        let f = to_verify[i];
        if v != () && v.success && v.output.real == true
            && v.output.evidence != () && v.output.evidence != "" {
            confirmed.push(#{
                file: f.file,
                issue: f.issue,
                tier: v.output.tier_after,
                severity: f.severity,
                why: f.why,
                evidence: v.output.evidence,
                reason: v.output.reason,
            });
        } else {
            let why_reject = "no usable verification";
            if v != () && v.success && v.output.reason != () {
                why_reject = v.output.reason;
            }
            rejected.push(#{
                file: f.file,
                issue: f.issue,
                why_reject: why_reject,
            });
        }
        i += 1;
    }
}

log(confirmed.len().to_string() + "/" + to_verify.len().to_string() + " findings survived verification");

// ══════════════════════════════════════════════════════════════════════════
phase("Synthesize");
// ══════════════════════════════════════════════════════════════════════════

let conf_json = json_encode(confirmed);
let rej_json = json_encode(rejected);
let inv_notes = inv_notes_raw;

let syn = "";
syn += "You synthesize a TAKEOVER + TRIM report for PR #" + pr + " on branch " + branch + ".\n";
syn += "Author (Cursor/Grok agent) is no longer working this; operator is taking over. Read-only report only.\n";
syn += law_block;
syn += necessity_policy;
syn += "INVENTORY NOTES:\n" + inv_notes + "\n";
syn += "MODULE_MAP:\n" + json_encode(module_map) + "\n";
syn += "CONFIRMED FINDINGS (survived adversarial verify — treat as ground for decisions):\n" + conf_json + "\n";
syn += "REJECTED / UNVERIFIED (do not treat as confirmed cuts):\n" + rej_json + "\n";
syn += "TASK: Produce structured fields AND a full markdown report in `markdown` with these sections:\n";
syn += "1. Executive takeover brief\n";
syn += "2. Confirmed cuts / simplify (cut_or_simplify)\n";
syn += "3. Confirmed fixes before merge (keep_but_fix, ordered P0→P2)\n";
syn += "4. Must keep (iron-peak)\n";
syn += "5. Do not touch / do not expand (SN-0)\n";
syn += "6. First 3–5 steps for the operator after this report\n";
syn += "7. Merge bar checklist (production-grade, dogfoodable)\n";
syn += "Be ruthless on cuts but never recommend prototype theater. Prefer delete-or-honest over half-wired keep.\n";
syn += "If a confirmed finding is cut_or_simplify, put it in confirmed_cuts; if keep_but_fix, confirmed_fixes.\n";

let report = agent(syn, #{
    label: "synthesize-takeover-report",
    capability_mode: "read-only",
    output_schema: report_schema,
});

if report == () || !report.success || report.output.markdown == () {
    // Fallback: dump confirmed findings without pretty markdown
    let fallback = "# Takeover trim (partial)\n\nSynthesis agent failed. Confirmed findings JSON:\n\n";
    fallback += conf_json;
    let path = write_scratch_file("takeover-trim-report.md", fallback);
    complete(#{
        summary: "Partial: synthesis failed; raw confirmed findings written",
        path: path,
        confirmed_count: confirmed.len(),
        rejected_count: rejected.len(),
    });
}

let md = report.output.markdown;
let path = write_scratch_file("takeover-trim-report.md", md);
log("Report written: " + path);

complete(#{
    summary: report.output.executive,
    path: path,
    title: report.output.title,
    confirmed_cuts: report.output.confirmed_cuts,
    confirmed_fixes: report.output.confirmed_fixes,
    must_keep: report.output.must_keep,
    do_not_touch: report.output.do_not_touch,
    first_steps: report.output.first_steps,
    merge_bar: report.output.merge_bar,
    confirmed_count: confirmed.len(),
    rejected_count: rejected.len(),
    verified_of: to_verify.len(),
});
