# Query and context

[Initial brainstorm](../README.md) · [All research](../../README.md)

Question: how can semantic judgments compose into bounded queries and provide agents with current, sufficient evidence?

## Scope

Typed query operators and argument binding, refreshable semantic views, source freshness, summary-to-raw evidence acquisition, context retention and compaction. Candidate discovery, evidence ranking and experimental graph navigation live in [retrieval and navigation](../03-retrieval-and-navigation/README.md).

## Research documents

- [Repository deep dive](2026-09-29-repository-deep-dive.md): pinned source inspection in ten repositories, including JevSQL, System One Search, Jev-Mem, TierMem and context-selection implementations. Four experiment families and follow-ups produced 100 successful evaluations, with five initial service failures preserved and recovered.

## Experiment artifacts

Scripts, fixtures and recorded results live in this concern's [experiments](experiments/) directory.

- [Query and context repository probes](experiments/retrieval-repos/README.md): freshness, context retention, typed query plans, sequential raw-source lookup, separate retry/follow-up records and audited metrics. The historical suite name `retrieval-repos` remains unchanged for reproducibility.

The typed-plan probes bind operations over in-memory dictionaries; they do not implement a production query engine or a complete graph traversal.

## Related concerns and shared sources

- [Retrieval and navigation](../03-retrieval-and-navigation/README.md) provides candidate acquisition, complementary evidence selection and graph stopping experiments.
- [Knowledge maintenance](../02-knowledge-maintenance/README.md) investigates source-dependent note refresh and durable update boundaries.
- [Knowledge formation](../01-knowledge-formation/README.md) covers source support and preserves the [shared baseline research](../01-knowledge-formation/2026-09-29-baseline-research.md).
- [Five-catalog repository landscape](../03-retrieval-and-navigation/2026-09-29-repository-landscape.md) records the collection's shared discovery coverage.

These findings are research evidence, not an accepted product design. Combined costs, coverage and limitations are in the [collection index](../README.md).

Continued after the interview in [actual history and representation experiments](../../expanded-exploration/02-agent-histories/README.md) and [source recovery/navigation](../../expanded-exploration/04-navigation-and-provenance/README.md). Historical experiments here remain unchanged.
