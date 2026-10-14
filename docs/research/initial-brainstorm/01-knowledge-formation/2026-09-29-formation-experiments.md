# Formation experiments: composing bounded judgments into evidence-preserving workflows

Date: 2026-09-29. Research evidence, not an accepted architecture or glossary decision.

The useful capability is larger than a classifier: Jev can select episode relationships, assertion relationships, exact source spans and candidate topics; code can combine those judgments into reversible source-backed proposals. This experiment also found a concrete reason to resist adding more semantic gates automatically: an underspecified companion Noul rejected explicit aliases that the more fully specified Choice recognized correctly. Changing the question improved that signal, while a policy using the original Choice distribution already preserved all eight labeled identity-positive observations without false merges in this small sample.

## Reproducible artifacts and provenance

All artifacts are in this research track's [formation-lab experiment directory](experiments/formation-lab/README.md). [run.py](experiments/formation-lab/run.py) defines all fixtures and prompts; [fixtures.json](experiments/formation-lab/fixtures.json) is the frozen request specification; [results.json](experiments/formation-lab/results.json) preserves full states, questions, probabilities, raw provider usage, model and latency. [analyze.py](experiments/formation-lab/analyze.py) computes offline policies, baselines, alternate views and deterministic invariants; [analysis.json](experiments/formation-lab/analysis.json) preserves every label disagreement. [summary.json](experiments/formation-lab/summary.json) provides compact machine-readable counts. [diagnostic.py](experiments/formation-lab/diagnostic.py) and its [separate results](experiments/formation-lab/diagnostic-results.json) are explicitly post-result development evidence.

Only authored synthetic inputs were sent. No private chats or credentials appear in fixtures or results. Python SDK 0.7.2 used an explicit environment key, OpenRouter base URL and requested `typesafe/jev-1.13`; every main response resolved to `typesafe/jev-1.13-20260917`. Raw successful HTTP responses were captured through SDK client hooks, so cost is measured rather than inferred from pricing. No failed requests occurred.

The main run contains **96 unique fixtures**, each evaluated in two variants: **192 successful API requests, 712 typed questions, 256 predeclared Choice checks**. Each family has 12 development fixtures and four holdout fixtures. The 24 holdout fixtures change domain to museums, archives and photographs; all labels and questions were authored before any live response, with no tuning between development and holdout. They are a true unedited synthetic holdout for this run, not an independently collected or representative corpus. Variants and related cases are dependent observations, so 48 holdout calls are not 48 independent examples.

The second variant appends unrelated maintenance prose and a quoted instruction attack; sources, spans and topic candidate mappings are reversed where available. This is one combined distractor/ordering intervention: it cannot isolate their separate effects. Message order and chronological dates are preserved. The second run reuses entity fixtures with revised question wording and is not fresh holdout validation.

## Primary-source patterns inspected

