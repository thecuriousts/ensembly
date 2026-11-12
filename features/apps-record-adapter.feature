Feature: Applications ledger record adapter

  The application ledger stays the source of truth.
  The adapter appends record events and never rewrites or deletes ledger rows.
  Confirmed is a record status with no ledger source.
  Rejected evidence uses source legacy-unknown.
  Interview maps to next-stage 1. The record type requires evidence, so that evidence source is also legacy-unknown.
  Silent uses the row submit time when it is known, and the caller-supplied observed time when it is not.
  The idempotency key is the canonical link key, a tab, and the stage token.
  Writers on this path are laptop-1 and mac-mini.

  Scenario: Every ledger status maps
    Given the six ledger statuses
    Then prepared maps to intended
    And submitted maps to submitted
    And interview maps to next-stage:1
    And rejected maps to rejected with evidence source legacy-unknown
    And withdrawn maps to withdrawn
    And silent-closed maps to silent
    And no ledger status maps to confirmed

  Scenario: An unknown status is refused
    When the status name is offered
    Then the error names that status and the six known names

  Scenario: Import twice is once, and old ledger rows stay
    Given a ledger with an older row and a later row for the same link
    When the adapter imports that ledger twice
    Then the log gains the current stage once
    And the second import appends nothing
    And the ledger bytes are unchanged
    And a same key with a different payload is refused

  Scenario: Compare reports each link and writes nothing
    Given a log that replays to the ledger
    When compare runs
    Then each link key is a match and the command exits 0
    And the ledger and the log are byte-identical afterwards
    When one record stage differs from the ledger
    Then that link key is a mismatch and the command exits non-zero
    And the ledger and the log are still byte-identical
