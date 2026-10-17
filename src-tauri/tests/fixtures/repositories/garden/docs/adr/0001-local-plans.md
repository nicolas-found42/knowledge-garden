# ADR-0001: Keep query plans local

## Decision
Keep parsed query plans local to each test process.
Only temporary test processes use this policy; production processes keep plans remotely.

## Consequences
Tests remain isolated.
