# Initial brainstorm

[All research collections](../README.md)

Research date: 2026-09-29. This is a research collection and a capability proposal, not an accepted product architecture. Four agents originally investigated formation/maintenance and retrieval/graph topics. Their findings are now organized into four concerns, with each report and its experiment suite kept together. Every concern has a local `experiments/` directory.

The later [expanded exploration](../expanded-exploration/README.md) builds on this collection after the completed interview, adding real histories, public media, conversation URL linking, richer method demonstrations and navigation/prose comparisons. The experiment records and totals below remain the original study's evidence.

## Four concerns

| Concern | Focus and completed investigation | Local artifacts |
| --- | --- | --- |
| [Knowledge formation](01-knowledge-formation/README.md) | [208 calls: conversation, evidence, identity, lifecycle and topics](01-knowledge-formation/2026-09-29-formation-experiments.md), plus the original mixed baseline | [experiments](01-knowledge-formation/experiments/) |
| [Knowledge maintenance](02-knowledge-maintenance/README.md) | [160 calls and 11 pinned repository inspections: maintenance, dependent-note refresh, source acquisition and procedures](02-knowledge-maintenance/2026-09-29-repository-deep-dive.md) | [experiments](02-knowledge-maintenance/experiments/) |
| [Retrieval and navigation](03-retrieval-and-navigation/README.md) | [257 calls: broad recall, complementary evidence, graph walks, clarification and reusable features](03-retrieval-and-navigation/2026-09-29-retrieval-experiments.md), plus repository discovery | [experiments](03-retrieval-and-navigation/experiments/) |
| [Query and context](04-query-and-context/README.md) | [100 successful calls and 10 pinned repository inspections: evidence freshness, query operators, raw-source drilling and context retention](04-query-and-context/2026-09-29-repository-deep-dive.md) | [experiments](04-query-and-context/experiments/) |

The folders separate the primary concerns of the existing studies. Individual studies retain their cross-cutting examples and original records; related findings are linked from the concern indexes.

The [original research and 57-call baseline](01-knowledge-formation/2026-09-29-baseline-research.md) remains intact. The [five-catalog repository landscape](03-retrieval-and-navigation/2026-09-29-repository-landscape.md) preserves the complete 1,077-repository inventory and its coverage distinctions. The retrieval folder also contains an explicitly labeled extract of the two original retrieval probes; that extract is not counted as a new run.

The two repository-led passes inspected pinned implementation snapshots from **18 distinct repositories** (11 formation snapshots and 10 retrieval snapshots, with three overlaps). Their manifests identify the exact files inspected; this is selected implementation research rather than an exhaustive audit of every file in those repositories.

## What using more of Jev could mean

The broader research supports a product in which semantic judgments participate throughout the knowledge lifecycle. Small decisions can be composed into behaviors that are more useful than any single label:

| Potential capability | Composition to explore | Evidence and boundary |
| --- | --- | --- |
| Understand short replies in their actual context | Episode relationship + antecedent selection + preserved source IDs | Formation lab separates topic continuity from adoption. Quoted instructions and implicit references still cause mistakes. |
| Maintain a changing wiki | Source dependency lookup in code + semantic impact judgment + version-checked proposal | Repository-inspired impact tests matched all 120 note-level labels across prompt variants/repeats; these reuse ten synthetic scenarios. |
| Learn recurring procedures | Exact distinct-session/outcome counts + semantic recurrence/exception judgment + original command evidence | Memsearch and Jev-Mem provide source patterns. Experiments support candidate nomination but expose recurrence-label ambiguity. |
| Build entity and topic views | Candidate identity/topic choices + independent membership signals + code clustering invariants | Topic selection matched 32/32 labeled observations; explicit identity wording materially improved a weak companion Noul. Neither result establishes unrestricted ontology discovery. |
| Find indirectly relevant constraints | Broad or hybrid candidate acquisition + per-source relevance/directness | On the miniature corpus, broad Choice answered or abstained correctly on 8/8 queries; three shortlist failures came from omitted evidence. |
| Assemble complementary evidence | Judge the next candidate against already selected evidence and explicit missing facets | Six bundle selections matched expected sets. Global relevance alone does not express marginal contribution. |
| Drill from a summary into raw evidence | Inspect missing scope/qualifiers + choose a known source + rejudge fetched material | Tiered memory is a concrete repository pattern. Distinguish useful abstention from disagreement with facts the model never received. |
| Query notes through a small set of operations | Select a permitted operator + branch-specific argument questions + code validation | Read, compare, follow and select form a small experimental vocabulary. The experiments simulate binding over fixture dictionaries; they do not implement a production query engine. |
| Change views without rerunning inference | Retain independently meaningful Scores/Nouls + local filters and weights | Planning, implementation and evidence-audit views reuse stored judgments at zero additional inference cost. No human preference ranking was measured. |
| Explore a graph toward a question | Choose among observed edges + bounded branching + explicit evidence-facet checks | The graph tests exposed wrong branches and premature completion. This remains a research direction, not a validated controller. |
| Keep useful agent context | Preserve required provenance/protocol in code + judge optional context by task constraints | The repository work distinguishes topical relevance from constraints, source qualifiers and message/tool dependencies. |

