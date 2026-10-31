# UNCHECKED trial: bend was not run on this file. scripts/formal-check --bend runs it when bend is installed.
# Law: closing a ledger run twice equals closing it once (ledger close is idempotent).
import Base

type Run is Data:
  Opened{run: U32}
  Closed{run: U32}

def close(r: Run) -> Run:
  match r:
    case Opened{run}: Closed{run}
    case Closed{run}: Closed{run}

law close_idempotent:
  for r: Run
  {close(close(r)) == close(r) : Run}

def close_idempotent(r):
  match r:
    case Opened{run}: {==}
    case Closed{run}: {==}
