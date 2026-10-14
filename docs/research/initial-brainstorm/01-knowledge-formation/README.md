# Knowledge formation

[Initial brainstorm](../README.md) · [All research](../../README.md)

Question: how can small semantic judgments turn raw conversations and selected information into source-supported candidate knowledge?

## Scope

Conversation boundaries and antecedents, explicit and implicit decisions, evidence coverage, assertion relationships, entity identity, candidate topics and exact source spans. The focus is interpreting incoming material and representing what it supports. Ongoing note refresh and procedural nomination live in [knowledge maintenance](../02-knowledge-maintenance/README.md).

## Research documents

- [Baseline research and first 57 requests](2026-09-29-baseline-research.md): shared feasibility study, TypeSafe cookbook map, initial architecture ideas and open design questions. Its mixed baseline remains intact here.
- [Formation experiments](2026-09-29-formation-experiments.md): 96 frozen synthetic fixtures, 192 main calls and 16 targeted diagnostics covering conversational references, distributed evidence, assertion lifecycle, identity, candidate topics and exact spans.

## Experiment artifacts

Scripts, fixtures and recorded results live in this concern's [experiments](experiments/) directory.

- [Baseline Jev wiki probes](experiments/baseline-jev-wiki/README.md): the original mixed feasibility runs, preserved together for audit and replay.
- [Formation lab](experiments/formation-lab/README.md): composition experiments, raw replies, costs, offline policies and code invariants.

## Related concerns and shared sources

- [Knowledge maintenance](../02-knowledge-maintenance/README.md) applies formation judgments to durable notes, changing sources and recurring procedures.
- [Retrieval and navigation](../03-retrieval-and-navigation/README.md) finds and assembles the evidence used to answer questions.
- [Query and context](../04-query-and-context/README.md) covers source freshness and the recovery of information hidden by summaries or context selection.
- [Five-catalog repository landscape](../03-retrieval-and-navigation/2026-09-29-repository-landscape.md) is the shared repository inventory for the collection.

These findings are research evidence, not an accepted product design. Combined costs, coverage and limitations are in the [collection index](../README.md).

Continued after the interview in [expanded source interpretation](../../expanded-exploration/01-source-interpretation/README.md) and [actual agent histories](../../expanded-exploration/02-agent-histories/README.md). Historical experiments here remain unchanged.