## Lessons that changed the initial recommendation

1. **Semantic judgments are reusable data.** Preserve the question meaning/version, source revision, raw output and probabilities. Multiple views and policies can then be evaluated locally without paying for another judgment over unchanged evidence.
2. **Clear questions can outperform extra gates.** In the identity diagnostic, explicitly asking whether named `left` and `right` are the same entity improved agreement from 11/16 to 15/16. The initial underspecified Noul reduced the usefulness of an otherwise accurate Choice. More questions are useful when they answer independently necessary questions.
3. **Available evidence and hidden truth require different metrics.** A cached-only question cannot establish an unseen current revision. An omitted search candidate cannot be recovered by reranking. Preserve abstention and diagnose retrieval gaps separately from semantic errors.
4. **Planning and completing a task are separate problems.** Choosing a promising link or useful next source worked more reliably than deciding that an entire goal was complete. Required evidence facets, exhausted search and budget stops need distinct representations.
5. **A source module carries caller assumptions.** Several attractive implementations depend on upstream quote proof, input filtering, source hashes or protected protocol messages. Reusing only the Jev prompt can discard those assumptions.

An ambitious next prototype could combine imported source revisions, a derived semantic judgment index, evidence-linked Markdown proposals, and a query interface that retrieves complementary evidence and drills into raw history. The graph, editor, automation boundary and generative drafting choices remain open for the design interview.

## Observed inference ledger

The four investigations recorded **725 successful SDK evaluations** and **five initial failed evaluations**, all five subsequently recovered by explicit retries with the original records preserved. The successful responses consumed **434,717 input tokens and 52,515 output tokens**, with **$0.018258114 in reported cost**. Every successful SDK response resolved to `typesafe/jev-1.13-20260917`.

The separate [repository-discovery run](03-retrieval-and-navigation/experiments/repository-discovery/README.md) submitted 335 public README cards; its CLI reported 306 scored cards, 29 unjudged IDs and **$0.004334190**. It does not expose individual API responses/model IDs, so it is outside the 725-evaluation ledger. Combined observed reported cost for this research expansion is **$0.022592304**, about **2.26 US cents**. Billing for failed responses is unavailable; these are observed costs, not a complete account billing reconciliation. Coding-agent costs are not included. The original baseline is excluded from these expansion totals.

## How to interpret the results

These are mostly short, authored synthetic examples with descriptive source names and small, sometimes deliberately weak baselines. Repeated fixtures and paired variants are correlated observations. The formation experiment reserves 24 unedited synthetic holdout fixtures; most other work is development evaluation. Prompt refinements, post-hoc policies and label ambiguities are explicitly separated from frozen results. There is no single pooled accuracy metric because the studies measure different behaviors.

No private conversation corpus was sent. Downloaded repository code was inspected as source, not executed. The reports link exact implementation evidence and distinguish tested compositions from untested ideas. Scripts, fixtures, raw successful replies, error records, cost/usage, source snapshots, label audits and replay instructions live under the four concerns' local experiment directories. The folder reorganization did not rerun inference or modify recorded experiment data.
