// Generated draft — Grok Build workflow template.
// Install: copy to .grok/workflows/goal-driven-hitl-spine-iteration.rhai (or ~/.grok/workflows/).
// Requires Grok host APIs: agent(), phase(), log(), complete() — not rhai-host.
// Skill chain: todo-seed-from-goal → approval-gate-mapping → circular-import-break → git-tag-commit-push

let meta = #{
    name: "goal-driven-hitl-spine-iteration",
    description: "Seed todos from goal, implement core flows, fix skeptic-flagged gates, run full test suite, then finalize with git",
    phases: [
        #{ title: "Explore", detail: "Load goal, map kernel modules, seed todos" },
        #{ title: "Implement", detail: "Build projection/graph/HITL flows and break circular deps" },
        #{ title: "Harden", detail: "Fix approval state mapping and re-activation logic" },
        #{ title: "Finalize", detail: "Tag, commit, push" }
    ],
    when_to_use: "Seed todos from goal, implement core flows, fix skeptic-flagged gates, run full test suite, then finalize with git",
};

phase("Explore");
let r0 = agent(
    "Follow skill 'todo-seed-from-goal'. Load goal, map kernel modules, seed todos. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "todo-seed-from-goal",
        capability_mode: "read-only",
        output_schema: #{
            "type": "object",
            "required": ["done", "notes"],
            "properties": #{
                "done": #{ "type": "boolean" },
                "notes": #{ "type": "string" },
            },
        },
    },
);
if r0 == () || !r0.success {
    log("phase failed: todo-seed-from-goal");
}


phase("Implement");
let r1 = agent(
    "Follow skill 'approval-gate-mapping'. Build projection/graph/HITL flows and break circular deps. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "approval-gate-mapping",
        capability_mode: "read-write",
        output_schema: #{
            "type": "object",
            "required": ["done", "notes"],
            "properties": #{
                "done": #{ "type": "boolean" },
                "notes": #{ "type": "string" },
            },
        },
    },
);
if r1 == () || !r1.success {
    log("phase failed: approval-gate-mapping");
}


phase("Harden");
let r2 = agent(
    "Follow skill 'circular-import-break'. Fix approval state mapping and re-activation logic. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "circular-import-break",
        capability_mode: "read-write",
        output_schema: #{
            "type": "object",
            "required": ["done", "notes"],
            "properties": #{
                "done": #{ "type": "boolean" },
                "notes": #{ "type": "string" },
            },
        },
    },
);
if r2 == () || !r2.success {
    log("phase failed: circular-import-break");
}


phase("Finalize");
let r3 = agent(
    "Follow skill 'git-tag-commit-push'. Tag, commit, push. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "git-tag-commit-push",
        capability_mode: "read-write",
        output_schema: #{
            "type": "object",
            "required": ["done", "notes"],
            "properties": #{
                "done": #{ "type": "boolean" },
                "notes": #{ "type": "string" },
            },
        },
    },
);
if r3 == () || !r3.success {
    log("phase failed: git-tag-commit-push");
}

complete(#{
    summary: "Finished workflow goal-driven-hitl-spine-iteration",
    skill_chain: ["todo-seed-from-goal","approval-gate-mapping","circular-import-break","git-tag-commit-push"],
});
