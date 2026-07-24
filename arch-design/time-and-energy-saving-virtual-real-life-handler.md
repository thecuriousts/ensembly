# Control Insights for ensembly — Issue #1  
**Fitness for Human in the loop & out of the loop**

> Living document distilled from first-principles analysis of the Map of Control Theory, Operations Research, Physics foundations, multi-agent systems, and the specific requirements of Issue #1.  
> Purpose: keep implementation focused, structural, and high-quality while remaining small and optimal.

---

## 1. Core Design Principles (from Issue #1)

These are non-negotiable:

- **Automate the digital.**
- **Surface the physical.**
- **Wait only for permission.**
- **Make the truth playable** (even if the game surface is optional / excluded for now).

The human is the scarce, high-value resource.  
The swarm handles digital thrash.  
The runtime keeps the global truth and decides when the human must intervene.

Evaluation criteria for every decision:

1. **Correctness** — does it do the right thing?
2. **Effectiveness** — does it save energy and effort for the future?
3. **Efficiency** — does it spend less time and cost?

---

## 2. Architecture Decision: Exclude Game Layer (for now)

**Decision**: Keep the core free of Unity / immersive game code.

**Rationale**:
- Dramatically smaller codebase
- Higher structural clarity
- More optimal focus on the real problems
- Game surface can later be added as an optional client that speaks the same message/event protocol

**What remains (the high-value core)**:
- Authoritative life-state model
- Directed dependency graph + critical-path engine
- Multi-agent coordination for digital tasks
- Clean HITL / HOOTL separation
- Time-sensitive automatic actions
- Reliable state updates and message passing

---

## 3. Two-Layer Model (Must Preserve)

```
┌─────────────────────────────────────────────────────┐
│           Program Runtime (Rust Control Plane)      │
│  • Life-state + Directed Graph (single source of    │
│    truth)                                           │
│  • Critical Path / PERT / Monte Carlo engine        │
│  • Global prioritization & resource allocation      │
│  • Trigger / Invoke logic                           │
│  • HITL escalation (“wait only for permission”)     │
│  • Message / Event bus                              │
└──────────────────────┬──────────────────────────────┘
                       │ typed messages / events
                       ▼
┌─────────────────────────────────────────────────────┐
│         Driving Multi-Agents (Workers)              │
│  • Local decision-making & coordination             │
│  • Digital task execution                           │
│  • Local planning & estimation                      │
│  • Report status / results back to runtime          │
└─────────────────────────────────────────────────────┘
```

- **Runtime** owns global truth, prioritization, and when to surface physical beacons or authorization gates.
- **Multi-agents** own local intelligence and actual digital work.

This separation is essential for both correctness and scalability.

---

## 4. Technique Placement (Where Each Technique Belongs)

### A. Program Runtime (Control Plane) — Global

| Technique                              | Role in Runtime                                      | Priority |
|----------------------------------------|------------------------------------------------------|----------|
| Directed Graph + Critical Path (OR)    | Core prioritization engine                           | Must     |
| PERT / Monte Carlo                     | Uncertainty quantification on the critical path      | Must     |
| Hierarchical / Global Planning         | Day-level and sequence-level planning                | Must     |
| Optimal / Predictive Control thinking  | Allocation of swarm capacity + human attention       | High     |
| Global State Estimation                | Fused belief about the entire life state             | High     |
| System Analysis concepts               | Bottleneck detection, overall system health          | High     |
| Graph Theory + Optimization (Math)     | Foundation of dependency analysis                    | Must     |
| Event-driven + Hooks + Triggers        | Invoke logic and time-sensitive automation           | Must     |
| Robust methods                         | Handling uncertainty in durations & dependencies     | High     |

### B. Driving Multi-Agents — Local

