// pr-finish-scope — close Issue #1 / PR open review items cleanly, no SN-0 creep.
//
// Purpose: finish remaining in-scope work to merge readiness.
// Not: re-architect, game, Eve, RL, multi-agent scale, dashboards.
//
// Run:
//   /workflow pr-finish-scope {"pr":"2","branch":"issue-1/hitl-hootl-runtime-core","base":"master"}
// Optional: {"implement":false} for plan-only (default implement:true)

let meta = #{
    name: "pr-finish-scope",
    description: "Finish open PR review items in-scope: confirm gaps, implement minimal fixes, verify dogfood, merge-bar report — no scope creep",
    when_to_use: "After a harden pass; PR has remaining review comments; close Issue #1 slice cleanly without SN-0 expansion",
    phases: [
        #{ title: "ScopeLock", detail: "law + open review seeds + refuse SN-0" },
        #{ title: "Confirm", detail: "re-verify each open item against HEAD" },
        #{ title: "Plan", detail: "ordered minimal fix list" },
        #{ title: "Implement", detail: "apply in-scope fixes only" },
        #{ title: "Verify", detail: "cargo test + runtime dogfood" },
        #{ title: "Report", detail: "merge-bar status" },
    ],
};

// ── Schemas ──────────────────────────────────────────────────────────────

let open_item_schema = #{
    "type": "object",
    "required": ["items"],
    "properties": #{
        "items": #{
            "type": "array",
            "maxItems": 12,
            "items": #{
                "type": "object",
                "required": ["id", "severity", "file", "issue", "still_open", "evidence", "in_scope"],
                "properties": #{
                    "id": #{ "type": "string" },
                    "severity": #{ "type": "string" },
                    "file": #{ "type": "string" },
                    "issue": #{ "type": "string" },
                    "still_open": #{ "type": "boolean" },
                    "evidence": #{ "type": "string" },
                    "in_scope": #{ "type": "boolean" },
                },
            },
        },
    },
};

let plan_schema = #{
    "type": "object",
    "required": ["steps", "out_of_scope", "notes"],
    "properties": #{
        "steps": #{
            "type": "array",
            "maxItems": 12,
            "items": #{
                "type": "object",
                "required": ["id", "action", "file", "done_when"],
                "properties": #{
                    "id": #{ "type": "string" },
                    "action": #{ "type": "string" },
                    "file": #{ "type": "string" },
                    "done_when": #{ "type": "string" },
                },
            },
        },
        "out_of_scope": #{
            "type": "array",
            "maxItems": 12,
            "items": #{ "type": "string" },
        },
        "notes": #{ "type": "string" },
    },
};

let implement_schema = #{
    "type": "object",
    "required": ["done", "changed_files", "skipped", "notes"],
    "properties": #{
        "done": #{ "type": "boolean" },
        "changed_files": #{
            "type": "array",
            "maxItems": 20,
            "items": #{ "type": "string" },
        },
        "skipped": #{
            "type": "array",
            "maxItems": 12,
            "items": #{ "type": "string" },
        },
        "notes": #{ "type": "string" },
    },
};

let verify_schema = #{
    "type": "object",
    "required": ["tests_ok", "dogfood_ok", "log"],
    "properties": #{
        "tests_ok": #{ "type": "boolean" },
        "dogfood_ok": #{ "type": "boolean" },
        "log": #{ "type": "string" },
    },
};

let report_schema = #{
    "type": "object",
    "required": ["title", "executive", "resolved", "remaining", "merge_ready", "markdown"],
    "properties": #{
        "title": #{ "type": "string" },
        "executive": #{ "type": "string" },
        "resolved": #{
            "type": "array",
            "maxItems": 16,
            "items": #{ "type": "string" },
        },
        "remaining": #{
            "type": "array",
            "maxItems": 12,
            "items": #{ "type": "string" },
        },
        "merge_ready": #{ "type": "boolean" },
        "markdown": #{ "type": "string" },
    },
};

// ── Args ─────────────────────────────────────────────────────────────────

let pr = "2";
let branch = "issue-1/hitl-hootl-runtime-core";
let base = "master";
let repo = "thecuriousts/ensembly";
let do_implement = true;

if args != () {
    if args.pr != () { pr = args.pr; }
    if args.branch != () { branch = args.branch; }
    if args.base != () { base = args.base; }
    if args.repo != () { repo = args.repo; }
    if args.implement != () { do_implement = args.implement; }
}

log("pr-finish-scope: PR #" + pr + " implement=" + do_implement.to_string());

// ── Scope law (anti-creep) ───────────────────────────────────────────────

