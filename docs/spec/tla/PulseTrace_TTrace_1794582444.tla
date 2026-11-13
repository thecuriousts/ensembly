---- MODULE PulseTrace_TTrace_1794582444 ----
EXTENDS Sequences, TLCExt, Toolbox, PulseTrace, Naturals, TLC

_expression ==
    LET PulseTrace_TEExpression == INSTANCE PulseTrace_TEExpression
    IN PulseTrace_TEExpression!expression
----

_trace ==
    LET PulseTrace_TETrace == INSTANCE PulseTrace_TETrace
    IN PulseTrace_TETrace!trace
----

_inv ==
    ~(
        TLCGet("level") = Len(_TETrace)
        /\
        phase = ("none")
        /\
        ev = ([old |-> "none", new |-> "none"])
        /\
        st = ([old |-> "verified", new |-> "stated"])
        /\
        snip = ([old |-> 0, new |-> 0])
        /\
        by = ([old |-> "kernel", new |-> "kernel"])
        /\
        chk = ("idle")
        /\
        row = ([old |-> "open", new |-> "absent"])
        /\
        writes = (1)
        /\
        ptr = ("nil")
    )
----

_init ==
    /\ ev = _TETrace[1].ev
    /\ snip = _TETrace[1].snip
    /\ row = _TETrace[1].row
    /\ st = _TETrace[1].st
    /\ by = _TETrace[1].by
    /\ chk = _TETrace[1].chk
    /\ phase = _TETrace[1].phase
    /\ writes = _TETrace[1].writes
    /\ ptr = _TETrace[1].ptr
----

_next ==
    /\ \E i,j \in DOMAIN _TETrace:
        /\ \/ /\ j = i + 1
              /\ i = TLCGet("level")
        /\ ev  = _TETrace[i].ev
        /\ ev' = _TETrace[j].ev
        /\ snip  = _TETrace[i].snip
        /\ snip' = _TETrace[j].snip
        /\ row  = _TETrace[i].row
        /\ row' = _TETrace[j].row
        /\ st  = _TETrace[i].st
        /\ st' = _TETrace[j].st
        /\ by  = _TETrace[i].by
        /\ by' = _TETrace[j].by
        /\ chk  = _TETrace[i].chk
        /\ chk' = _TETrace[j].chk
        /\ phase  = _TETrace[i].phase
        /\ phase' = _TETrace[j].phase
        /\ writes  = _TETrace[i].writes
        /\ writes' = _TETrace[j].writes
        /\ ptr  = _TETrace[i].ptr
        /\ ptr' = _TETrace[j].ptr

\* Uncomment the ASSUME below to write the states of the error trace
\* to the given file in Json format. Note that you can pass any tuple
\* to `JsonSerialize`. For example, a sub-sequence of _TETrace.
    \* ASSUME
    \*     LET J == INSTANCE Json
    \*         IN J!JsonSerialize("PulseTrace_TTrace_1794582444.json", _TETrace)

=============================================================================

 Note that you can extract this module `PulseTrace_TEExpression`
  to a dedicated file to reuse `expression` (the module in the 
  dedicated `PulseTrace_TEExpression.tla` file takes precedence 
  over the module `PulseTrace_TEExpression` below).

---- MODULE PulseTrace_TEExpression ----
EXTENDS Sequences, TLCExt, Toolbox, PulseTrace, Naturals, TLC

expression == 
    [
        \* To hide variables of the `PulseTrace` spec from the error trace,
        \* remove the variables below.  The trace will be written in the order
        \* of the fields of this record.
        ev |-> ev
        ,snip |-> snip
        ,row |-> row
        ,st |-> st
        ,by |-> by
        ,chk |-> chk
        ,phase |-> phase
        ,writes |-> writes
        ,ptr |-> ptr
        
        \* Put additional constant-, state-, and action-level expressions here:
        \* ,_stateNumber |-> _TEPosition
        \* ,_evUnchanged |-> ev = ev'
        
        \* Format the `ev` variable as Json value.
        \* ,_evJson |->
        \*     LET J == INSTANCE Json
        \*     IN J!ToJson(ev)
        
        \* Lastly, you may build expressions over arbitrary sets of states by
        \* leveraging the _TETrace operator.  For example, this is how to
        \* count the number of times a spec variable changed up to the current
        \* state in the trace.
        \* ,_evModCount |->
        \*     LET F[s \in DOMAIN _TETrace] ==
        \*         IF s = 1 THEN 0
        \*         ELSE IF _TETrace[s].ev # _TETrace[s-1].ev
        \*             THEN 1 + F[s-1] ELSE F[s-1]
        \*     IN F[_TEPosition - 1]
    ]

=============================================================================



Parsing and semantic processing can take forever if the trace below is long.
 In this case, it is advised to uncomment the module below to deserialize the
 trace from a generated binary file.

\*
\*---- MODULE PulseTrace_TETrace ----
\*EXTENDS IOUtils, PulseTrace, TLC
\*
\*trace == IODeserialize("PulseTrace_TTrace_1794582444.bin", TRUE)
\*
\*=============================================================================
\*

---- MODULE PulseTrace_TETrace ----
EXTENDS PulseTrace, TLC

trace == 
    <<
    ([phase |-> "none",ev |-> [old |-> "none", new |-> "none"],st |-> [old |-> "stated", new |-> "stated"],snip |-> [old |-> 0, new |-> 0],by |-> [old |-> "kernel", new |-> "kernel"],chk |-> "idle",row |-> [old |-> "absent", new |-> "absent"],writes |-> 0,ptr |-> "nil"]),
    ([phase |-> "none",ev |-> [old |-> "none", new |-> "none"],st |-> [old |-> "verified", new |-> "stated"],snip |-> [old |-> 0, new |-> 0],by |-> [old |-> "kernel", new |-> "kernel"],chk |-> "idle",row |-> [old |-> "open", new |-> "absent"],writes |-> 1,ptr |-> "nil"])
    >>
----


=============================================================================

---- CONFIG PulseTrace_TTrace_1794582444 ----
CONSTANTS
    BuggyAdmit = TRUE
    BuggyCheck = FALSE
    MaxWrites = 7

INVARIANT
    _inv

CHECK_DEADLOCK
    \* CHECK_DEADLOCK off because of PROPERTY or INVARIANT above.
    FALSE

INIT
    _init

NEXT
    _next

CONSTANT
    _TETrace <- _trace

ALIAS
    _expression
=============================================================================
\* Generated on Fri Nov 13 16:07:25 CET 2026