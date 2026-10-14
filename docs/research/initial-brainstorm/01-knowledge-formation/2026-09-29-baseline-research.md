# A personal Markdown wiki: research and live Jev experiments

Research date: 2026-09-29. Status: evidence and design proposal, not an accepted architecture or implementation specification.

## Starting point

The requested product is a personal wiki / knowledge graph on the user's computer, using Markdown files, ingesting coding-agent chats and other explicitly supplied information. The repository currently contains agent guidance and empty domain-documentation scaffolding; no product code or agreed domain terms existed when research began. The user requested live TypeSafe experiments using the configured OpenRouter key and research of five linked-repository catalogs before a design interview.

My recommendation for discussion is an evidence-preserving import-and-review loop. Keep the original source material, derive proposed knowledge from identifiable source spans, and surface suggested Markdown changes and links for review. Search should find useful passages even before all those passages have been turned into curated pages. This is a design inference from the experiments below, not a decision already made by the user.

## Primary-source cookbook map

The requested [console cookbook URL](https://console.typesafe.ai/docs/cookbooks) returned HTTP 403. The live [TypeSafe documentation index](https://docs.typesafe.ai/llms.txt) was accessible and pointed to the cookbook pages below. Markdown pages that the web tool could not open were retrieved directly over HTTPS. Thus the cookbook research was completed through the current public docs, not the inaccessible console page.

| Cookbook | Relevant pattern | Proposed application and limit |
| --- | --- | --- |
| [Entity alignment](https://docs.typesafe.ai/cookbooks/entity_alignment) | Judge candidate entity pairs and surface field agreement. | Suggest aliases and duplicate pages. Preserve an uncertain outcome; never equate a component with its parent or an independent fork with upstream. |
| [Semantic find](https://docs.typesafe.ai/cookbooks/semantic_find) | Select source IDs and independently assess whether an answer exists. | Return exact chat turns supporting a decision or answer. A ranked winner alone does not establish that the answer exists. |
| [Re-ranking](https://docs.typesafe.ai/cookbooks/rerank_typesafe) | Fast candidate retrieval followed by semantic judgments. | Rank a bounded lexical/embedding shortlist. Candidate retrieval recall remains a separate problem. |
| [Citation check](https://docs.typesafe.ai/cookbooks/citation_check) | Exact quote-presence checks in code, then semantic support checks. | Verify proposed wiki assertions against their cited context; distinguish source support from independent truth. |
| [Pre-parsed value extraction](https://docs.typesafe.ai/cookbooks/pre_parsed_value_extraction_cookbook) | Broad candidate discovery, bounded selection, exact copying and normalization. | Select a referenced repository, command, file, date mention, or message ID without generating the value. Missing candidates cannot be recovered by a selector. |
| [Structure recovery](https://docs.typesafe.ai/cookbooks/autoformat) | Judge ambiguous line joins and block roles; render original text in code. | Repair pasted plain text when its structure is genuinely missing. Existing Markdown and structured chat exports should use ordinary parsers. |
| [Extraction cascade](https://docs.typesafe.ai/cookbooks/sde_cascade) | Generate candidate structured content, verify fields, escalate difficult cases. | Optional later stage for fluent page drafting or open-ended concept extraction; the present experiments do not test a generative model. |

The cookbook examples use their own datasets, model versions, prices and routing thresholds. Their published success rates are not results for this wiki. [TypeSafe's building guide](https://docs.typesafe.ai/concepts/how-to-build-with-system-one) and [state guide](https://docs.typesafe.ai/concepts/state) support keeping execution in code and grouping independently answerable questions over relevant state.

## What intelligent judgment can replace

| Fragile approach | Bounded judgment | Code still owns |
| --- | --- | --- |
| Phrase lists such as “we decided”, “sounds good”, and “actually” | Is this proposed, accepted, rejected, superseded, or unresolved in its actual conversational context? | Source roles, turn order, IDs, scope metadata, evidence availability and write policy. |
| String similarity used as identity | Are these two mentions the same entity, different entities, or unresolved? | Candidate generation, stable IDs, alias records and reversible merge proposals. |
| Keyword overlap used as answer quality | Does this passage answer this question? How directly? | Search index, shortlist construction, access boundaries and exact source links. |
| Regex rules for negation and changing decisions | Do two assertions conflict within the same scope and time, paraphrase one another, or coexist? | Date ordering, version history and preserving both assertions. |
| Trusting a generated citation because a quote exists | Does its surrounding source actually support the full claim? | Exact quote and span validation; the model does not certify external truth. |
| Custom heuristics for malformed prose formatting | Is an ambiguous block a heading, step, command, prose or warning? | Known syntax, text preservation and Markdown rendering. |

Do not replace deterministic JSONL/JSON parsing, Markdown syntax parsing, hashes, deduplication by exact source ID, arithmetic, timestamp comparisons, filesystem operations or atomic writes with model judgments. The [Jev 1.13 limitations page](https://docs.typesafe.ai/model-jaggedness/jev-1.13) specifically discusses numeric precision, date comparison, indirection, irrelevant context and adversarial content. Jev does not generate prose; selecting source text and writing an original explanation are different capabilities.

## Live experiment method

Used TypeSafe Python SDK 0.7.2 with explicit `api_key=OPENROUTER_API_KEY`, `base_url=https://openrouter.ai/api`, and `model=typesafe/jev-1.13`, following the [OpenRouter compatibility guide](https://openrouter.ai/docs/guides/community/typesafe-sdk) and [TypeSafe Python guide](https://docs.typesafe.ai/sdk/python). All 57 successful calls returned resolved model `typesafe/jev-1.13-20260917`. All input was synthetic; no private user chat history was sent.

Fixtures and full request/response records live in [baseline experiment scripts and records](experiments/baseline-jev-wiki/README.md). Labels were authored before each run. Refinements were informed by observed failures, so this is a development study, not a held-out benchmark. Several expected labels were intentionally strict but remain debatable; raw disagreements are preserved instead of retrospectively changing labels.

| Run and task | Exact Choice checks matched | Interpretation |
| --- | ---: | --- |
| Initial: adoption status plus supporting user message | 20/24 across 12 cases | Includes a consequential false adoption and ambiguous subtype disagreements. |
| Initial: entity identity | 8/8 | Promising bounded pair judgments, including aliases, components, forks and insufficient identity evidence. |
| Initial: assertion relationship | 6/6 | Distinguished paraphrase, conflict, compatible scope/modality and unrelated content. |
| Initial: citation support | 7/8 | Remaining disagreement was unsupported versus contradicted; neither accepted the claim. |
| Refined: original adoption cases | 22/24 | Missing-antecedent false adoption fixed; two debatable non-acceptance labels still differ. |
| Refined: six new adoption variants | 12/12 | Included wrong/missing antecedents, multiple options, external quotation and later correction. Tiny development sample. |
| Refined: retrieval | 2/2 | Retrieved a paraphrased reason over keyword distractors and abstained on an absent answer. Companion Scores gave the answering passage 2.00/2 versus 0.98 for the keyword distractor. |
| Refined: unmarked block roles using numeric array paths | 3/5 | Misclassified the shell command as prose and the following explanation as warning. |
| Structure follow-up: named fields | 5/5 | Same text and labels, named paths instead of numeric array references. |
| Structure follow-up: explicit target text | 5/5 | Same text and labels, target text embedded directly in the question. |

### Consequential failure: hypothetical candidate becomes evidence

State contained candidate “Use SQLite for the local search index” and only one user message: “Yes, do that.” The initial model labeled it accepted with confidence 1.00 and selected the user message with confidence 0.98. There was no antecedent in the supplied conversation. A confidence threshold alone would not catch this.

The revised question explicitly says that the candidate is an unverified hypothesis, and pronouns must resolve inside supplied messages. It returned unknown (confidence 0.79), no evidence (confidence 0.57), and grounding Noul 0.11. The six new variants also matched their labels. This supports explicit evidence contracts and abstention, but does not prove the revised classifier is safe to grant automatic write authority.

### Other disagreements and diagnostic limits

The initial quoted malicious example was labeled rejected instead of unknown; the overly broad “every project” assertion was labeled rejected instead of unknown; and “no database choice is recorded” was classified as contradicting rather than merely failing to support an accepted-choice claim. These expose taxonomy boundaries and should not all be reported as equivalent factual failures. Both adoption cases selected no evidence. The citation attack did not produce a supports verdict.

The numeric-reference structure failure is consistent with reference-resolution trouble, but this experiment does not establish its internal cause. Named fields and explicit target text both fixed it on the same five blocks. Prefer named records, preserve context, and inspect the actual prompts instead of assuming a parser replacement is reliable.

The [Choice](https://docs.typesafe.ai/primitives/choice), [Noul](https://docs.typesafe.ai/primitives/noul), [Score](https://docs.typesafe.ai/primitives/score), and [confidence](https://docs.typesafe.ai/confidence) docs distinguish selecting an option, probability of a proposition, graded position, and concentration of a distribution. Our labeled scores measure Choice agreement only. Companion Nouls and Scores were inspected diagnostically; their calibration was not measured.

### Timing, usage and cost

| Run | Requests | Input/output tokens | Median request time | Whole-run time with up to four concurrent calls | Reported cost |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial | 34 | 17,924 / 2,340 | 0.286 s | 4.453 s | Unavailable in saved SDK serialization |
| Refined | 21 | 18,115 / 2,567 | 0.737 s | 4.460 s | $0.000760830 |
| Structure follow-up | 2 | 1,958 / 508 | 0.327 s | 0.381 s | $0.000082236 |

Total observed usage: 37,997 input and 5,415 output tokens. Known cost for the latter 23 calls: $0.000843066. This is not the total cost of all 57 calls: the first 34 lack recorded billing cost. The initial JSON's zero cost combined with `cost_available: false` denotes missing measurement, not free calls. The runner was amended to capture raw successful responses through the SDK HTTP client and emit null for missing cost on future runs.

These are short synthetic contexts and a single pass per prompt version. They do not establish whole-vault latency, scale, general accuracy, language coverage, provider reliability, or a monthly budget. No baseline comparison against embeddings, a generative extractor or full-text search over a real corpus was performed.

## Candidate initial architecture to discuss

1. Explicitly select a source or source collection for import. A format adapter reads structured chat messages, roles, timestamps and attachments where available. Other selected material enters through a text/Markdown adapter first.
2. Preserve an immutable imported source revision with stable origin and content hash. A conversation's branches and actual message order remain distinguishable; do not infer authority from its final assistant summary.
3. Propose useful source spans and page associations. Begin with extractive selection plus semantic judgments; add generative drafting only if polished synthesis is a required user outcome.
4. Retrieve a bounded shortlist of existing pages or entities, then judge aliases, relationships, contradictions and source support. Keep uncertain outcomes visible.
5. Present a reviewable set of new-page, link and revision proposals with source evidence. The user decides the future automation boundary during the interview.
6. Store accepted pages as Markdown. Represent approved links in a readable format and build disposable search/backlink indexes as needed. The file format, editor, ID scheme and graph representation are unsettled decisions.

Suggested first vertical slice, contingent on the interview: ingest a small selected set of coding-agent conversations; answer “what did I decide about this project, why, and where?” with exact source links; produce reviewable updates to one project page. Then test whether curated conceptual pages or agent retrieval is the more valuable next step.

Suggested validation after choosing a real source set: label accepted/proposed/superseded examples and adversarial edge cases; measure unsupported-promotion rate, missed useful evidence, duplicate-entity suggestions, accepted useful proposals and review time. Test interrupted import, repeated import, moved/edited notes and deleted source material with deterministic code. Do not adopt numerical model thresholds from this synthetic run.

## Linked repository research

The separate [five-catalog inventory and source inspection](../03-retrieval-and-navigation/2026-09-29-repository-landscape.md) records coverage, direct repository evidence and applicability. Catalog endorsements are discovery aids; recommendations should rest on the linked projects' own README and source, with maintenance/license/adoption checks appropriate to any later reuse.

## Open design tree for the interview

Already supplied by the user: personal use; on-computer Markdown files; coding-agent chats plus chosen other information; TypeSafe/OpenRouter experiments now.

Current independent frontier:

- Primary outcome: retrieve past decisions, maintain a readable synthesized wiki, feed future agents, or explore conceptual links. Pick the first outcome that makes the product worth using.
- Authority of imported knowledge: proposed changes requiring review versus automatic creation/update, and the acceptable review workload.
- Data boundary for the product: local storage alone versus all processing local, or hosted processing of explicitly selected excerpts. Experiment authorization is not a settled future ingestion policy.
- Breadth of capture: every conversation from enabled sources versus individually selected conversations/documents; incidental personal facts need a conscious rule.

Later branches depend on those answers: first connector and source samples; search versus drafting; assertion/page granularity; source retention; identity and link representation; editor compatibility; update/retraction/deletion behavior; automation and review UX; operating budget; success criteria.

Proposed vocabulary, not yet glossary entries: **Source** (chosen original material), **Source revision** (a preserved version of it), **Evidence span** (an identifiable part that supports or challenges an assertion), **Knowledge proposal** (a suggested addition/change awaiting the agreed acceptance policy), **Knowledge page** (a curated unit a person reads), **Entity** (a thing referred to across sources/pages). Discuss whether assertions need their own identity before formalizing them as a domain concept.

No ADR or accepted glossary terms have been created: those require resolved terms and genuine decisions from the interview. Research scripts and reports are complete authorized work; product implementation is outside this brainstorming stage.
