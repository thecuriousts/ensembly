Control Insights for ensembly — Issue #1  
**Fitness for Human-in-the-loop (HITL) & Human-out-of-the-loop (HOOTL)**

> Living document.  
> Primary expression: **AppGenMathPhyLang** — applied generative math-physics language.  
> Secondary: professor-style natural-language commentary for human readability.

---

# I. Pure AppGenMathPhyLang Formalization

```
// ──────────────────────────────────────────────────────────────
// 0. Signature (minimal)
// ──────────────────────────────────────────────────────────────
S          : life-state space
G = (V,E)  : directed dependency graph on tasks / states
CP(G)      : critical-path operator
P          : PERT / Monte-Carlo uncertainty measure
Runtime    : control plane (Rust)
Agents     : multi-agent workers
Human      : scarce resource
Digital    : automatable domain
Physical   : surface domain
HITL       : human-in-the-loop gate
HOOTL      : human-out-of-the-loop regime

// ──────────────────────────────────────────────────────────────
// 1. Core Design Principles (Axioms)
// ──────────────────────────────────────────────────────────────
Automate(Digital)
Surface(Physical)
WaitOnlyForPermission(Human)
TruthPlayable(S)                    // even if game surface excluded

Human ∈ Scarce ∧ HighValue
Agents ⊨ DigitalThrash
Runtime ⊨ GlobalTruth ∧ EscalationPolicy

∀δ ∈ Decisions :
  Evaluate(δ) ≔ (Correctness(δ) , Effectiveness(δ) , Efficiency(δ))
  // Correctness  = does the right thing
  // Effectiveness = saves energy & future effort
  // Efficiency   = spends less time & cost

// ──────────────────────────────────────────────────────────────
// 2. Architecture Decision
// ──────────────────────────────────────────────────────────────
Core ≔ Runtime ∖ GameLayer
  // Rationale: |Core| ≪ |Full| , clarity↑ , focus↑
  // GameLayer later recoverable as optional client over same MsgBus

Core ⊃ {
  LifeState(S),
  G + CP,
  MultiAgentCoord(Digital),
  HITL / HOOTL separation,
  TimeSensitiveAuto,
  TypedMsgBus
}

// ──────────────────────────────────────────────────────────────
// 3. Two-Layer Model (Invariant Structure)
// ──────────────────────────────────────────────────────────────
Runtime : {
  S + G          // single source of truth
  CP + P         // critical path + uncertainty
  GlobalPrior + ResourceAlloc
  Trigger / Invoke
  HITL-escalation (“wait only for permission”)
  MsgBus
}

Agents : {
  LocalDecision + Coord
  DigitalExec
  LocalPlan + Estimate
  Report → Runtime
}

Runtime ⊥ Agents   via typed events / messages
Runtime ⊨ GlobalTruth ∧ Prioritization ∧ Surface(Physical ∨ AuthGate)
Agents  ⊨ LocalIntelligence ∧ DigitalWork

// ──────────────────────────────────────────────────────────────
// 4. Technique Placement
// ──────────────────────────────────────────────────────────────
// A. Runtime (Global)
CP(G) + GraphOpt         : prioritization engine          [Must]
P (PERT / MonteCarlo)    : uncertainty on CP              [Must]
HierPlan / GlobalPlan    : day- & sequence-level           [Must]
PredCtrl / OptimalCtrl   : swarm capacity + Human attention [High]
GlobalStateEst           : fused belief κ_global ∈ Δ(S)   [High]
SysAnalysis              : bottleneck + health             [High]
Event + Hook + Trigger   : invoke logic                    [Must]
RobustMethods            : uncertainty in duration / dep   [High]

// B. Agents (Local)
MultiAgentCtrl           : coord, claim, conflict avoid    [Must]
Adaptive / RL-style      : local policy improvement        [High]
LocalPlan                : short-horizon                   [High]
LocalStateEst            : κ_local                         [High]
MDP / DP                 : sequential local decisions      [Med–High]
RobustLocal              : timeouts, failures, noise       [High]

// C. Cross-cutting (deferred)
PINN / hybrid residual   : only if continuous dynamics of S needed
HJB continuous OC        : usually overkill; prefer discrete CP

// ──────────────────────────────────────────────────────────────
// 5. Infrastructure Challenges → Solution Operators
// ──────────────────────────────────────────────────────────────
1. Coord
   G + CP  ≔ coordination substrate
   Agents claim / request  only through CP engine
   Prefer structured claim  ≻ free-form chatter

2. Invoke / Trigger
   Triggers ≔ Δ(G) ∪ Δ(CP) ∪ (deadline → 0) ∪ AuthNeeded
   Declarative ∧ Inspectable

3. State Update
   Runtime ≔ single source of truth
   ∀ mutation : typed ∧ auditable ∧ (immutable ∨ versioned)
   Leverage Rust ownership + type system

4. MsgBus
   Typed ∧ Ordered channels  Runtime ↔ Agents
   ManualCmd ∪ AutoTrigger ∪ AgentReport  travel on same bus
   Support back-pressure ∧ clear ownership

// ──────────────────────────────────────────────────────────────
// 6. Implementation Order (Essentials First)
// ──────────────────────────────────────────────────────────────
1. S + G                         // single source of truth
2. CP + P (basic MonteCarlo)
3. Typed MsgBus (Runtime ↔ Agents)
4. Trigger / Invoke  driven by Δ(G)
5. HITL escalation points (PhysicalBeacon + AuthGate)
6. Basic Agents  that claim from G and Report
7. Observability against (Correctness, Effectiveness, Efficiency)

Only after solid: Adaptive / RL local policies or hybrid modeling.

// ──────────────────────────────────────────────────────────────
// 7. Key Invariants (Must Never Be Violated)
// ──────────────────────────────────────────────────────────────
Runtime ⊨ SingleSourceOfTruth(S , Prioritization)
Digital ⊂ Automate  ∧  Physical ∪ HighRisk ⊂ Surface
Human never forced;  loop waits only when necessary
Privacy ≔ local-first
∀ prioritization decision : explainable via CP(G)
System → greater automation while preserving Human judgment where it matters

// ──────────────────────────────────────────────────────────────
// 8. Relationship to Existing Code
// ──────────────────────────────────────────────────────────────
BuildOn(peram-core : Rust → WASM)
Strengthen GraphIR → first-class G + CP
Day/Turn loops driven by CP
CLI(approve, deny, claim, …) ↦ MsgBus
Eisenhower prioritization  ≼  G + CP   (replace or augment)

// ──────────────────────────────────────────────────────────────
// 9. Success Metrics
// ──────────────────────────────────────────────────────────────
Success(δ)  ⇔  ΔCorrectness(δ) > 0
            ∨  ΔEffectiveness(δ) > 0   // energy + future effort saved
            ∨  ΔEfficiency(δ) > 0      // time + cost spent

Measure continuously.
```

