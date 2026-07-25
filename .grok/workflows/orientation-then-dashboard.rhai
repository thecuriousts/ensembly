// Generated draft — Grok Build workflow template.
// Install: copy to .grok/workflows/orientation-then-dashboard.rhai (or ~/.grok/workflows/).
// Requires Grok host APIs: agent(), phase(), log(), complete() — not rhai-host.
// Skill chain: seed-todos-from-goal → write-orientation-doc

let meta = #{
    name: "orientation-then-dashboard",
    description: "Produce architecture orientation docs then implement a pure-IR dashboard surface with stats and insights",
    phases: [
        #{ title: "Explore", detail: "Survey hosts, IR, and entry points" },
        #{ title: "Document", detail: "Write MAP.md and wire links" },
        #{ title: "Build", detail: "Implement dashboard IR, projectors, CLI and tests" }
    ],
    when_to_use: "Produce architecture orientation docs then implement a pure-IR dashboard surface with stats and insights",
};

phase("Explore");
let r0 = agent(
    "Follow skill 'seed-todos-from-goal'. Survey hosts, IR, and entry points. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "seed-todos-from-goal",
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
    log("phase failed: seed-todos-from-goal");
}


phase("Document");
let r1 = agent(
    "Follow skill 'write-orientation-doc'. Write MAP.md and wire links. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "write-orientation-doc",
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
    log("phase failed: write-orientation-doc");
}


phase("Build");
let r2 = agent(
    "Follow skill 'Build'. Implement dashboard IR, projectors, CLI and tests. Use read-only tools first when mapping; "
        + "then apply edits. Report {done: bool, notes: string}.",
    #{
        label: "Build",
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
    log("phase failed: Build");
}

complete(#{
    summary: "Finished workflow orientation-then-dashboard",
    skill_chain: ["seed-todos-from-goal","write-orientation-doc"],
});
