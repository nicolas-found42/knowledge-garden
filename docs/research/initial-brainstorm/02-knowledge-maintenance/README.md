# Knowledge maintenance

[Initial brainstorm](../README.md) · [All research](../../README.md)

Question: how can a Markdown knowledge collection stay useful as sources change and repeated work reveals reusable procedures?

## Scope

Maintenance versus lookup, impact of source edits on dependent notes, acquisition of missing evidence, recurring procedural candidates, and the provenance and version checks surrounding durable updates. Initial interpretation, entity identity and topic assignment live in [knowledge formation](../01-knowledge-formation/README.md).

## Research documents

- [Repository deep dive](2026-09-29-repository-deep-dive.md): pinned implementation research in 11 repositories and 160 live calls on maintenance, dependent-note refresh, evidence acquisition and procedural candidates. The source review also documents the formation and citation contracts on which maintenance depends.

## Experiment artifacts

Scripts, fixtures and recorded results live in this concern's [experiments](experiments/) directory.

- [Repository-inspired maintenance probes](experiments/formation-repos/README.md): 40 synthetic cases, two prompt variants and two repeats; full responses, source snapshots and a retained label audit. The historical suite name `formation-repos` remains unchanged for reproducibility.

## Related concerns and shared sources

- [Knowledge formation](../01-knowledge-formation/README.md) covers evidence support, assertion lifecycle and identity before maintenance acts on them.
- [Query and context](../04-query-and-context/README.md) investigates refreshable semantic views, source freshness and context recovery.
- [Retrieval and navigation](../03-retrieval-and-navigation/README.md) helps distinguish missing knowledge from evidence that a search failed to retrieve.
- [Baseline research](../01-knowledge-formation/2026-09-29-baseline-research.md) and the [five-catalog repository landscape](../03-retrieval-and-navigation/2026-09-29-repository-landscape.md) provide shared context.

These findings are research evidence, not an accepted product design. Combined costs, coverage and limitations are in the [collection index](../README.md).

Continued after the interview in [expanded connections and reusable methods](../../expanded-exploration/03-connections-and-methods/README.md), with executed failure/correction examples and new implementation evidence. Historical experiments here remain unchanged.