let refuse_block = "";
refuse_block += "REFUSE (out of scope for this finish pass — do not implement):\n";
refuse_block += "- Game layer / peram-core world / multiplayer / geo sim expansion\n";
refuse_block += "- Eve remote HITL bridge\n";
refuse_block += "- Multi-agent conflict at scale / RL / adaptive policies\n";
refuse_block += "- Continuous C/E/E dashboards\n";
refuse_block += "- Legacy Node swarm product features\n";
refuse_block += "- Kernel rewrite thrash, new frameworks\n";
refuse_block += "- Persist trigger prev_* to DB (document-only is enough for Issue #1)\n";
refuse_block += "- Making Auth/Physical next_* CP-only (intentional asymmetry — document only)\n";

let scope_block = "";
scope_block += "IN SCOPE (Issue #1 iron-peak close-out only):\n";
scope_block += "- Dual-SoT honesty on CLI: turn persist + top-level approve/claim/complete vs Runtime\n";
scope_block += "- Agent claim CP law for direct claim()\n";
scope_block += "- Doc comments for Auth/Physical off-CP fallback asymmetry\n";
scope_block += "- rustfmt store save_life_state indent\n";
scope_block += "- Document process-local trigger edge state\n";
scope_block += "- Tests for any code fix; cargo test -p peram-kernel green\n";
scope_block += "- Dogfood: runtime load → tick×N → approve pay-rent → claim/complete grocery\n";

// Seeded from post-harden PR review (2026-07-26) — re-verify, do not invent new epics
let seed_block = "";
seed_block += "SEEDED OPEN ITEMS from PR #2 post-harden review (confirm against HEAD):\n";
seed_block += "O1 P1 main.rs turn cold-path: two independent let _ = save_life_state + save_snapshot — use save_runtime_pair and fail loud.\n";
seed_block += "O2 P1 main.rs top-level Approve/Deny/Claim/Complete mutate Snapshot only — after runtime load, desyncs graph Auth. When life_state exists: route through Runtime (MsgBus + save_runtime_pair) OR refuse with loud error pointing to runtime approve|claim|complete.\n";
seed_block += "O3 P2 agent.rs AgentWorker::claim does not require on_critical_path — claim_next is safe; direct claim can bypass. Require CP membership or restrict claim.\n";
seed_block += "O4 P2 critical_path next_auth_gate / next_physical_beacon off-CP fallback — intentional never-hide-permission; document only, do not force CP-only.\n";
seed_block += "O5 P2 store.rs save_life_state column-0 indent; runtime prev_* trigger edge state process-local — rustfmt + one-line doc (do not build Eve persistence).\n";
seed_block += "RESOLVED already (do not re-fix): next_hootl CP-only, hitl_surfaces edge, triggers_emitted real, approve action_id, claim XOR complete per tick, save_runtime_pair on runtime path.\n";

let law_block = "";
law_block += "LAW: arch-design/formal_problem_definition.AppGenMathPhyLang.md · docs/DECISIONS.md Issue #1 · AGENTS.md · SN-0 deferred list.\n";
law_block += "PR https://github.com/" + repo + "/pull/" + pr + " branch " + branch + " base " + base + "\n";
law_block += "Tools: gh, git, read_file, grep, search_replace/write, cargo, run_terminal_command. Read code before answering.\n";

// ══════════════════════════════════════════════════════════════════════════
phase("ScopeLock");
// ══════════════════════════════════════════════════════════════════════════

let lock_prompt = "";
lock_prompt += "You are the scope lock for finishing PR #" + pr + " Issue #1 HITL/HOOTL runtime.\n";
lock_prompt += law_block;
lock_prompt += scope_block;
lock_prompt += refuse_block;
lock_prompt += seed_block;
lock_prompt += "TASK: Using gh pr view/comments and git show HEAD, produce a short notes field: confirm HEAD is on the harden commit path, list refuse rules you will enforce. ";
lock_prompt += "Return items[] empty array (Confirm phase owns items). still_open false for all. This phase only returns structured ack.\n";
lock_prompt += "Output items with one placeholder id LOCK severity note file AGENTS.md issue scope-locked still_open false evidence ok in_scope true.\n";

let lock = agent(lock_prompt, #{
    label: "scope-lock",
    capability_mode: "read-only",
    output_schema: open_item_schema,
});
if lock == () || !lock.success {
    log("ScopeLock agent failed — continuing with seeds");
} else {
    log("ScopeLock ok");
}

// ══════════════════════════════════════════════════════════════════════════
phase("Confirm");
// ══════════════════════════════════════════════════════════════════════════