The live [TypeSafe index](https://docs.typesafe.ai/llms.txt), [Python SDK guide](https://docs.typesafe.ai/sdk/python), [Score reference](https://docs.typesafe.ai/primitives/score), and [composite scoring pattern](https://docs.typesafe.ai/patterns/composite-scoring) informed batching and reuse. The attempted OpenRouter compatibility page returned HTTP 403 via direct HTTPS in this session; provider configuration follows the explicit local skill configuration and previously validated SDK setup, with wire responses confirming actual usage.

Three related cookbooks were inspected directly:

- [Entity alignment](https://docs.typesafe.ai/cookbooks/entity_alignment) combines an identity score with field checks and separates merge, review and leave-unlinked outcomes. Here, individual identity is deliberately distinguished from name similarity and pairwise judgments are subjected to a complete-link cluster invariant.
- [Citation checking](https://docs.typesafe.ai/cookbooks/citation_check) first locates exact quotes in code, then evaluates semantic support in context. Here, support is checked across several source snippets, including partial claims and unresolved counterevidence; exact source copying remains deterministic.
- [Structure recovery](https://docs.typesafe.ai/cookbooks/autoformat) batches adjacent-line judgments and block roles while preserving line IDs. Here, conversation episodes and knowledge roles are judged separately, so sharing a topic cannot by itself establish an inherited assent or an adopted decision.

These are design inspirations, not transferred accuracy claims. The linked cookbooks' thresholds and results do not validate thresholds for Knowledge Garden.

## Six hypotheses and results

| Composition / hypothesis | Requests | Frozen Choice matches | Holdout matches | Measured main-run cost |
| --- | ---: | ---: | ---: | ---: |
| Episode continuity plus separately resolved antecedent | 32 | 58/64 | 16/16 | $0.000762888 |
| Full assertion coverage across multiple sources plus per-source support signals | 32 | 31/32 | 8/8 | $0.000813036 |
| Semantic replacement/retraction plus date ordering computed in Python | 32 | 26/32 | 6/8 | $0.000763812 |
| Selective entity identity plus clustering invariants | 32 | 30/32 | 8/8 | $0.000663684 |
| Bounded topic candidate selection plus independent topic membership signals | 32 | 32/32 | 8/8 | $0.001001196 |
| Exact span roles plus separate adopted-decision signals | 32 | 48/64 | 10/16 | $0.001111992 |
| **Total** | **192** | **225/256** | **56/64** | **$0.005116608** |

### 1. Episode windows can carry antecedents, but a boundary is not evidence

Cases include unbound assent, a wrong preceding proposal, multiple options, scoped switches, disagreement, quotations, delayed selection and different-domain holdout. The composition asks both whether a turn continues the same episode and whether the preceding supplied proposal resolves an implicit reference. An episode can continue while the antecedent remains absent: “Yes, add caching” after “Redis or local dictionary?” does not select Redis. Both variants correctly select no antecedent for that case and for unbound assent.

The six disagreements concern two debatable episode labels and one clearer antecedent miss, each repeated across variants. “Rename the button?” followed by “Yes, use SQLite” was predeclared same but returned new; changing from wiki to payroll was predeclared new but returned same. Those expose a missing definition of task versus topic continuity. “No, that would not work” returned no antecedent even with the exact previous proposal present; the rejection's reference should resolve. The fixed labels are retained rather than silently repaired.

Reusable capability: episode proposals and inherited context links can be presented independently. A later extractor can include the exact antecedent span when a short user reply would otherwise be meaningless. This study does not implement a whole-conversation segmentation algorithm or measure long-distance antecedent recovery.

### 2. Distributed evidence works; quotation can defeat full-claim support

The model recognized support when one source states the selected database and a second supplies its reason, with no single source containing the entire claim. Both variants were correct. All clauses matter: missing benchmark reasons, conditional adoption, wrong project, assistant-only assertions, and overbroad generalizations returned unsupported. Directly conflicting records returned mixed rather than choosing whichever source appeared last in the mapping.

The consequential failure is the quoted source: `A tutorial says "We selected SQLite"` supporting the candidate `The user selected SQLite`. Clean input returned unsupported with 0.57 option probability; the distractor/order variant returned supports. The provenance and actor contract exists in the prompt but the model still promoted quoted content. Its companion full-support Noul was above 0.5, so a bare majority gate did not prevent the error.

A fixed threshold sweep is reported rather than selected as a proven optimum. Choice=supports AND full-support Noul>=0.8 produced two actions across 32 calls, no false actions, and missed four of six supported observations. Using Choice's supports probability>=0.9 produced all six supported observations and no false actions. This sample favors the simpler fully specified signal; it does not establish general calibration or write authority.

### 3. Assertion lifecycle belongs to semantics plus deterministic time

Fixtures separate explicit replacement, withdrawal, paraphrase, optional future changes, another project, an index coexisting with authoritative files, assistant suggestions, quotes, earlier replacement dates and equal dates. Python parses ISO dates and computes newer/same-date booleans. The semantic question is explicitly told not to compare dates. The action policy requires a strictly newer record plus explicit replacement or retraction.

The six raw relation disagreements are compatible versus unrelated for different scopes or complementary storage roles. These are label-boundary disagreements; none is a false retirement action. At explicit-Noul>=0.8, the policy takes all eight intended replacement/retraction actions across the 32 observations with zero false actions. Earlier and equal-date records cannot retire the old assertion even if semantic replacement is correctly recognized. At 0.9 it takes six actions and misses two. This is concrete evidence for code-owned temporal ordering and separately retained assertions, rather than asking the model to decide a single current truth.

Date arithmetic is limited to complete ISO dates here. Relative dates, uncertain timestamps, branch histories and source revision precedence are untested.

### 4. Identity is selective; composing probabilities needs care

Explicit aliases, renames and canonical repository identifiers mostly match, while components, forks, model family versus release, deployment versions, ambiguous names and distinct owners remain separate. The two false same labels are the same ambiguous bridge fixture under both variants: an unspecified “Nick” is compared with “Nicolas, nicknamed Nick, at repo a/garden.” The supplied alias establishes the second description's nickname; it does not identify the first bare Nick. Choice same probability is 0.50, with 0.49 uncertain, so distribution concentration exposes this ambiguity.

A Choice same-probability>=0.8 policy makes eight true proposals and zero false proposals across 32 observations; >=0.9 makes seven and misses one. By contrast, the original companion Noul says 0.43 for the explicit KG alias while Choice says same with probability 1.00. At a 0.5 companion threshold the AND policy takes two *false* actions and misses all six development positive observations. A very conservative Noul gate yields no actions; zero false actions then means zero useful coverage, not successful identification.

The 16-call diagnostic repeats entity state with three independently batched Nouls. Original wording at >=0.5 matches 11/16 labels; an explicit question naming `left` and `right`, alias/rename/identifier criteria and exclusions matches 15/16; inline descriptions match 13/16. Named wording removes false negatives but retains one false positive. This supports better question contracts over blindly multiplying signals, and does not prove the original issue was an SDK, model or reference-resolution bug. The Choice question had substantially richer identity criteria than the original Noul; that is a material confound.

`complete_link()` is a separate deterministic composition: clusters merge only if **every cross-pair** passes a supplied probability threshold. Its counterexample gives A–B=.99, B–C=.99, A–C=.10; the output retains C separately. These probabilities are constructed test input, not a claimed model-produced non-transitive triple. An absent pair blocks merging. This prevents unsupported transitive identity expansion; it does not make incorrect pair judgments true.

### 5. Candidate topics can be selected without inventing an ontology

Four provisional research topics and a no-match outcome are supplied in state. The model selects a primary topic, asks independent membership Nouls so multiple topics may apply, and supplies a reuse-quality Score. All 32 primary selections match fixed labels, including wrong-topic negatives, classifier attacks, archive holdout and a multi-topic span whose first substantive topic is storage. The naive keyword baseline matches 13/16 clean fixtures, versus 16/16 model primary selections.

This demonstrates candidate **selection**, not discovery of new ontology terms. The model cannot name an omitted category. Membership probabilities and usefulness Scores are stored but not separately pre-labeled or calibrated in this run. Novel terms would need candidate creation from observed source spans or a different generative process, followed by coverage testing and user discussion.

### 6. Extractive selection preserves spans, but role definitions need work

Two exact spans per fixture receive independently batched role Choices and decision Nouls. Roles allow a combined decision+reason within one original span; code must never manufacture a new sentence by recombining incompatible fragments. Original text is retained under stable source IDs, and the offline audit checks exact substring copying.

Raw accuracy is 48/64. Many disagreements are **reference-label problems**, not clear semantic failures: the predefined observation role includes reported facts, but labels call “It is sunny,” “The hallway is blue” and “No result yet” none. Model observation is defensible under the written criterion. An “Example of malicious content” line being quotation is likewise ambiguous. These are preserved and identified because changing labels after seeing results would inflate accuracy.

A more meaningful miss is attributing a handbook recommendation to proposal rather than quotation in holdout. The distinction between an actor's suggestion and quoted source advice needs stronger source attribution. The distractor variant also turns “No decision made” into decision rather than observation, creating an explicit unsafe role promotion. An adopted-decision role cannot itself authorize a filesystem write; exact source evidence and scope must accompany any proposal.

## Reusable scores, baselines and invariants

The offline analyzer generates four ranked evidence views using different weights on stored support-quality Score and full-support Noul. This costs **zero further inference**. The top three remain direct supported cases, while lower rankings change; the quoted case appears highly enough to illustrate why relevance rankings need a separate eligibility contract. Weighted scores cannot compensate for fabricated scope or resolve conflicting records.

Minimal string baselines on 16 clean fixtures each match 10/16 coverage labels, 8/16 identity labels and 13/16 topic labels. Model clean results are 16/16, 15/16 and 16/16 respectively. These baselines are intentionally weak controls: no embedding, BM25, generative extractor or real-corpus comparison is claimed.

Seven deterministic assertions pass: complete-link bridge exclusion, stable sorted ordering, missing-pair merge prevention, date crossing a month boundary, same-date retirement exclusion and two exact-copy span checks. These check critical composition properties. They are not substitutes for testing an importer, duplicate-source handling or Markdown patch application, none of which is implemented here.

## Cost, latency and limits

Main run: 121,824 input tokens, 21,344 output tokens, median request duration 0.364 seconds, 15.858 seconds wall with up to six concurrent calls. Diagnostic: 16 requests, 48 Noul questions, 6,490 input and 832 output tokens, 2.672 seconds wall, $0.000272580. Combined: **208 successful requests, 760 typed questions, 128,314 input + 22,176 output tokens, $0.005389188** measured wire cost. The diagnostic has no frozen Choice checks and must not be added to the 225/256 figure.

Only two paired Choice outcomes changed under the combined perturbation: the quotation support verdict and the “No decision made” span role. This is evidence of mostly stable bounded outputs on these short inputs, alongside two material promotions. No long-context or input-size scaling test was performed. Parallel latency is machine/provider/concurrency specific; it is not a production throughput promise.

This work identifies several reusable components worth further investigation: inherited evidence links, full-clause source coverage, semantic lifecycle events gated by code dates, selective alias proposals protected by complete-link clustering, bounded topic membership, and exact span selection with source preservation. A real next evaluation should resolve ambiguous label definitions first, then test these components on a separately labeled, explicitly selected corpus and measure unsupported promotions and useful proposal coverage together. No production system, accepted glossary terms or ADRs were created.