---

# Formal Read-Aloud of Section I

*(Spoken continuously in the style of a mathematics–physics professor, American English; classical quantifier language: “for all,” “there exists,” formal set-membership.)*

## Section One. Pure AppGenMathPhyLang Formalization.

**Zero. The signature, kept minimal.**

Let S denote the life-state space.

Let G equal the ordered pair of vertex set V and edge set E.

Thus G is the directed dependency graph whose vertices are tasks or states.

Let CP of G denote the critical-path operator that extracts the longest path under the current duration estimates.

Let P denote the uncertainty measure given either by classical PERT analysis or by Monte-Carlo sampling.

Runtime is the central control plane, realized in Rust.

Agents are the multi-agent digital workers.

Human is a scarce resource.

Digital is the automatable domain.

Physical is the domain that must be surfaced.

HITL stands for the human-in-the-loop gate.

HOOTL stands for the human-out-of-the-loop regime.

**One. Core design principles, stated as axioms.**

Automate every purely digital action.

Surface every physical action.

The control loop waits only for explicit human permission.

The global truth of S remains playable even when the immersive game surface is excluded.

Human belongs to the intersection of the scarce set and the high-value set.

Agents satisfy DigitalThrash.

Runtime satisfies GlobalTruth and EscalationPolicy.

For all delta belonging to the set of decisions,

the evaluation of delta is the ordered triple consisting of the correctness of delta, the effectiveness of delta, and the efficiency of delta.

Correctness asserts that the system performs the right action.

Effectiveness asserts that energy and future effort are conserved.

Efficiency asserts that time and monetary cost are reduced.

**Two. Architecture decision.**

Core is Runtime with the game layer removed.

Rationale: the cardinality of Core is strictly smaller than the cardinality of the full system.

Clarity increases.

Focus increases.

The game layer remains recoverable later as an optional client speaking the same typed message bus.

Core properly contains:

the life-state model of S,

the directed graph G together with its critical-path operator,

multi-agent coordination restricted to the digital domain,

the clean separation of HITL from HOOTL,

time-sensitive automatic actions,

and the typed message bus.

**Three. The two-layer model, an invariant structure.**

Runtime owns:

S together with G as the single source of truth,

CP together with P as the critical-path-plus-uncertainty engine,

global prioritization together with resource allocation,

the trigger-and-invoke logic,

the HITL escalation points that wait only for permission,

and the typed message bus.

Agents own:

local decision-making together with coordination,

actual digital task execution,

short-horizon planning together with estimation,

