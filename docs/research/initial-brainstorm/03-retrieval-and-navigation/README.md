# Retrieval and navigation

[Initial brainstorm](../README.md) · [All research](../../README.md)

Question: how can Jev find relevant knowledge, assemble complementary evidence and navigate known links toward an answer?

## Scope

Candidate recall and ranking, indirect constraints, evidence bundles, source disagreement and temporal scope, bounded graph walks, stopping, clarification and reusable relevance scores. Typed query execution, current-source checks and agent context management live in [query and context](../04-query-and-context/README.md).

## Research documents

- [Retrieval experiments](2026-09-29-retrieval-experiments.md): 257 live calls across broad retrieval, constraints, complementary evidence, temporal scope, graph walks, clarification, reusable scores and competing valid answers.
- [Five-catalog repository landscape](2026-09-29-repository-landscape.md): the collection's shared inventory of 1,077 repositories and original source findings. It covers all four concerns and distinguishes catalog discovery from implementation inspection.

## Experiment artifacts

Scripts, fixtures and recorded results live in this concern's [experiments](experiments/) directory.

- [Retrieval lab](experiments/retrieval-lab/README.md): miniature corpus, initial labels, follow-up probes, full responses and a machine-readable summary.
- [Baseline retrieval probes](experiments/baseline-retrieval/README.md): two historical records extracted from the original mixed baseline, with provenance; these are not additional calls.
- [Jev-assisted repository discovery](experiments/repository-discovery/README.md): 335 public README cards, 306 semantic judgments and explicit clipped/unjudged coverage metadata. This discovery pass supplied leads across the collection.

## Related concerns and shared sources

- [Query and context](../04-query-and-context/README.md) turns selected evidence into bounded operations and retrieves raw material when summaries omit needed details.
- [Knowledge formation](../01-knowledge-formation/README.md) studies the source-supported knowledge that retrieval consumes, and retains the [shared baseline research](../01-knowledge-formation/2026-09-29-baseline-research.md).
- [Knowledge maintenance](../02-knowledge-maintenance/README.md) distinguishes unsuccessful lookup from a genuine maintenance need.

These findings are research evidence, not an accepted product design. Combined costs, coverage and limitations are in the [collection index](../README.md).

Continued after the interview in [expanded navigation and provenance](../../expanded-exploration/04-navigation-and-provenance/README.md), with five local evidence views and a separate generated-wiki comparison. Historical experiments here remain unchanged.
