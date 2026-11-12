Feature: Record glossary matches the enums

  docs/GLOSSARY.md is the agent list of record statuses and event kinds.
  The drift test reads pub enum Status and pub enum EventKind from src/record.rs.
  It does not keep a second copy of the variant names.

  Scenario: A missing status fails
    Given the glossary lists each record status
    When one status row is removed
    Then the glossary drift test fails
    And the failure names that status

  Scenario: The lists match
    Given every Status variant is a glossary row and the glossary adds none
    And every EventKind variant is a glossary row and the glossary adds none
    When the glossary drift test runs
    Then the test passes