// Parallel confirm each seeded open item against code
let seeds = [
    #{ id: "O1", severity: "P1", file: "crates/peram-kernel/src/main.rs",
       issue: "turn cold-path uses two independent let _ = saves instead of save_runtime_pair fail-loud" },
    #{ id: "O2", severity: "P1", file: "crates/peram-kernel/src/main.rs",
       issue: "top-level approve/deny/claim/complete Snapshot-only; desync when life_state exists" },
    #{ id: "O3", severity: "P2", file: "crates/peram-kernel/src/agent.rs",
       issue: "AgentWorker::claim does not require on_critical_path" },
    #{ id: "O4", severity: "P2", file: "crates/peram-kernel/src/critical_path.rs",
       issue: "next_auth_gate/next_physical_beacon off-CP fallback needs intentional-asymmetry docs only" },
    #{ id: "O5", severity: "P2", file: "crates/peram-kernel/src/store.rs",
       issue: "save_life_state rustfmt indent; document process-local trigger prev_* on Runtime" },
];

let cjobs = [];
for s in seeds {
    let cp = "";
    cp += "Confirm whether this PR open item is STILL OPEN on current workspace HEAD.\n";
    cp += "ID: " + s.id + " SEVERITY: " + s.severity + "\n";
    cp += "FILE: " + s.file + "\n";
    cp += "ISSUE: " + s.issue + "\n";
    cp += law_block;
    cp += "Read the file with read_file/grep. still_open=true only with concrete evidence (quote code).\n";
    cp += "in_scope=true unless the fix would require SN-0 work (Eve, game, etc).\n";
    cp += "Return items array with exactly one object for this id.\n";
    cjobs.push(#{
        prompt: cp,
        label: "confirm:" + s.id,
        capability_mode: "read-only",
        output_schema: open_item_schema,
    });
}
let cresults = parallel(cjobs);

let open_items = [];
for r in cresults {
    if r != () && r.success && r.output.items != () {
        for it in r.output.items {
            if it.still_open == true && it.in_scope == true {
                open_items.push(it);
            } else {
                log("closed or out-of-scope: " + it.id + " open=" + it.still_open.to_string());
            }
        }
    }
}
log("Open in-scope items: " + open_items.len().to_string());

if open_items.len() == 0 {
    // Nothing open — still verify + report
    log("No open in-scope items — skip implement, verify + report");
}

// ══════════════════════════════════════════════════════════════════════════
phase("Plan");
// ══════════════════════════════════════════════════════════════════════════

let plan_prompt = "";
plan_prompt += "Produce an ordered minimal fix plan for PR #" + pr + " finish-scope.\n";
plan_prompt += law_block + scope_block + refuse_block;
plan_prompt += "OPEN CONFIRMED ITEMS:\n" + json_encode(open_items) + "\n";
plan_prompt += "Rules: P1 before P2. Prefer smallest honest fix. Document-only when review says document not fix.\n";
plan_prompt += "For O2 prefer: if load_life_state Some → route approve/deny/claim/complete through Runtime + save_runtime_pair; else legacy snapshot-only with loud status note.\n";
plan_prompt += "out_of_scope: list SN-0 and anything you refused.\n";
plan_prompt += "If open_items empty, steps=[] and notes=already closed.\n";

let plan_r = agent(plan_prompt, #{
    label: "finish-plan",
    capability_mode: "read-only",
    output_schema: plan_schema,
});

let steps = [];
let out_of_scope = [];
let plan_notes = "";
if plan_r != () && plan_r.success {
    if plan_r.output.steps != () { steps = plan_r.output.steps; }
    if plan_r.output.out_of_scope != () { out_of_scope = plan_r.output.out_of_scope; }
    if plan_r.output.notes != () { plan_notes = plan_r.output.notes; }
}
log("Plan steps: " + steps.len().to_string());

// ══════════════════════════════════════════════════════════════════════════
phase("Implement");
// ══════════════════════════════════════════════════════════════════════════

let impl_notes = "skipped";
let changed = [];
let skipped = [];

