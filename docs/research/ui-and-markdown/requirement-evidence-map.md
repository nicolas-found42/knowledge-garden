# Requirement-to-evidence map

[Confirmed authority](../../../.scratch/ideas/current-specification.md) · [Synthesis](2026-09-29-synthesis.md) · [Decision brief](decision-brief.md)

**Demonstrated** means the stated bounded experiment ran, not that the requirement is production-complete. **Partial** has an implemented or documented example with a material gap. **Unavailable** means the necessary application/environment was absent. **Unmeasured** is an explicit research limit, never removal from scope. All alternatives and recommendations below remain unaccepted.

| Brief requirement or open question | Alternatives investigated / evidence needed | Experiment or primary evidence | Result | Limitation / remaining decision |
| --- | --- | --- | --- | --- |
| General personal data, beyond projects/chats | Mixed file-first tasks versus note/project-first tasks | [S corpus][S], 21 assets; bounded nine-file private structural probe | **Partial:** actual PDF/PPTX/photo/video/audio/code/text assets; people/place/event/question/claim records | Authored connections; no fresh face recognition, handwriting, office visual fidelity or long-video semantics |
| All lasting knowledge is Markdown | Relationship notes, page-local fences, table ledger | [D representations][D], actual 8-field round trips | **Demonstrated:** all three encode a complete fixture relation | General parser/editor interoperability, many-edge file density, schema evolution open |
| Pages, relations, annotations, saved views survive cache loss | Markdown authority with disposable index versus hidden cache-only fields | [D rebuild][D], 26 assertions | **Demonstrated:** modeled knowledge and saved intent reconstructed | Vector/thumbnail rebuild and every future app field remain to test |
| Retain copied originals in original formats | Copy source revisions; byte dedup independently of acquisition identity | [D identity/version][D], [S asset hashes][S] | **Demonstrated:** original bytes retained; identical bytes do not merge source identity | Physical dedup, branching histories, external-source update workflow not implemented |
| Stable identity after moves/renames | Persistent IDs plus ordinary paths; explicit path repair | [D filesystem move and link checks][D] | **Partial:** ID lookup recovers; stale ordinary link actually breaks | How/when app repairs visible links; duplicate-ID remediation; live Obsidian rename |
| Evidence selectors and missing sources | Heading/block/hash/revision selectors; recovery versus fallback | [D anchors][D], prior provenance mutations | **Partial:** anchor deletion/heading change and missing/corrupt bytes detected | Precise region/media/PDF recovery; support must be rejudged after content changes |
| External edits authoritative | Separate proposals, sequential base-hash guards; compare automatic merge/section ownership | [D proposal acceptance/rejection][D] | **Demonstrated:** stale page/source blocks replacement; external text preserved in fixture | Atomic multiwriter race and real editor merge ergonomics untested |
| External-only authoring; purposeful app writes allowed | No editor in concepts; controlled acceptance writes Markdown | [U interface][U], [D writes][D] | **Partial:** no Markdown editor or chat; proposal state durable | UI has no live importer/label/acceptance actions connected to disk |
| Plugin-free Obsidian with full app capabilities | Flat native metadata/links with richer app interpretation | Official pinned Obsidian help, disposable vault | **Unavailable** for actual app UI: no installed Obsidian found | Must test real properties/backlinks/embeds/edit/rename/move; unsupported originals need tested external-open behavior; capability requirement retained |
| Local-first Mac stack | SwiftUI/WKWebView vs Tauri 2 vs Electron | [P per-requirement stack table][P], actual Mac Node watcher | **Partial:** viable documented primitives; actual watcher/reconciliation behavior | No packaged shell, native bridge, signed/notarized binary, runtime-footprint comparison |
| External edit detection | File inode watcher vs directory invalidation plus scan | [D watcher][D], 56 callbacks/55 final files | **Demonstrated:** file watcher misses edit after replacement; full scan reconciles | Crash/restart/resume, network/sync drives, larger FSEvents workloads untested |
| Direct on-demand ingestion must exist | Explicit import/picker versus optional watched-folder extension | [P file access/adapters][P], retained-source experiments | **Partial:** source copying/rebuild performed by research runners | No end-to-end user import interaction; watched folders remain optional/open |
| Readable wiki of linked Markdown files | Quiet page, optional evidence dialog; richer research sidebar | [U three concepts][U], [D vault][D] | **Partial:** linked reading/source inspection runs | Reading is source-card text, not full Markdown renderer; human readability unmeasured |
| Whole collection and local scales | Individual map, expandable kind groups, neighborhood, list fallback | [U shared-corpus map][U], actual points/edges/group benchmarks | **Partial:** 27-item interactive map; synthetic drawing up to 50k/400k | Integrated 50k accessible navigation, meaningful clustering, label density and layout stability untested |
| Files and extracted people/places/concepts/events/claims/questions/decisions navigable | Item-kind records plus typed directed evidence links | [S 27-item corpus][S], [U inspectors][U] | **Demonstrated:** each requested kind represented and linked | Records authored; extracted identity/granularity remains unresolved |
| Keyword search and filters | SQL FTS baseline plus format filters; simple UI substring search | [S 16 frozen requests][S] | **Partial:** FTS top-one 8/16; index rebuild equivalent | Different UI/benchmark engines; no scale ranking or extraction coverage claim |
| Meaning-based search | Broad semantic selection vs top-five rerank; future local embeddings/hybrid | [S live comparison][S] | **Partial:** both 14/16, with different misses | Online calls over small cards; no local vector index/embedding rebuild; UI synonym demonstration clearly labeled |
| Visual relationship queries | Typed endpoints/direction/filter in code and relationship form | [S three exact queries][S], [U graph query][U] | **Partial:** 3/3 bounded results, actual endpoint/type form | Arbitrary multi-hop/Boolean/time queries and durable visual builder absent |
| Natural-language cited answers without chat | Extractive result presentation vs generative answer plus verifier | [S six independent answers][S], [audit][A] | **Partial:** 13/13 valid IDs, 9/13 exact quotes, two context defects; verifier accepts all | Source context/origin needed; no production grounded-answer claim or chat added |
| Activity timeline | Acquisition/collection-action timestamps and replay | [S date checks][S], [U acquisition replay][U] | **Demonstrated** for authored events | UI replay does not replay relationship revisions; activity log persistence needs specification |
| Reasoning timeline | Explicit proposals/observations/revisions independent of file dates | [S three-event history][S], [U time mode][U] | **Demonstrated** for controlled sequence | Natural cross-thread chronology/branches untested |
| Source-described events, sort and uncertainty | Earliest interval-bound sort; unknown group; explicit mode switch | [S nine invariants][S], actual browser descending-events check | **Demonstrated** bounded sort and overlap semantics | Date extraction, uncertain intervals across sources, human comprehension remain open |
| Unobtrusive composition/context retention | Narrow reading, three-column research, wide canvas; shared selection | [U concepts][U], actual workspace switch | **Partial:** context in memory; URL keeps selected item/mode/concept | Full history/back/forward, persistent layout and human preference unmeasured |
| Keyboard/accessibility/clutter | Semantic controls, SVG keyboard/list fallback, focus styling | [U renderer checks][U], browser AX/screenshot, repaired contrast/overflow | **Partial:** controls exposed, two visual repairs verified | No screen-reader walkthrough, complete keyboard tour, WCAG audit, participant timing or preference study |
| Broad survey and several materially different concepts | Behavior-selected precedents; same-corpus variations | [P 13 precedents][P], [U concepts][U] | **Delivered:** first-party source ledger and three inspectable structures | Precedents documented, not live app usability comparisons |
| Preserve access boundaries and prior evidence | Public research URLs separate from conversation occurrences | Corpus/source manifests, private aggregates, previous collections unchanged | **Demonstrated in recorded work:** zero conversation fetches; private maps/assets outside repo | Broad authorization does not imply future unrestricted acquisition or publication |

