# Connections, candidate omissions and learned-method boundaries

[Concern index](README.md) · [Replay and records](experiments/README.md)

This study tests useful relationships and the boundaries around them. It does not ingest private histories or establish a production method-learning system. All conversation-like examples are explicitly authored. Public repository code is captured with commit/file evidence; six small Python executions are real local outputs, rather than invented successful runs.

## Frozen paired comparisons

The [fixture file](experiments/cases.json) contains **30 unique authored pairs**, six per family: same entity/project, decision consequence/revision, support/contradiction, common method and speculative analogy. Twenty-five development pairs and five holdout pairs were written before provider evaluation. No prompt tuning preceded the first run, and no case was repeated for a statistical count. Each Jev request asked a relation Choice and independently useful Noul over the same state. GPT-4.1-mini received the identical pair texts and label definitions, in one development batch and one holdout batch, with labels, usefulness and exact supporting quote requested. The conservative deterministic baseline performs explicit repo-token identity checks and otherwise abstains into the family's insufficient/silent/low-value label. It is intentionally narrow; this is not a comparison against a strong retrieval or entailment engine.

| Engine | Development agreement with original labels | Holdout agreement with original labels | What the comparison can establish |
| --- | --- | --- | --- |
| Jev, generic relation wording | 22 / 25 | 3 / 5 | Most small scoped relations can be selected, but generic identity wording is inadequate. |
| GPT-4.1-mini, batched generative judgment | 25 / 25 | 4 / 5 | A generative baseline is competitive here; its output/citation and provenance duties need independent validation. |
| Conservative deterministic checks/abstention | 8 / 25 | 3 / 5 | Exact IDs answer some identity cases cheaply; semantic relationships need other evidence/judgments. |

These are **agreement counts**, not validated real-world accuracy. In particular, `method-hold_boundary` was labeled shared method although its text does not supply successful outcomes required by the chosen label definition. Jev chose insufficient and GPT chose unrelated; withholding shared-method is defensible. The original label and outputs are preserved and [the audit](experiments/audit.json) marks the case ambiguous. Its illustrative connection still merits considering as an unverified method hypothesis. The bare duplicate Phoenix names' usefulness label is also ambiguous: suppressing that connection can be useful clutter control. No aggregate usefulness accuracy is claimed.

Three Jev identity mistakes occurred in development: an explicit alias and distinct canonical repos were marked uncertain, while two bare Phoenix names were marked same. A **post-hoc diagnostic**, with the exact same six identity fixtures but a precise “same individual entity” question plus identity evidence rules, agreed on all six. This supports question-design diagnosis, not an independent success rate. The alias had only 0.59 same probability and 0.38 confidence even after fixing the question. A workflow that conflates argmax with established identity would still be too aggressive. Raw initial and diagnostic records remain distinct. [Initial records](experiments/results.json), [diagnostic records](experiments/diagnostic-results.json).

Jev labeled the held-out spice-cabinet alphabetical parallel plausible although it offered no demonstrated practical improvement to the wiki. Its **independent usefulness was 0.15**, so an illustrative 0.5 usefulness filter would remove the clutter. This shows why relation plausibility and value are different judgments. It does not calibrate 0.5 as a product threshold. The two deliberately misleading analogies—winter leaves as access restoration and a closed door as project abandonment—were rejected by the initial relation labels. Speculative ideas were never treated as causal evidence.

All 30 generated supporting quotes were found as exact substrings in the provided left/right text. That establishes quote fidelity in this small corpus, not truth, full-context support, uniqueness or recoverable character offsets. The exact quote exists check is a useful deterministic safeguard, while modality/context still needs inspection.

## Candidate generation is a separate failure surface

A second experiment flattened the 30 pairs into **60 independently identified source nodes** and offered a ten-edge budget. Token Jaccard selected the ten most overlapping pairs. GPT-4.1-mini proposed up to ten useful cross-source connections with ID references, explanations and statuses. Jev then reviewed the generated candidates for support and utility. This tests a different pipeline stage: no semantic review can select a useful pair that acquisition never offers. [Candidate runner and raw results](experiments/candidate-results.json).

