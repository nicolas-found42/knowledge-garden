# General files, search evidence and honest timelines

Research date: 2026-09-29 America/New_York. [Collection](../README.md) · [Authoritative brief](../../../../.scratch/ideas/current-specification.md). UTC records fall on September 30. This builds on the [initial retrieval experiments](../../initial-brainstorm/03-retrieval-and-navigation/2026-09-29-retrieval-experiments.md) and [expanded conversion/provenance findings](../../expanded-exploration/2026-09-29-synthesis.md). It does not select an architecture or relax the four requested search modes.

## Corpus and boundaries

The [shared corpus](experiments/corpus.json) contains **27 items, 30 relationships and eight tasks**. Every relationship is researcher-authored; no discovered person identity or inferred graph accuracy is claimed. The [21 asset files](experiments/source-manifest.json) include two actual controlled PDFs, a three-slide PPTX, a CSV, text/Markdown/HTML downloads, a one-second WAV, and reused public fruit JPG/book scan plus executed controlled MP4/Python/conversation. The original public-media acquisition chain remains in the earlier media manifest. A PNG book-page derivative is not the original PDF. Item cards clearly distinguish public, controlled, missing and merely linked evidence.

The ordinary-data scenario joins an authored participant, place, study visit, photograph **annotation**, report, slides, homework, counts, two plan versions, a question and a trail. The caption attached to a real public fruit photograph is deliberately controlled and is **not its true provenance**. It tests whether a model or interface incorrectly promotes an annotation into original evidence. No face recognition was tested. A missing HEIC and an unacquired URL remain available as gap records. The URL uses a reserved invalid domain; its destination was not acquired.

[Eight actual-file checks](experiments/public-asset-results.json) pass: PDF text retains selected qualification phrases, PPTX XML contains three slides and the intended rate statement, CSV rows preserve the calculation, images decode, the WAV has the expected frame count, and FFprobe reads the MP4. This establishes bounded container/text behavior, not rendering fidelity for arbitrary office files or semantic understanding of pixels/audio/video. Model calls use the **authored source-card bodies**, not newly extracted multimodal inputs.

A separate [personal-file probe](experiments/private-format-aggregates.json) considered 6,479 entries beneath Documents, excluding Git/hidden/cache/build directories and enforcing depth/size limits. Nine eligible extensions each supplied one deterministic hash-selected sample: CSV, DOCX, JPEG, JPG, PDF, PNG, PPTX, TXT and XLSX. The copied bytes matched and every original hash remained unchanged. Structural probes succeeded: the selected PDF had an extractable 26-page text layer; PPTX had three slide XML parts; document/sheet XML had text nodes; three image containers decoded. The one-row CSV is a weak semantic sample. This is **nine file-level observations**, not population support or nine independently demonstrated semantic importers. Exact mappings, copied private assets and any parser logs remain outside the repository; only aggregate results are public. No private file text was sent to a model.

| Format/activity | Evidence in this wave | Support boundary |
| --- | --- | --- |
| Photos/images | Actual public JPG/scan; three private container decodes; caption-source failure | Person/location recognition and ordinary-photo truth untested |
| Homework/PDF | Two controlled PDFs; actual private text-layer check | Handwriting, scan OCR generality, complex layout untested here; earlier OCR failures retained |
| Presentations | Controlled three-slide deck and one private PPTX structural read | Notes, charts, animations, layout fidelity and arbitrary decks untested |
| Downloads/text/HTML | Actual authored files, format filters and source links | No arbitrary-site scraping or hostile HTML importer |
| CSV/XLSX/DOCX | Controlled numeric rows; private structural probes | Tables/meaning/formula fidelity not established |
| Saved video | Actual earlier executed MP4 retained and probed | Long-video continuity, speech/timing fidelity remain earlier gaps |
| Audio | One-second controlled tone | No speech transcription, tone/emotion or real recording evaluation |
| Code/conversations | Reused executed source and controlled discussion; earlier actual histories referenced | This wave adds no natural-history semantic sample |
| HEIC/inaccessible originals | Explicit missing fixture | HEIC decoding not tested; absence cannot become negative visual evidence |

## Four search modes and the result contract