| Technique                              | Role in Agents                                       | Priority |
|----------------------------------------|------------------------------------------------------|----------|
| Multi-agent Control                    | Coordination, conflict avoidance, work claiming      | Must     |
| Adaptive / Intelligent (RL-style)      | Local policy improvement over time                   | High     |
| Local Planning                         | Short-horizon decisions for individual agents        | High     |
| Local State Estimation                 | Belief about the digital systems they manage         | High     |
| MDPs / Dynamic Programming (OR)        | Mathematical model of sequential local decisions     | Medium–High |
| Robust local handling                  | Timeouts, failures, noisy digital signals            | High     |

### C. Cross-cutting / Optional for later

- Physics-Informed Neural Networks (PINNs) or hybrid residual models → only if continuous dynamics modeling of life-state becomes necessary.
- Full continuous-time optimal control (HJB) → usually overkill; discrete methods on the task graph are preferred.

---

## 5. Main Infrastructure Challenges (and how to attack them)

These are the real engineering bottlenecks once the game layer is removed:

1. **Coordination**  
   Solution direction: Make the directed dependency graph the coordination substrate. Agents claim or request work only through the graph/critical-path engine. Prefer structured claiming over free-form agent-to-agent chatter.

2. **Invoke / Trigger**  
   Solution direction: Derive triggers from changes in the life-state graph and critical path (new critical tasks, approaching deadlines, authorization needed). Keep the trigger system declarative and inspectable.

3. **Updating state and data**  
   Solution direction: Single source of truth in the Rust runtime. All mutations go through a narrow, typed, auditable interface. Leverage Rust ownership and type system heavily. Prefer immutable or versioned graph updates where practical.

4. **Message passing infrastructure**  
   Solution direction: Typed, ordered event/message bus (or channels) between runtime ↔ agents. Manual commands, automatic triggers, and agent reports all travel on the same infrastructure. Support back-pressure and clear ownership of messages.

---

## 6. Recommended Implementation Order (Essentials First)

1. **Life-state model + Directed dependency graph** (single source of truth)
2. **Critical Path / PERT engine** with basic Monte Carlo uncertainty
3. **Typed message / event bus** (runtime ↔ agents)
4. **Trigger / Invoke system** driven by graph changes
5. **HITL escalation points** (physical beacons + authorization gates)
6. **Basic multi-agent workers** that can claim work from the graph and report results
7. **Observability** against the three evaluation criteria (correctness, effectiveness, efficiency)

Only after the above are solid should more advanced adaptive / RL-style local policies or hybrid modeling be added.

---

## 7. Key Invariants (Must Never Be Violated)

- The runtime is the single source of truth for life-state and prioritization.
- Digital work is automated; physical work and high-risk decisions are surfaced.
- The human is never forced into the loop; the loop waits for the human only when necessary.
- Privacy boundaries remain local-first.
- Every prioritization decision is explainable via the critical path / graph.
- The system trends toward greater automation while preserving human judgment where it matters.

---

## 8. Relationship to Existing ensembly Code

- Build on the existing `peram-core` (Rust → WASM) direction.
- Strengthen the Graph IR into a first-class directed dependency graph with critical-path computation.
- Keep the day/turn planning loops, but drive them from the new critical-path engine.
- CLI commands (`approve`, `deny`, `claim`, etc.) become messages into the same event bus.
- The current Eisenhower-style prioritization can be replaced or augmented by the graph + critical-path approach.

---

## 9. Success Metrics (from Issue #1)

A change is successful when it improves at least one of:

- **Correctness** of prioritization and escalation
- **Effectiveness** (energy and future effort saved)
- **Efficiency** (time and cost spent)

Measure these continuously.

---

*This document is intended to live in the ensembly repository (suggested location: `docs/CONTROL-INSIGHTS-ISSUE-1.md`) and to be updated as implementation progresses.*

**Last distilled**: 2026-07-24  
From the full discussion chain on Control Theory foundations, Operations Research, multi-agent systems, and their concrete application to ensembly Issue #1.