Against fourteen known useful original pairs (excluding uncertain identities and explicit different-entity pairs), lexical acquisition surfaced **4 / 14** and generative acquisition **8 / 14**. Neither budget can recover all fourteen, so these are bounded lower-bound recall checks. Six lexical and two generative pairs lay outside that reference set; they are not automatically false, because there is no exhaustive pairwise labeling of all 1,770 possible edges. The reference set also retains the audited method case. The richer candidate budget traded coverage and clutter differently; these numbers do not establish corpus-scale recall or superiority across languages/formats.

The lexical top hit was the two identical Phoenix names, exactly where identity evidence was missing. Several later hits linked source-hash decisions with other decision text because of wording. It missed consequential and contradictory pairs that broad pair evaluation could judge correctly. The generator missed the quoted-deletion contradiction, a scope-specific method exception, the historical index change, the bounded decision consequence, the unverified OCR/chat method parallel and the diagnostics/citation structural analogy. These are **candidate omissions**, not Jev semantic mistakes.

Three generated edges illustrate remaining risks:

- The two Atlas projects were described as a “contradiction,” although the texts assert distinct projects rather than incompatible claims. Jev support 0.44/usefulness 0.23 would suppress this particular proposed relation under an illustrative 0.5 policy. An identity-difference record may still be worth storing to avoid a later merge.
- The generator asserted that `alice/garden`'s independently maintained fork came from the named `bob/garden` upstream. The text never explicitly identifies the fork's parent. **Jev support 0.82 accepted this unproved target association.** The broad known pair label was different entities; it did not license this extra specific claim. This is a candidate-specific semantic support failure, rather than candidate omission.
- The plant/note analogy had type “speculative” but status “tentative.” Jev support 0.87/usefulness 0.53 did not enforce status consistency. Code must preserve original constructed-source provenance and label a structural analogy speculative. Confidence cannot turn it into recorded experience.

No speculative link was installed or written into a user's wiki. These suggestions remain experiment records. An inference controller should measure candidate coverage, relation support, source specificity, value and status consistency separately; a single success label conceals distinct failures.

## Source-derived quotation diagnostic

The pinned research-pipeline `source_support.py` uses a normalized substring helper to bypass inference. We extracted and executed its two pure functions via Python AST, without installing the application or executing its network/store code. The six **new** diagnostic fixtures cover a positive assertion, negation, a hypothetical passing run, quoted attack text, an open question and a past version. All six contain the exact claimed sentence. The helper returns support on **6 / 6**, including the five context-sensitive negative/revision cases. This tests the helper with general text inputs; it intentionally bypasses Claim/Unit and upstream claim detection.

The repository's Claim type is a verbatim accepted Unit tied to source hash/span. Therefore the helper test is **an adaptation-precondition warning, not an end-to-end repository bug report**. Importing a convenient “quote found means supported” shortcut into arbitrary conversations loses the distinction between occurrence and adoption/assertion. [Pinned helper](https://github.com/shimo4228/jev-research-pipeline/blob/6dbac2bcf4221908cc16c934093cb1c52b4666ef/src/jev_research_pipeline/jev/source_support.py), [upstream Claim/Unit boundary](https://github.com/shimo4228/jev-research-pipeline/blob/6dbac2bcf4221908cc16c934093cb1c52b4666ef/src/jev_research_pipeline/model/nodes.py#L230).

Jev agreed with five of six diagnostic labels, missing the positive assertion by choosing silent (0.61 versus support 0.38). The prompt's “current actual fact” wording may have conflated source assertion with empirical verification; the failure is retained, and the study does not tune it away. GPT agreed with five of six, choosing compatible for the hypothetical run instead of silent. Both preserve the essential distinction that the run's success is not established. This is a six-fixture development diagnostic, not a holdout benchmark.

One local AST compilation failed because the newly created future-import node lacked line numbers. `ast.fix_missing_locations` corrected it **before any provider call**. That record is categorized as experiment code error, not service/model failure. [Control failure record](experiments/control-failures.json).

## Rich demonstration: three histories, failures, outputs and an exception

