// Generated draft — Grok Build workflow template.
// Install: copy to .grok/workflows/goal-driven-swarm-evolution.rhai (or ~/.grok/workflows/).
// Requires Grok host APIs: agent(), phase(), log(), complete() — not rhai-host.
// Skill chain: goal-to-todo-seed → private-public-distill → apply-external-patterns

let meta = #{
    name: "goal-driven-swarm-evolution",
    description: "Iteratively bootstrap and refine an autonomous swarm project from legacy code using goal plans, private distillation, roadmap modules, game surface, and performance patterns",
    phases: [
        #{ title: "Bootstrap", detail: "Seed todos from goal, implement roadmap modules" },
        #{ title: "Distill", detail: "Private alpha pack + tracked gist with credits" },
        #{ title: "Polish", detail: "Game surface + pattern integration + git tag/push" }
    ],
    when_to_use: "Iteratively bootstrap and refine an autonomous swarm project from legacy code using goal plans, private distillation, roadmap modules, game surface, and performance patterns",
};

phase("Bootstrap");
let r0 = agent(
    "Follow skill 'goal-to-todo-seed'. Seed todos from goal, implement roadmap modules. Use read-only tools first when mapping; "
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


phase("Distill");
let r1 = agent(
    "Follow skill 'private-public-distill'. Private alpha pack + tracked gist with credits. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "private-public-distill",
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
    log("phase failed: private-public-distill");
}


phase("Polish");
let r2 = agent(
    "Follow skill 'apply-external-patterns'. Game surface + pattern integration + git tag/push. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "apply-external-patterns",
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
    log("phase failed: apply-external-patterns");
}

complete(#{
    summary: "Finished workflow goal-driven-swarm-evolution",
    skill_chain: ["goal-to-todo-seed","private-public-distill","apply-external-patterns"],
});
