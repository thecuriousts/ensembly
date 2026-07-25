// Generated draft — Grok Build workflow template.
// Install: copy to .grok/workflows/goal-to-secured-project.rhai (or ~/.grok/workflows/).
// Requires Grok host APIs: agent(), phase(), log(), complete() — not rhai-host.
// Skill chain: plan-to-todo-seeding → project-rename-propagation → privacy-path-scrubbing

let meta = #{
    name: "goal-to-secured-project",
    description: "End-to-end cycle that turns a user goal into a new or expanded project with todos, implementation, rename, privacy scrub and push",
    phases: [
        #{ title: "Explore", detail: "read plan + inspect existing layout" },
        #{ title: "Build", detail: "seed todos, write structure, expand features" },
        #{ title: "Harden", detail: "rename, scrub paths, verify, push" }
    ],
    when_to_use: "End-to-end cycle that turns a user goal into a new or expanded project with todos, implementation, rename, privacy scrub and push",
};

phase("Explore");
let r0 = agent(
    "Follow skill 'plan-to-todo-seeding'. read plan + inspect existing layout. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "plan-to-todo-seeding",
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
    log("phase failed: plan-to-todo-seeding");
}


phase("Build");
let r1 = agent(
    "Follow skill 'project-rename-propagation'. seed todos, write structure, expand features. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "project-rename-propagation",
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
    log("phase failed: project-rename-propagation");
}


phase("Harden");
let r2 = agent(
    "Follow skill 'privacy-path-scrubbing'. rename, scrub paths, verify, push. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "privacy-path-scrubbing",
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
    log("phase failed: privacy-path-scrubbing");
}

complete(#{
    summary: "Finished workflow goal-to-secured-project",
    skill_chain: ["plan-to-todo-seeding","project-rename-propagation","privacy-path-scrubbing"],
});