The [method-demo input](experiments/method-demo.json) contains three distinct **constructed coding histories** and a fourth unresolved-question history. Their messages point to actual failed/corrected source files and execution output. This is controlled coding evidence, not a reconstruction of the user's past conversations and not a real public issue conversation. Public implementation patterns above contextualize the demo without supplying invented historical participants.

| Constructed thread | Failed implementation and actual outcome | Corrected implementation and actual outcome | Bounded knowledge supported |
| --- | --- | --- | --- |
| CSV customer identifiers | `str(int(x))` collapses `007` and `7`; the distinct-ID assertion fails, exit 1. | Return the original string; the invariant passes, exit 0. | Preserve padding for identifiers declared text. Numeric quantities may have different semantics. |
| URL occurrence key | Key excludes query; `?view=all` and `?view=mine` collapse, assertion fails, exit 1. | Include original query in the key; invariant passes, exit 0. | These two distinct query destinations are distinguishable. No actual URL was fetched; `example.invalid` is a controlled placeholder. |
| Cache scope | Case-folded text alone collapses tenant A/B and policy v1/v2; assertion fails, exit 1. | Partition key by tenant and policy before text normalization; both invariants pass, exit 0. | These exact tenant/policy cases retain separation. This is not a complete cache authorization design. |
| Signed URL question | No signing specification or verification output. | No correction was executed. | Sorting query bytes remains unresolved; do not generalize the passing URL fixture to it. |

All six executable source files and their raw stdout/stderr/exit codes live in [coding-demo](experiments/coding-demo/). The reusable candidate is **preserve the exact identity/scope boundary, reproduce a collision, correct within that boundary, run its invariant, then use semantic judgment where compatible**. The failed attempts are essential evidence: dropping them would hide the motivation and make “normalize first” look equally justified.

Jev selected that candidate over normalizing everything, increasing timeouts or sorting all queries. It selected the bounded claim over universal support, assigned 0.97 to distinct successful recurrence within the three authored histories, and only 0.04 to signed-query verification. These judgments describe the controlled records; they do not infer that the user has repeated this method in real projects. Exact thread count and actual exit codes remain deterministic facts.

The generative draft retained the same scoped method, all three exceptions and the signed-query unresolved question. Every generated message ID exists. However, it **omitted explicit constructed-history provenance** despite the instruction to preserve actual versus authored status. A readable draft with valid references can still mislead when copied out of its surrounding experiment. Source-status inheritance should therefore be a host invariant, rather than a generative promise. [Full draft and Jev outputs](experiments/results.json).

This demo shows a useful method candidate plus limitations. It does not establish that six tiny invariant runs train a universal skill, that an unfamiliar format is supported, or that a high-probability nomination is permission to install the method. The rehearsal-bench proposal can test reproducibility on genuinely unseen histories before any activation design is accepted.

## Exact observed usage and practical limits

| Engine | Successful API requests | Input tokens | Output tokens | Reported cost (USD) | Median request latency |
| --- | --- | --- | --- | --- | --- |
| `typesafe/jev-1.13-20260917` | 53 | 27,908 | 3,161 | $0.001172136 | 0.258533 s |
| `openai/gpt-4.1-mini` | 5 | 6,328 | 2,673 | $0.006739920 | 4.271520 s |
| Combined | **58** | **34,236** | **5,834** | **$0.007912056** | Different batch/task shapes; no pooled latency claim |

There were no failed provider requests or retries. All successful responses include provider usage.cost. Latencies are elapsed request times, include network overhead, and do not measure whole product workflows; Jev pair calls were concurrent, while diagnostics/candidate reviews and generative batches had different scheduling. Jev sums to 15.114440 request-seconds and GPT to 24.137375; sums are not wall-clock runtime. The full model IDs, usage and raw replies are preserved. The ledger excludes coding-agent costs and failed-code execution overhead; provider-reported costs do not reconcile the account's eventual bill. [Audited ledger](experiments/audit.json).

The next discriminating work is not more calls on these fixtures. It is a frozen corpus of independent real histories with source-linked successful/failed outputs, human-rated useful connections, missing-candidate measurement and explicit method applicability. The five proposed experiences identify falsifiable tasks for that next investigation. Tool retention, source/message granularity, automatic promotion, speculative defaults and navigation architecture remain unresolved.