and the reporting map that sends status back to the runtime.

Runtime is independent of the agents.

Communication occurs exclusively through typed events and messages.

Runtime satisfies the conjunction of global truth, prioritization, and the surfacing of either physical actions or authorization gates.

Agents satisfy the conjunction of local intelligence and digital work.

**Four. Technique placement.**

*A. Techniques that reside in the runtime, that is, the global layer.*

The critical-path operator of G together with graph optimization constitutes the prioritization engine and is mandatory.

The uncertainty measure P, whether PERT or Monte-Carlo, quantifies the critical path and is mandatory.

Hierarchical planning and global planning operate at the day and sequence level and are mandatory.

Predictive control and optimal control allocate swarm capacity and human attention and are of high priority.

Global state estimation produces a fused belief kappa-global belonging to the simplex of probability measures on S and is of high priority.

System analysis detects bottlenecks and monitors health and is of high priority.

Event, hook, and trigger logic implements invocation and is mandatory.

Robust methods handle uncertainty in durations and dependencies and are of high priority.

*B. Techniques that reside in the agents, that is, the local layer.*

Multi-agent control, responsible for claiming, conflict avoidance, and coordination, is mandatory.

Adaptive and reinforcement-learning-style policy improvement is of high priority.

Short-horizon local planning is of high priority.

Local state estimation producing kappa-local is of high priority.

Markov decision processes and dynamic programming for sequential local decisions range from medium to high priority.

Robust local handling of timeouts, failures, and noise is of high priority.

*C. Cross-cutting techniques that are deferred.*

Physics-informed neural networks or hybrid residual models are admitted only if continuous dynamics of the life-state space become necessary.

Full continuous-time Hamilton-Jacobi-Bellman optimal control is ordinarily overkill.

The discrete critical-path methods on the task graph are preferred.

**Five. Infrastructure challenges mapped to solution operators.**

*One. Coordination.*

The directed graph G together with its critical-path operator is the sole coordination substrate.

Agents claim or request work only through that engine.

Structured claims are strictly preferred to free-form chatter.

*Two. Invoke and trigger.*

The set of triggers is the union of

the change set of G,

the change set of the critical path,

the set of deadlines that have reached zero,

and the set of points at which authorization is required.

Every trigger remains declarative and inspectable.

*Three. State update.*

The runtime is the single source of truth.

For every mutation there exist the three properties:

the mutation is typed,

the mutation is auditable,

and the mutation is either immutable or versioned.

The ownership and type system of Rust are leveraged heavily.

*Four. Message bus.*

Typed and ordered channels connect the runtime to the agents.

Manual commands, automatic triggers, and agent reports all travel on the same bus.

The bus supports back-pressure and clear ownership of every message.

**Six. Implementation order, essentials first.**

1. Construct the life-state model S together with the directed dependency graph G, thereby establishing the single source of truth.

2. Construct the critical-path operator together with the basic Monte-Carlo realization of P.

3. Construct the typed message bus linking runtime to agents.

4. Construct the trigger-and-invoke system driven by changes in the graph.

5. Insert the HITL escalation points consisting of physical beacons and authorization gates.

6. Introduce basic agents that claim work from the graph and report results.

7. Establish continuous observability against the three evaluation criteria of correctness, effectiveness, and efficiency.

Only after the foregoing are solid may adaptive or reinforcement-learning local policies, or hybrid continuous modeling, be introduced.

**Seven. Key invariants that must never be violated.**

The runtime satisfies the property of being the single source of truth for both the life-state and prioritization.

The digital domain is contained in the automatable set.

The physical domain union the high-risk set is contained in the surface set.

The human is never forced into the loop.

The loop waits for the human only when necessary.

Privacy is local-first.

For every prioritization decision there exists an explanation that refers solely to the critical path of the graph.

The system evolves toward greater automation while still preserving human judgment wherever that judgment matters.

**Eight. Relationship to existing code.**

We build directly upon the existing peram-core, which maps Rust to WebAssembly.

The Graph intermediate representation is strengthened until it becomes a first-class directed dependency graph carrying critical-path computation.

Day-level and turn-level planning loops continue to exist but are now driven by the critical-path engine.

Command-line interface actions such as approve, deny, and claim become ordinary messages on the same event bus.

The present Eisenhower-style prioritization is either replaced by or augmented by the graph-plus-critical-path approach.

**Nine. Success metrics.**

A change delta is successful if and only if at least one of the following three inequalities holds:

the change in correctness of delta is strictly positive,

or the change in effectiveness of delta is strictly positive (energy and future effort are saved),

or the change in efficiency of delta is strictly positive (time and cost are reduced).

These quantities are measured continuously.

*(End of pure formalization read-aloud.)*
