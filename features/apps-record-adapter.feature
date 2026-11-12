Feature: Applications ledger record adapter

  The ledger is the source of truth. The adapter appends record events and never rewrites or deletes rows.

  ledger status     record
  prepared          intended
  submitted         submitted
  interview         next-stage:1, evidence legacy-unknown
  rejected          rejected, evidence legacy-unknown
  withdrawn         withdrawn
  silent-closed     silent
  no ledger row     confirmed is not produced

  Key = link_key, a tab, and the stage.
  Re-import compares ledger fields only. Writer and clock are ignored, so a second import appends nothing.
  After planning, import replays the log. A link whose stage differs is named and the command exits non-zero. Nothing is appended.
  Compare checks the stage, the submit time, and silent since.
  Exit 1 is a mismatch. Exit 2 is an error. A missing log, or a directory given as the log, is an error.
  A row with no submit time stores the import clock and marks that stamp synthetic.
  Writers: laptop-1, laptop-2, mac-mini.

  Known limit: the key cannot record a return to an earlier stage.
  submitted -> interview -> submitted is refused.
  The ledger stays the truth. The log is never rewritten. That link blocks the rest of the import.

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
    When the adapter imports that ledger twice, with another writer or another clock
    Then the log gains the current stage once
    And the second import appends nothing
    And the ledger bytes are unchanged
    And a same key with different ledger fields is refused

  Scenario: A return to an earlier stage is refused
    Given one link that moved submitted, then interview, then submitted
    When import runs
    Then the command exits non-zero and names that link
    And the log bytes match the log from before that import
    And the ledger bytes are unchanged

  Scenario: Compare reports each link and writes nothing
    Given a log that replays to the ledger
    When compare runs
    Then each link key is a match and the command exits 0
    And the ledger and the log are byte-identical afterwards
    When the stage, the submit time, or silent since differs
    Then that link is a mismatch and the command exits 1
    When the log is missing or the log path is a directory
    Then compare exits 2 and writes nothing
