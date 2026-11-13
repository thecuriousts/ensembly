---------------------------- MODULE PulseTrace ----------------------------
(***************************************************************************)
(* Small model of the pulse store: two trace rows (old, new), the          *)
(* supersession phase machine, the evidence pin (verify_hash), and the     *)
(* read-only check. Synthetic; abstracts away text, time and hosts.        *)
(*                                                                         *)
(* BuggyAdmit = TRUE  models admit before E2 (verify_hash left NULL).      *)
(* BuggyCheck = TRUE  models `pulse check` today: its verdict ignores the  *)
(*                    evidence pin and half-applied supersessions.         *)
(***************************************************************************)
EXTENDS Naturals

CONSTANTS BuggyAdmit, BuggyCheck, MaxWrites

Keys == {"old", "new"}

VARIABLES
    row,     \* [Keys -> {"absent","open","locked"}]
    st,      \* [Keys -> {"verified","stated"}]   status: tool-verified or not
    by,      \* [Keys -> {"kernel","foreign"}]    who wrote the row
    ev,      \* [Keys -> {"none","match"}]        verify_hash pins the snippet
    snip,    \* [Keys -> 0..1]                    snippet version
    ptr,     \* "nil" or "new": old.superseded_by
    phase,   \* supersession phase
    writes,  \* committed write transactions (bounded by MaxWrites)
    chk      \* last read-only verdict; reset to "idle" by any write

dbVars == <<row, st, by, ev, snip, ptr, phase, writes>>
vars == <<dbVars, chk>>

TypeOK ==
    /\ row \in [Keys -> {"absent", "open", "locked"}]
    /\ st \in [Keys -> {"verified", "stated"}]
    /\ by \in [Keys -> {"kernel", "foreign"}]
    /\ ev \in [Keys -> {"none", "match"}]
    /\ snip \in [Keys -> 0..1]
    /\ ptr \in {"nil", "new"}
    /\ phase \in {"none", "intent", "applied", "verified"}
    /\ writes \in 0..MaxWrites
    /\ chk \in {"idle", "ok", "warn"}

Init ==
    /\ row = [k \in Keys |-> "absent"]
    /\ st = [k \in Keys |-> "stated"]
    /\ by = [k \in Keys |-> "kernel"]
    /\ ev = [k \in Keys |-> "none"]
    /\ snip = [k \in Keys |-> 0]
    /\ ptr = "nil"
    /\ phase = "none"
    /\ writes = 0
    /\ chk = "idle"

Write == writes' = writes + 1 /\ chk' = "idle"

\* Kernel admit (pulse admit / insert_trace_tx).
Admit(k, s, l) ==
    /\ row[k] = "absent"
    /\ row' = [row EXCEPT ![k] = l]
    /\ st' = [st EXCEPT ![k] = s]
    /\ by' = [by EXCEPT ![k] = "kernel"]
    /\ ev' = [ev EXCEPT ![k] = IF BuggyAdmit THEN "none" ELSE "match"]
    /\ snip' = [snip EXCEPT ![k] = 0]
    /\ Write
    /\ UNCHANGED <<ptr, phase>>

\* A legacy or foreign row: tool-verified with no pin (old Rust admit, hand edits).
ForeignRow(k) ==
    /\ row[k] = "absent"
    /\ row' = [row EXCEPT ![k] = "open"]
    /\ st' = [st EXCEPT ![k] = "verified"]
    /\ by' = [by EXCEPT ![k] = "foreign"]
    /\ ev' = [ev EXCEPT ![k] = "none"]
    /\ Write
    /\ UNCHANGED <<snip, ptr, phase>>

\* Open rows may change their snippet; the kernel re-pins it.
Upsert(k) ==
    /\ row[k] = "open"
    /\ by[k] = "kernel"
    /\ snip' = [snip EXCEPT ![k] = 1 - snip[k]]
    /\ ev' = [ev EXCEPT ![k] = IF BuggyAdmit THEN "none" ELSE "match"]
    /\ Write
    /\ UNCHANGED <<row, st, by, ptr, phase>>

Lock(k) ==
    /\ row[k] = "open"
    /\ row' = [row EXCEPT ![k] = "locked"]
    /\ Write
    /\ UNCHANGED <<st, by, ev, snip, ptr, phase>>

\* A different snippet for a locked row is refused: no write at all.
AdmitConflicting(k) ==
    /\ row[k] = "locked"
    /\ UNCHANGED vars

SupIntent ==
    /\ phase = "none" /\ row["old"] # "absent" /\ row["new"] # "absent"
    /\ phase' = "intent" /\ Write
    /\ UNCHANGED <<row, st, by, ev, snip, ptr>>

SupApply ==
    /\ phase = "intent"
    /\ phase' = "applied" /\ ptr' = "new" /\ Write
    /\ UNCHANGED <<row, st, by, ev, snip>>

SupVerify ==
    /\ phase = "applied"
    /\ phase' = "verified" /\ Write
    /\ UNCHANGED <<row, st, by, ev, snip, ptr>>

\* A crash between transactions, or SQLITE_BUSY after busy_timeout: nothing commits.
Crash == UNCHANGED vars
BusyTimeout == UNCHANGED vars

Unevidenced == \E k \in Keys : row[k] # "absent" /\ st[k] = "verified" /\ ev[k] = "none"
HalfWritten == phase \in {"intent", "applied"}

\* pulse check / pulse canon --from-db: mode=ro + query_only, so no db variable moves.
Check ==
    /\ chk' = IF ~BuggyCheck /\ (Unevidenced \/ HalfWritten) THEN "warn" ELSE "ok"
    /\ UNCHANGED dbVars

Next ==
    \/ \E k \in Keys, s \in {"verified", "stated"}, l \in {"open", "locked"} : Admit(k, s, l)
    \/ \E k \in Keys : ForeignRow(k) \/ Upsert(k) \/ Lock(k) \/ AdmitConflicting(k)
    \/ SupIntent \/ SupApply \/ SupVerify
    \/ Crash \/ BusyTimeout
    \/ Check

Spec == Init /\ [][Next]_vars

WithinBound == writes <= MaxWrites - 1

----------------------------------------------------------------------------
\* Rules. Each maps to a test in the Rust suite (see README.md).

ConfirmedNeedsEvidence ==
    \A k \in Keys : (row[k] # "absent" /\ by[k] = "kernel" /\ st[k] = "verified") => ev[k] = "match"

CheckIsHonest == chk = "ok" => (~Unevidenced /\ ~HalfWritten)

HalfStateResumable ==
    /\ phase = "intent" => ENABLED SupApply
    /\ phase = "applied" => ENABLED SupVerify

PointerOnlyAfterApply == ptr = "new" => phase \in {"applied", "verified"}

ReadOnlyNeverWrites == [][Check => UNCHANGED dbVars]_vars

NoWriteOnTimeout == [][BusyTimeout => UNCHANGED dbVars]_vars

LockedNeverOverwritten ==
    [][\A k \in Keys : row[k] = "locked" => (row'[k] = "locked" /\ snip'[k] = snip[k])]_vars
=============================================================================
