# Evidence recovery and five navigation experiences

Research date: 2026-09-29. [Concern index](README.md) · [Expanded exploration](../README.md).

The useful unit need not be a generated page. A source record can retain its original media, selected spans, conversions, links and interpretation status, while several views present the same evidence. This investigation implements a small local comparison, measures authored evidence-set coverage and tests source-anchor behavior. It does not determine the best home view, measure human usability or choose a storage engine.

## Source-backed possibilities

The [Web Annotation Data Model](https://www.w3.org/TR/annotation-model/#selectors) distinguishes a resource from a selected part and its source state. Its position-selector warning motivated our edit test; a location alone does not preserve a passage across revisions. [PROV-O](https://www.w3.org/TR/prov-o/#wasDerivedFrom) distinguishes derivation, quotation and revision. That vocabulary suggests keeping an OCR observation, a model interpretation and their source separately identifiable, without requiring this project to adopt RDF.

[IIIF Presentation 3](https://iiif.io/api/presentation/3.0/#21-defined-types) supplies a useful precedent for collections, ordered canvases and ranges across books or timed media. The [Media Fragments specification](https://www.w3.org/TR/media-frags/#naming-space) describes temporal and spatial fragment dimensions. These are representation precedents, not proof that an arbitrary viewer, PDF or video already supports reliable deep links. The media concern actually exercised page and time derivatives; precise generated-region and event localization remain unverified.

One additional implementation was inspected: **Hypothesis client**, commit `b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a`. Its [HTML anchoring code](https://github.com/hypothesis/client/blob/b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a/src/annotator/anchoring/html.ts#L35) gathers range, text position, quote and media-time selectors; it checks the stored quote when trying positional/range anchors before falling back. The [quote matcher](https://github.com/hypothesis/client/blob/b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a/src/annotator/anchoring/match-quote.ts#L86) combines quote similarity, surrounding context and position hints. These are concrete implementation mechanisms, not validated fidelity results for Knowledge Garden.

The [PDF implementation](https://github.com/hypothesis/client/blob/b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a/src/annotator/anchoring/pdf.ts#L551) requires a quote selector to validate text position, then falls back to quote search. Its page-text normalization and offset translation show why page number, extracted text offsets and rendered text coordinates are not interchangeable. Five files were captured; inspection focused on HTML anchor selection, quote scoring, PDF quote/position validation and rendered-text handling. The complete application was not executed. The [source manifest](experiments/provenance-lab/source-manifest.json) records exact URLs and file digests; text-range.ts was captured as a dependency reference, not exhaustively reviewed.

## Anchor experiment: recover and abstain separately

The experiment selected one short passage from each of three acquired primary documents: TypeSafe State, W3C PROV-O and IIIF Presentation. Eleven controlled edits per document included prefix insertion, appended content, emoji, repeated quotations, a duplicate document, deletion, replacement, internal whitespace changes and a changed surrounding assertion. These are **33 correlated mutations of three passages**, not 33 independent real-world cases.

The three resolvers are intentionally simple: reuse the old character offsets; return the first exact quote; or require exactly one matching quote with exact surrounding context. This is independent research code, **not a benchmark of the Hypothesis implementation**. JavaScript DOM/UTF-16 offsets and PDF text layers are not implemented by this Python Unicode-code-point experiment.

The [audited results](experiments/provenance-lab/audited-results.json) distinguish recoverable exact targets from absence/ambiguity:

| Resolver | Correct recoveries / 21 recoverable targets | Proper abstentions / 12 absent or ambiguous targets | Wrong known target | Ambiguous attachment | Missed recoverable target |
| --- | ---: | ---: | ---: | ---: | ---: |
| Position only | 6/21 | 0/12 | 15 | 3 | 0 |
| First exact quote | 18/21 | 9/12 | 3 | 3 | 0 |
| Unique quote and context | 15/21 | 12/12 | 0 | 0 | 6 |

Position-only also attaches to nine cases with no unchanged exact target. Whole-document duplication requires abstention under this experiment's uniqueness policy: returning the first occurrence there is ambiguous, not proof of a semantically wrong passage. Changing whitespace makes the exact sequence unavailable to these resolvers; it does not make the information inherently unrecoverable. Context matching avoids wrong attachments at the cost of six recoverable misses. The pooled `correct=27/33` for both quote methods hides that tradeoff and should not be presented as recovery accuracy.

The initial `immutable_original_recovered` field checks an original text slice already in memory, repeated across mutations. It does **not** demonstrate durable archive lookup. A separate portable disk experiment writes three short source snapshots keyed by their hashes, resolves references through a registry and validates hash plus span. It recovered **3/3 original excerpts** and rejected **9/9 deliberately missing, corrupt or invalid-offset references**. This is a bounded local file test, not a storage durability or access-control audit. Thirty mutated revisions require a new support judgment before their text can establish a current assertion; recovering the original remains useful historical evidence.

The frozen [portable excerpts](experiments/provenance-lab/portable-fixtures.json) reproduce the same mutation metrics without acquiring any URL. Original full-document snapshot digests remain in the manifest; current live page bytes may differ. An initial off-by-one error in a fixture insertion length stopped before metrics were written. The code now derives insertion lengths and asserts each gold substring. The failure is preserved separately as an implementation failure.

## Comparing the five requested experiences

Open the [local evidence comparison](experiments/navigation-lab/index.html). It uses **24 source/proposal cards, 22 typed relationships and four question collections**, all based on saved public or explicitly constructed artifacts from this research. It contains no private conversation text or private URL occurrences. Media and links are local; the page does not acquire destinations or call a model.

| Experience | Concrete implemented behavior | What it makes easier to inspect | Limits to test next |
| --- | --- | --- | --- |
| Readable wiki | Four concise authored research capsules with source links, status and media drill-down | A coherent explanation of an investigation and its caveats | These fixed authored paragraphs do not change when a status filter hides a source card. Generated prose is separately evaluated in the wiki lab. |
| Source explorer | Filters by project, person/author, topic, format, source date and observation date; text search; original artifact links and hashes | The available evidence, failed acquisitions, derived records and unknown dates | Date coverage is sparse. “Observed today” does not imply the source was created today. Private thread ingestion is not wired into this demonstration. |
| Reasoning timeline | Controlled motion-demo proposal, adoption, implementation and measurement order; explicitly grouped method histories | How an adopted change relates to its actual outcome and remaining uncertainty | Cross-thread chronology is unknown for the method fixtures. Their display groups are not a fabricated chronology. Unordered sources remain accessible through other views. |
| Connection neighborhood | A selected card and its incoming/outgoing typed links; status filtering | Which evidence supports or motivates a link, and which links remain speculative | One-hop authored graph only. It does not implement automatic graph construction, exhaustive discovery or entity merging. |
| Focused collection | Evidence cards grouped around one concrete question, including exceptions and unavailable-source records | Compare several kinds of material needed to answer the question | Groups are authored with known task requirements; evaluating automatic grouping is separate work. |

The images, video and source code are substantive demonstrations rather than imaginary media labels. The method cards link to the program **and** the corresponding real execution record and failed version. Speculative/constructed records are visibly labeled. A status filter also hides a disallowed map center; no label is silently converted into a recorded fact.

## Clutter and content coverage

Eight authored questions define required record IDs. [The evaluation](experiments/navigation-lab/evaluation.json) asks whether those IDs remain available in a displayed set and counts other cards. It measures **required-node coverage**, not interface reachability, reader accuracy, time, clicks, utility or preference.

| Display policy | Required sets fully available / 8 | Mean visible cards | Mean additional cards beyond required IDs |
| --- | ---: | ---: | ---: |
| Every record | 8/8 | 24.0 | 21.375 |
| Recorded only | 6/8 | 20.0 | 17.875 |
| Recorded and tentative | 7/8 | 23.0 | 20.5 |
| Authored question collection, all statuses | 8/8 | 6.5 | 3.875 |

The collection comparison is an **oracle grouping**: project membership and requirements were authored together. It shows the content-availability tradeoff to test, not demonstrated automated curation. Hiding speculation removes a required connection proposal from the question that asks how to test that proposal. It does not establish that speculative links belong in every default view.

A second count illustrates the promotion decision: replacing five thread cards and one measurement card with all 17 individual messages and all 50 frame measurements would expand the same display from **24 to 85 items**. That count does not measure perceived clutter, and detailed records remain useful when a reader opens their source. It argues for testing expandable evidence, not deleting tools or forcing every record into one granularity.

The local browser checks use the configured Jev browser-use runner with an isolated owned browser and a localhost allowlist. The source explorer test actually switched views and selected PNG, then independently read a title reflecting one visible record. Sequential checks also verified the timeline and connections tabs. Three earlier concurrent checks lost their browser-control connections; their failures remain recorded. The focused-collection sequential check failed before observation with a missing browser session, so that tab's browser verification is **incomplete**, although its source/links and JavaScript syntax passed static checks. This is a browser-control failure, not a demonstrated application defect. These smoke checks are not a human usability study or a complete visual/accessibility audit. The browser runner does not expose token/cost responses, so its chooser ticks are excluded from the measured inference ledger.

## Separating generated prose from source structure

The [wiki lab](experiments/wiki-lab/README.md) compares fixed source bundles, deliberately lossy observations and six source-linked generated capsules. It audits sentence support against original referenced evidence and manually checks omitted provenance and qualifications. Generated pages can help reading, but valid source IDs and fluent text do not by themselves preserve construction history, uncertainty or exceptions. A fixed narrative also needs its own revision/dependency behavior; hiding a speculative record in the explorer does not remove a speculative sentence from an already drafted page.

## Next discriminating work

Reserve naturally selected tasks with independently labeled required evidence. Have a reader use the five views to recover a specific source qualifier, identify a reversal, inspect an actual output and reject a misleading connection; measure success, extra items inspected and time. Compare source-first selections and generated narratives using the same available material. Include deleted sources, duplicate quotations, branch-aware real histories, a handheld book photo and independently annotated video events.

The experiments support keeping original source recovery, semantic support, status inheritance and presentation as separate checks. They do not choose a graph database, Markdown schema, default experience, confidence threshold or automatic promotion policy.