[SQLite FTS5](https://www.sqlite.org/fts5.html) supplies token matching and ranking. The experiment uses a small BM25 OR-token query with exact format filtering, not an optimized search product. The index is built from the corpus and rebuilt after destruction; ordered retrieval agrees. [TypeSafe's reranking recipe](https://docs.typesafe.ai/cookbooks/rerank_typesafe) motivated a separate semantic choice over a shortlist; [its API](https://docs.typesafe.ai/api) and [OpenRouter compatibility documentation](https://openrouter.ai/docs/guides/community/typesafe-sdk) were rechecked before calls. All request bodies, selected candidates and replies are retained.

Sixteen frozen development requests include paraphrases, partial memories, format constraints and four requests for unavailable evidence. The metric requires an acceptable single seed, or `none` when the specified evidence is not established. This is a narrow contract: a UI may usefully show the merely-linked record or person card as an **explanation of the gap**, even though choosing it fails this benchmark. Selection of such a card is not by itself a hallucinated claim about its contents.

| Method | Correct first seed / abstention | What failed |
| --- | ---: | --- |
| FTS5 lexical baseline | 8/16 | Paraphrase, missing evidence and mismatched seed granularity; no semantic abstention policy |
| Jev over all eligible cards | 14/16 | Returned the unacquired link for a request about website contents and the person record for a request about inspected face pixels |
| Jev over lexical top five | 14/16 | The desired slides/homework were absent from the candidate set; it abstained for slides and chose a related rate claim for homework |

The lexical shortlist contained an acceptable seed for **10/12 answerable requests**. The two semantic variants therefore tie on aggregate accuracy while failing at different stages. Broad retrieval correctly finds all 12 answerable seeds but misses two strict abstentions. The shortlist variant handles four unavailable-evidence cases correctly but loses two real targets. It cannot repair an omitted candidate. These fixture-specific counts are not calibrated general accuracy and should not select a universal confidence threshold.

Three exact relational queries pass: incoming `mentions` edges to the person combined with PPTX returns the slides; accepted `supports` edges return the count table for the rate claim; `revises` returns the older plan. The browser's relationship builder executes endpoint/type filtering over the same edges. Its current interaction does not yet compose arbitrary multi-hop predicates, temporal constraints, Boolean groups or saved visual queries. Those capabilities remain design work. The durable-record concern separately preserves saved query intent in Markdown.

Meaning search in the clickable UI is visibly labeled a small synonym demonstration. The live model comparison runs outside that UI. No local embedding model, vector index, vector rebuild, hybrid rank fusion or end-to-end interactive inference latency was benchmarked. Meaning-based search remains a firm requirement, and these are explicit implementation alternatives/gaps.

## Single-question answers and citation failures

Six supporting-model calls answer independent questions with source IDs and quotes. Six Jev calls then judge support. They do not form a chat; no follow-up conversation UI is introduced. A lexical extractive presentation is also available in the concepts, but it is not scored as generated answering. The independent [sentence audit](answer-audit.md) distinguishes source support from exact quote validity and preserved origin.

Across **13 sentences**, all cite existing IDs, only **9 quotes are exact**, and the researcher audit finds **11 context-supported, one qualified and one unsupported** sentence. Four quote strings combine or alter source text. More seriously, one exact quotation comes from the controlled picnic caption; the answer turns it into a claim that the real photograph was taken in May 2024. Another sentence broadens absence of a date established by pixels into absence of all date information. **Jev accepted every sentence**, including both contextual defects. Only one answer explicitly preserves controlled origin, despite the prompt asking for it.

This repeats and sharpens the earlier wiki finding: valid citations, exact copied text and a second model are separate checks. Exact quotation cannot rescue a wrong relationship between annotation and original source. A candidate recommendation is for code to retain origin/acquisition/revision labels in answer citations, and for the interface to expose the actual quoted context with the supporting original. That is a recommendation to investigate, not evidence that a complete answer system is reliable.

## Three time axes

The browser implements activity, reasoning and source-described events over the same corpus. [Nine deterministic assertions](experiments/timeline-results.json) pass: 2026 import versus 2024 described event stays separate; unknowns remain outside ascending/descending chronology; a May-wide uncertain date overlaps May 17 and is not asserted before/after it; UTC offsets normalize; recorded proposal/observation/revision order survives; and format filtering retains the date meaning.

Sorting by an interval's earliest bound is only a display convention. It does not assert that the whole interval preceded another item. Month-only photo metadata is uncertainty, not a month-long photograph event. Unknown dates remain available. The reasoning sequence is authored; no cross-thread natural-history chronology or natural-language date extraction was validated. Acquisition replay filters items by activity time; it does **not** reconstruct historical relationship revisions because those edges lack dated histories in this corpus. The later spec must decide what a replay means for changed/deleted evidence and derived relationship status.

## Latency, cost and retained limits

The [original live run](experiments/run-20260930T004745Z/summary.json) records **44/44 successful transport requests**, **80,977 input tokens**, **5,804 output tokens** and **$0.005773986 reported router cost**. Resolved models are `typesafe/jev-1.13-20260917` and `openai/gpt-4.1-mini` (supporting model alias, not immutable dated build).

| Role | Calls | Median elapsed request | Reported USD |
| --- | ---: | ---: | ---: |
| Broad semantic selection | 16 | 2.094 s | 0.002483796 |
| Shortlist semantic selection | 16 | 0.615 s | 0.000583800 |
| Cited generation | 6 | 1.885 s | 0.002456784 |
| Answer verification | 6 | 0.278 s | 0.000249606 |

The ledger excludes coding agents, browser chooser calls/typing helper, local extraction, storage, electricity and billing outside reported response fields. Four concurrent retrieval workers, candidate lengths and provider conditions affect timings. The six generators receive small author-selected bundles, so their latency/cost does not include retrieval or arbitrary conversion. No transport failures, retries or parse repairs occurred; semantic and quotation failures are retained, not corrected in place. Primary-document acquisition and a browser-download timeout are recorded separately.
