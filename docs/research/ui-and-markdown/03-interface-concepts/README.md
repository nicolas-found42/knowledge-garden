# Three interface concepts over shared evidence

[Collection](../README.md) · [Actual browser results](browser-results.md)

Research artifact, 2026-09-29. These are throwaway interfaces for comparing navigation, not an implementation-stack choice or selected product architecture. The research question is whether quiet reading, a contextual research workspace and a collection-first canvas can coexist without losing the selected item, filters or evidence.

From the repository root, run `python3 docs/research/ui-and-markdown/serve.py`. Open [the prototype](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/index.html?variant=reading). There is no build step or dependency installation. Keep the server rooted at the repository so the shared corpus and captured source paths resolve.

- [A · Quiet reading](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/index.html?variant=reading): a narrow, serif reading page; navigation stays in a small left rail; supporting evidence opens on demand.
- [B · Research workspace](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/index.html?variant=workspace): a three-column arrangement with persistent related evidence beside the main task. Denser controls and sans-serif hierarchy suit comparison and inspection.
- [C · Visual canvas](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/index.html?variant=canvas): opens on the full collection map when no `view` is supplied; a dark, wide canvas and contextual lower inspector keep collection shape prominent.

The same route uses `?variant=reading|workspace|canvas`, with optional `view` and `item`. The bottom selector and left/right arrows change the concept in place. Keyboard arrows avoid inputs, selects, contenteditable fields and the evidence dialog. Every mode remains available in every concept. Mode switches preserve selected item, filters, current query, relationship constraints, trail, comparison items, zoom and acquisition replay in memory. A disclosure exposes the entire relevant state. Only concept, view and selected item survive a reload through the URL; this prototype does not claim saved state or durable Markdown views.

The three layouts are deliberately different: reading removes the permanent context panel and constrains prose width; workspace dedicates an entire right column to evidence and neighbors; canvas defaults to collection visualization and places its contextual inspector below the visual surface. Shared task controls make capability comparisons fair. Styling alone should not be treated as a usability result.

## Executable coverage

All records come from [one shared corpus](../04-search-and-timelines/experiments/corpus.json): 27 varied records, 30 authored/recorded relationships and eight task definitions. The corpus includes files, extracted people/places/concepts/events/claims/questions/decisions, an annotation and a trail. This concept folder creates no duplicate evidence corpus. Markdown authoring remains an external-editor workflow; the prototype offers source opening and reading, without native application launching, file edits or app saves. Public versus controlled origin, acquisition and epistemic status are displayed separately. No raw private material is used.

| Required purpose | Working interaction | What this does not establish |
| --- | --- | --- |
| Exploration | Full SVG collection; clickable nodes; neighbor navigation; typed edge inspector | Automated useful-connection discovery or human usefulness |
| Explanation | Click a line or “Why this link?”; inspect relationship status, quote, selector and evidence source | Automated entailment verification or reliable source anchoring after edits |
| Orientation | Whole collection, one-hop neighborhood, expandable kind groups, clear-group return | Topic clustering quality or a preferred grouping vocabulary |
| Partial memory | Keyword/body/tag matching, exact format and status filters, relationship constraints | Automatic multimodal recall from arbitrary personal files |
| Comparison | Pin two pages or use two item selectors; inspect source qualifications side by side | Automated contradiction detection or version diff |
| Change over time | Activity/reasoning/event timelines; ascending/descending lower-bound sort; collection acquisition replay carried to Map | Historical relationship-state replay, because edges lack revision timestamps |
| Gaps | Purpose filter returns questions, uncertain claims, unacquired links and unavailable sources | Missing-evidence inference beyond supplied fixture metadata |
| Trails | Add pages, move steps up/down, remove and reopen | Durable Markdown trail writes or automatically curated paths |

The linked reading page renders the supplied body as readable prose and exposes every supplied neighbor and relationship explanation. It contains no Markdown authoring editor. Source dialogs show source metadata and original links; JPG/PNG, MP4 and WAV previews use native browser media controls. PDF, PPTX, CSV, Python, JSON, TXT and HTML originals open via source links. Opening a merely linked original never changes its acquisition state. Missing originals stay explicitly missing. Native rendering/extraction quality for every format is assessed elsewhere, not inferred from a working link.

The four search modes are independently selectable:

1. Keyword + filters executes literal matching across title, body and tags.
2. Meaning executes a small **authored synonym-expansion demonstration**, prominently labeled. It is not embedding retrieval or a benchmark claim.
3. Relationship query executes exact filtering over supplied edges: choose a relation type and optional connected item; receive matching endpoints.
4. Cited answer presents matching source excerpts as a **labeled extractive fixture**, with direct source citations. It abstains when there is no lexical match. There is no conversational thread or follow-up chat interface. Actual retrieval/model measurements belong to the adjacent search experiment.

Timeline dates never silently borrow activity time as source-event time. Month uncertainty appears as a May 1–31 interval with its supplied “day unknown” label. Unknown dates sit outside chronological sorting. Sorting by an interval's start does not establish chronology between overlapping intervals. Acquisition replay filters items through an activity timestamp and preserves the selected page when switching back to reading; it does not manufacture edge histories.

## Accessibility and scale boundaries

There is a skip link, visible keyboard focus, native select/button/form semantics, a native modal dialog with Escape closure, keyboard-operated SVG nodes and edge evidence, and a list fallback for the map. No animations are required, and a reduced-motion rule prevents transitions. Color is supplemented by explicit status labels and dashed uncertain edges. Responsive layouts reduce to one column. This is implemented accessibility support, not an audit or assistive-technology result.