if do_implement && steps.len() > 0 {
    let ip = "";
    ip += "IMPLEMENT the finish-scope plan for ensembly PR #" + pr + ". You have write access.\n";
    ip += law_block + scope_block + refuse_block;
    ip += "ORDERED STEPS (do all, minimal diff):\n" + json_encode(steps) + "\n";
    ip += "OPEN ITEMS CONTEXT:\n" + json_encode(open_items) + "\n";
    ip += "Rules:\n";
    ip += "1) Edit only files needed for these steps under crates/peram-kernel and docs if law text must match.\n";
    ip += "2) Add/adjust unit tests for code behavior changes.\n";
    ip += "3) Do NOT expand SN-0. Do NOT refactor unrelated modules.\n";
    ip += "4) After edits, run: cargo test -p peram-kernel\n";
    ip += "5) Report changed_files, skipped (if any), done=true only if tests pass.\n";
    ip += "Use read_file before edit. Prefer search_replace for surgical fixes.\n";

    let impl_r = agent(ip, #{
        label: "implement-finish",
        capability_mode: "all",
        output_schema: implement_schema,
    });
    if impl_r != () && impl_r.success {
        if impl_r.output.changed_files != () { changed = impl_r.output.changed_files; }
        if impl_r.output.skipped != () { skipped = impl_r.output.skipped; }
        if impl_r.output.notes != () { impl_notes = impl_r.output.notes; }
        log("Implement done=" + impl_r.output.done.to_string() + " files=" + changed.len().to_string());
    } else {
        impl_notes = "implement agent failed";
        log(impl_notes);
    }
} else if !do_implement {
    impl_notes = "implement=false — plan only";
    skipped = ["all steps (plan-only mode)"];
} else {
    impl_notes = "no steps — nothing to implement";
}

// ══════════════════════════════════════════════════════════════════════════
phase("Verify");
// ══════════════════════════════════════════════════════════════════════════

let vp = "";
vp += "VERIFY Issue #1 finish-scope on workspace. Use execute tools.\n";
vp += "1) Run: cargo test -p peram-kernel  — set tests_ok from exit status.\n";
vp += "2) Dogfood with a temp DB:\n";
vp += "   cargo run -q -p peram-kernel -- runtime load --fixture fixtures/issue-1-runtime.json --db $TMP\n";
vp += "   four runtime tick, then runtime approve pay-rent, claim grocery-errand, complete grocery-errand, status\n";
vp += "   dogfood_ok if ends regime Hootl (or equivalent clear) without hard error.\n";
vp += "3) If O2 was fixed: briefly check top-level path does not desync when life_state present (optional smoke).\n";
vp += "Put full command output summary in log (truncate if huge).\n";

let ver = agent(vp, #{
    label: "verify-finish",
    capability_mode: "execute",
    output_schema: verify_schema,
});

let tests_ok = false;
let dogfood_ok = false;
let ver_log = "";
if ver != () && ver.success {
    tests_ok = ver.output.tests_ok;
    dogfood_ok = ver.output.dogfood_ok;
    if ver.output.log != () { ver_log = ver.output.log; }
}
log("Verify tests_ok=" + tests_ok.to_string() + " dogfood_ok=" + dogfood_ok.to_string());

// ══════════════════════════════════════════════════════════════════════════
phase("Report");
// ══════════════════════════════════════════════════════════════════════════

let rp = "";
rp += "Write the FINISH-SCOPE merge report for PR #" + pr + ".\n";
rp += law_block + scope_block + refuse_block;
rp += "PLAN NOTES: " + plan_notes + "\n";
rp += "STEPS: " + json_encode(steps) + "\n";
rp += "IMPLEMENT: " + impl_notes + " changed=" + json_encode(changed) + " skipped=" + json_encode(skipped) + "\n";
rp += "VERIFY: tests_ok=" + tests_ok.to_string() + " dogfood_ok=" + dogfood_ok.to_string() + "\n" + ver_log + "\n";
rp += "OUT OF SCOPE: " + json_encode(out_of_scope) + "\n";
rp += "merge_ready=true only if tests_ok and dogfood_ok and no open P1 left unfixed.\n";
rp += "markdown sections: Executive · What we finished · Verify · Remaining (doc-only/SN-0) · Merge bar · Operator next (commit/push if green).\n";

let rep = agent(rp, #{
    label: "finish-report",
    capability_mode: "read-only",
    output_schema: report_schema,
});

if rep == () || !rep.success || rep.output.markdown == () {
    let fallback = "# Finish-scope (partial)\n\n";
    fallback += "tests_ok=" + tests_ok.to_string() + " dogfood_ok=" + dogfood_ok.to_string() + "\n\n";
    fallback += "impl: " + impl_notes + "\n";
    fallback += "steps: " + json_encode(steps) + "\n";
    let path = write_scratch_file("finish-scope-report.md", fallback);
    complete(#{
        summary: "Partial finish report",
        path: path,
        tests_ok: tests_ok,
        dogfood_ok: dogfood_ok,
        merge_ready: false,
    });
}

let path = write_scratch_file("finish-scope-report.md", rep.output.markdown);
log("Report: " + path);

complete(#{
    summary: rep.output.executive,
    path: path,
    title: rep.output.title,
    resolved: rep.output.resolved,
    remaining: rep.output.remaining,
    merge_ready: rep.output.merge_ready,
    tests_ok: tests_ok,
    dogfood_ok: dogfood_ok,
    changed_files: changed,
});
