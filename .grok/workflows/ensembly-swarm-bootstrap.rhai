// Generated draft — Grok Build workflow template.
// Install: copy to .grok/workflows/ensembly-swarm-bootstrap.rhai (or ~/.grok/workflows/).
// Requires Grok host APIs: agent(), phase(), log(), complete() — not rhai-host.
// Skill chain: goal-to-todo-seed → tidy-commit-push → annotated-tag-push → distill-private-alpha

let meta = #{
    name: "ensembly-swarm-bootstrap",
    description: "Ingest goal, seed todos, implement roadmap modules, distill private alpha, perform tidy commits and annotated tagging to mark pre-swarm baseline",
    phases: [
        #{ title: "Goal Ingestion", detail: "Read goal, seed todos, wire looper and persona" },
        #{ title: "Roadmap Execution", detail: "Build realm/approvals/graph modules and tests" },
        #{ title: "Hygiene and Release", detail: "Tidy, commit, tag pre-swarm state, push" }
    ],
    when_to_use: "Ingest goal, seed todos, implement roadmap modules, distill private alpha, perform tidy commits and annotated tagging to mark pre-swarm baseline",
};

phase("Goal Ingestion");
let r0 = agent(
    "Follow skill 'goal-to-todo-seed'. Read goal, seed todos, wire looper and persona. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "goal-to-todo-seed",
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
    log("phase failed: goal-to-todo-seed");
}


phase("Roadmap Execution");
let r1 = agent(
    "Follow skill 'tidy-commit-push'. Build realm/approvals/graph modules and tests. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "tidy-commit-push",
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
    log("phase failed: tidy-commit-push");
}


phase("Hygiene and Release");
let r2 = agent(
    "Follow skill 'annotated-tag-push'. Tidy, commit, tag pre-swarm state, push. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "annotated-tag-push",
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
    log("phase failed: annotated-tag-push");
}

complete(#{
    summary: "Finished workflow ensembly-swarm-bootstrap",
    skill_chain: ["goal-to-todo-seed","tidy-commit-push","annotated-tag-push","distill-private-alpha"],
});
