---------------------------- MODULE flowingstack ----------------------------
\* One change: contract -> frozen eval -> ledger run -> review rounds -> ship or escalate -> close
\* Edges must equal the verified rows of formal/machine.tsv (test_formal checks)
EXTENDS Naturals, Sequences
VARIABLES state, round, frozen, ledger
vars == <<state, round, frozen, ledger>>
MaxRounds == 3

Edges == {<<"drafted", "contracted">>, <<"contracted", "frozen">>, <<"frozen", "running">>,
          <<"running", "review">>, <<"review", "review">>, <<"review", "approved">>,
          <<"review", "escalated">>, <<"approved", "shipped">>, <<"approved", "escalated">>,
          <<"shipped", "closed">>, <<"escalated", "closed">>}

Go(f, t, e) == state = f /\ <<f, t>> \in Edges /\ state' = t /\ ledger' = Append(ledger, e)

Init == state = "drafted" /\ round = 0 /\ frozen = FALSE /\ ledger = <<>>
Contract == Go("drafted", "contracted", "contract") /\ UNCHANGED <<round, frozen>>
Freeze == Go("contracted", "frozen", "lock") /\ frozen' = TRUE /\ UNCHANGED round
Open == Go("frozen", "running", "open") /\ UNCHANGED <<round, frozen>>
Grill == Go("running", "review", "grill") /\ round' = 1 /\ UNCHANGED frozen
Changes == round < MaxRounds /\ Go("review", "review", "grill") /\ round' = round + 1 /\ UNCHANGED frozen
Approve == Go("review", "approved", "approve") /\ UNCHANGED <<round, frozen>>
Escalate == round = MaxRounds /\ Go("review", "escalated", "escalate") /\ UNCHANGED <<round, frozen>>
Ship == frozen /\ Go("approved", "shipped", "ship") /\ UNCHANGED <<round, frozen>>
Blocked == ~frozen /\ Go("approved", "escalated", "blocked") /\ UNCHANGED <<round, frozen>>
Close == (Go("shipped", "closed", "close") \/ Go("escalated", "closed", "close")) /\ UNCHANGED <<round, frozen>>
Tamper == frozen /\ state \in {"running", "review", "approved"} /\ frozen' = FALSE /\ UNCHANGED <<state, round, ledger>>

Next == Contract \/ Freeze \/ Open \/ Grill \/ Changes \/ Approve \/ Escalate \/ Ship \/ Blocked \/ Close \/ Tamper
Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

RoundCap == round <= 3
ShipFrozen == state = "shipped" => frozen
AppendOnly == [][Len(ledger') >= Len(ledger) /\ SubSeq(ledger', 1, Len(ledger)) = ledger]_vars
EventuallyClosed == <>(state = "closed")
=============================================================================