## Eight visualizer purposes

These are purposes, not eight required permanent panels. The same [eight shared tasks](04-search-and-timelines/experiments/corpus.json) can be attempted in each concept. No participant outcomes are invented.

| Purpose | Implemented route / comparison | Evidence result and remaining choice |
| --- | --- | --- |
| Exploration | Select an item, follow typed link, open related source | Working authored graph; useful surprise versus noisy similarity still needs human judgment |
| Explanation | Click a relationship or “Why this link?”; inspect quote/selector/source | Evidence/status visible; precise region recovery remains partial |
| Orientation | Whole collection → groups → neighborhood → whole collection | Scope stays reachable; kind grouping is authored, semantic aggregation unresolved |
| Partial memories | Words/format/relationship clue; compare source preview | Actual query examples; visual likeness/date facets not implemented at full generality |
| Comparison | Pin two pages; inspect earlier/later plan and support | Working side-by-side evidence; automatic claim alignment/diff remains open |
| Change over time | Three time meanings + acquisition cutoff replay | Point/interval/unknown handling present; historical edges/reversals replay incomplete |
| Gaps | Filter questions, speculative/tentative records, missing and linked sources | Required kinds remain visible; model cannot treat absent source content as known |
| Trails | Add, reorder, remove and revisit mixed items | In-memory UI route; separately proven Markdown trail record; UI-to-disk saving not connected |

[P]: 01-precedents-and-stacks/2026-09-29-precedents-and-stacks.md
[D]: 02-durable-markdown/2026-09-29-durable-markdown.md
[U]: 03-interface-concepts/README.md
[S]: 04-search-and-timelines/2026-09-29-search-and-timelines.md
[A]: 04-search-and-timelines/answer-audit.md