The real map uses a deterministic phyllotaxis layout, draws up to 80 directly visible items; groups and lists preserve access to all 27 items in this fixture. It does not implement full access beyond that display cap. This tiny 27-item fixture fits fully; it is not evidence of production-scale graph performance. No force simulation, canvas pan or item dragging is implemented. Zoom buttons change the SVG viewBox; neighborhood/group changes are the navigation mechanisms under comparison.

[The automatic browser benchmark](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/scale.html) runs one warmup and five repeats at each size on page load, shows actual per-run and min/median/max results plus browser/runtime metadata, and offers a JSON download. It also measures building a 25px grid spatial index and 100 deterministic nearest-point hit tests within a 16px radius. These measure a bounded picking primitive, not end-to-end interaction latency.

Map → “Synthetic scale experiment” provides 1,000 / 10,000 / 50,000 deterministic synthetic points and a browser-invoked harness. `await window.runScaleBenchmark(50000)` runs the same measurement and places its result in `window.lastScaleResult`. It measures three distinct quantities:

- `layoutMs`: synthetic coordinate generation, excluding graph loading and clustering.
- `renderSubmissionMs`: Canvas 2D point-draw command submission, excluding graph edge drawing and source/media decoding.
- `elapsedUntilNextAnimationFrameMs`: elapsed time until the next callback after drawing; **not GPU-completion time**.

This is a bounded point-rendering experiment, not a full graph-engine benchmark. It deliberately records `edgesDrawn: 0`. That inline harness does not measure full interaction latency, DOM/accessibility representation at 50k or force layout. The separate automatic point and edge harnesses add bounded hit tests and edge-density measurements; [actual retained browser results](browser-results.md) distinguish them from CLI renderer success.

## Verification and comparison protocol

`node --check app.js` passes. `node check-render.cjs` performs a CLI DOM-stub renderer smoke check; [renderer-check.json](renderer-check.json) captures 18 layout/mode renders, all four search renderers, all three timeline modes, all eight purpose choices, 27 source inspectors and 30 relationship inspectors. It establishes that those code paths render against the actual corpus. It does not exercise real clicks, native modal behavior, focus, visual collisions, media decode or human usability. Actual coordinated browser testing is recorded in [browser-results.md](browser-results.md); its narrower assertions do not establish all renderer-smoke paths as browser-verified.

Use the eight shared task definitions in the rail without changing the corpus between concepts. Suggested evaluation:

- Explain the rate claim and recover its one-observation qualification; count whether the reader opened the table/homework evidence and retained the caveat.
- Recover the slides from “Maya, Riverside, presentation”; record search mode, extra records opened and elapsed time.
- Compare the two plans and identify what was never measured; record unsupported conclusions separately from successful navigation.
- Show the full collection, expand a group, inspect a neighborhood and return; count context losses, extra clicks and disorientation ratings.
- Distinguish import date from transaction/event date and month uncertainty; score fabricated exact dates as errors.
- Find missing photo, unacquired link and speculative rain claim; check that acquisition failure and epistemic uncertainty remain distinct.
- Assemble the notes → table → old plan → revised plan → unresolved question trail; check reordering and revisiting.

Counterbalance A/B/C order across readers. Use the same starting selected item and reset transient state between trials; otherwise the second concept inherits knowledge learned in the first. Measure task completion, evidence recovery, misleading certainty, action count and subjective clutter separately. No preferred home mode, validated usability decision or production architecture has been selected. The requested research collection preserves the prototype; no commit, issue mutation or throwaway branch was created by this subtask.

## Researcher visual repairs and denser scale fixture

The coordinating researcher inspected a 1280×720 browser screenshot and found two visible defects: the active Read label inherited white text while navigation's transparent-background rule won the CSS cascade; the long comparison-task select overflowed its 200px reading rail. Explicit pressed-navigation styling, full-width constrained rail selects and `min-width: 0` on grid columns address these findings. Canvas uncertainty badges now use a lighter gold. These are researcher-observed rendering defects and repairs, not human usability findings or a completed contrast audit.

[The separate edges benchmark](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/edges-scale.html) preserves the original point-only measurement. It runs 1k/10k/50k nodes at 2 and 8 authored directed edges per node, one warmup and three repeats per configuration. Full Canvas rendering submits every edge in batches of 5,000; the maximum fixture contains 50,000 nodes and 400,000 edges. The same graph is also aggregated into 12 deterministic index-modulo groups and at most 144 weighted directed group links. Direction exists in data but the drawing omits arrowheads. Grouping is deliberately synthetic and makes no semantic-quality claim.

Retained outputs separate allocation, deterministic position generation, edge generation, group aggregation, full draw submission, grouped draw submission and next-frame elapsed timings. Exact typed-array byte counts describe fixture storage; optional aggregate browser heap samples do not establish total browser or GPU memory. No edge culling is applied. Dense visual saturation is expected and benchmark completion does not establish readability or usable individual navigation at 50k.

Both automatic benchmark pages optionally POST completed JSON to `/_research/measurements`, a narrow local endpoint supplied by the coordinator. A missing endpoint leaves results visible and reports the retention failure; it does not invalidate the completed measurements. Actual JSON artifacts are retained by the coordinator in [browser-records](browser-records/).
