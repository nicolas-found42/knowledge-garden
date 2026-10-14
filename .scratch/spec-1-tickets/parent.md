## Problem Statement

The owner has personal information spread across ordinary photos, downloads, homework, PDFs, presentations, recordings, saved videos, documents, repositories, and agent conversations. Finding and connecting this material currently requires navigating its sources rather than one current, coherent collection. Manual tagging, editing, reconciliation, or approval queues would make maintaining that collection another job.

The owner needs an automatically maintained wiki that supports reading and navigation at the scale of 250,000 source files and several million granular facts or relationships, while keeping lasting knowledge accessible independently as Markdown.

## Solution

Build a local-first Mac application that ingests supplied files, watched directories, and explicitly supplied URLs, then automatically creates and maintains connected Markdown pages. The normal workflow is to view, search, and navigate; optional external editing is authoritative but never required.

Search returns pages by title, tags, keywords, meaning, links, and degrees of separation. It does not generate answers and does not provide chat. Start in a quiet reader, with evidence, related material, comparisons, timelines, trails, and a whole-collection canvas available on demand. Save named views only on request and restore the last navigation context automatically.

Apply updates at the level of the affected facts. A complete replacement, a targeted correction, and a separate event have different effects. Present one current value for the same fact, preserve optional manual corrections, and handle failures without user review chores.

Use Tauri 2 with a Rust background engine and TypeScript interface. Jev supplies semantic judgments throughout; allowed generative LLM work uses OpenCode Go with DeepSeek v4.1 and GPT-6 Luna. All lasting knowledge remains Markdown, original assets retain their formats, and local indexes are rebuildable. There is no application spending cap.

This is the project-level feature specification authorized for publication on September 30, 2026. It consolidates the completed specification interview and supersedes conflicting historical research recommendations. It does not claim the required implementation or validation already exists.

## User Stories

1. As the collection owner, I want to add ordinary photos, downloads, homework, documents, presentations, recordings, and videos, so that my collection covers everyday personal data as well as technical work.
2. As the collection owner, I want to add repositories and agent conversations alongside other material, so that related knowledge can connect across its original formats.
3. As the collection owner, I want to drop files or choose them directly, so that I can add material when I need it.
4. As the collection owner, I want to select directories for ongoing ingestion, so that new and changed files are processed without repeated import work.
5. As the collection owner, I want to supply a URL explicitly, so that I can include a selected external page or document.
6. As the collection owner, I want to retain links found inside sources as references without automatically following them, so that the collection respects the material I supplied.
7. As the collection owner, I want to inspect copied originals in their original formats, so that I can check knowledge against the source material.
8. As the collection owner, I want to access an original even when extraction is incomplete, so that an unsupported or damaged format does not make the file disappear.
9. As the collection owner, I want to have knowledge organized and updated automatically, so that I do not need to review, curate, or edit routine output.
10. As the collection owner, I want to start in a quiet reading space, so that I can focus on the page I opened.
11. As the collection owner, I want to return to my previous page and navigation context when I reopen the app, so that I can continue where I left off.
12. As the collection owner, I want to open related information and evidence only when useful, so that research tools do not crowd ordinary reading.
13. As the collection owner, I want to switch among reading, research, and canvas presentations without losing selection or scope, so that I can change perspective without restarting navigation.
14. As the collection owner, I want to go back to the previous page and view state, so that I can explore without losing my place.
15. As the collection owner, I want to search page names, so that I can open a page I remember.
16. As the collection owner, I want to search and filter by tags, so that I can narrow the collection using its organization.
17. As the collection owner, I want to use keyword search and filters, so that I can retrieve specific wording or file characteristics.
18. As the collection owner, I want to search remembered meaning, so that I can find material without recalling its exact words.
19. As the collection owner, I want to receive navigable pages as search results, so that I can read the knowledge directly instead of receiving a generated answer.
20. As the collection owner, I want to see useful titles, metadata, and existing excerpts in results, so that I can choose the relevant page.
21. As the collection owner, I want to follow links inside pages, so that I can move directly between connected material.
22. As the collection owner, I want to find pages within a selected number of links from a starting page, so that I can explore degrees of separation.
23. As the collection owner, I want to inspect the connecting path and its evidence, so that I can understand why a page is related.
24. As the collection owner, I want to combine typed multi-hop relationships with AND, OR, NOT, date, format, and status constraints, so that I can express a precise exploration.
25. As the collection owner, I want to see the active scope and filters, so that I can understand what a result set includes.
26. As the collection owner, I want to browse a grouped overview of the entire collection, so that I can orient myself without displaying every detail at once.
27. As the collection owner, I want to expand groups and selected neighborhoods while retaining access to all items, so that aggregation does not hide part of the collection permanently.
28. As the collection owner, I want to use automatic topic groups and alternative person, place, date, and format views, so that I have several useful routes into the same material.
29. As the collection owner, I want to find an item in multiple relevant groups, so that organization does not force a single category.
30. As the collection owner, I want to navigate people, places, concepts, events, claims, questions, and decisions through pages, so that extracted knowledge is accessible beyond file boundaries.
31. As the collection owner, I want to find fine source spans, individual messages, and media details in context, so that granularity does not force constant visual clutter.
32. As the collection owner, I want to inspect granular relationship types, direction, qualifiers, and supporting evidence, so that I can distinguish what a connection actually means.
33. As the collection owner, I want to compare current sources and relevant passages side by side, so that I can inspect meaningful differences.
34. As the collection owner, I want to switch and sort timelines by collection activity, reasoning events, or source-described events, so that different meanings of time remain distinct.
35. As the collection owner, I want to see approximate and unknown dates without invented precision, so that the collection does not fabricate chronology.
36. As the collection owner, I want to inspect missing evidence, unavailable sources, and uncertain connections, so that I can understand knowledge gaps without receiving a curation assignment.
37. As the collection owner, I want to follow and optionally assemble mixed-material trails with reorderable stops, so that I can study or revisit a useful route.
38. As the collection owner, I want to save a named search or navigation view only when I choose Save view, so that useful explorations persist without archiving every query.
39. As the collection owner, I want to reopen a saved view against current knowledge, so that its results remain useful as sources change.
40. As the collection owner, I want to keep saved views and lasting knowledge in Markdown, so that I can retain and navigate them independently of the application.
41. As the collection owner, I want to open the knowledge in ordinary Obsidian without a custom plugin, so that I have an independent way to read, navigate, and optionally edit it.
42. As the collection owner, I want to retain the full app capabilities while using compatible Markdown, so that Obsidian compatibility does not reduce the product.
43. As the collection owner, I want to make an optional correction in an external editor and have it remain authoritative, so that automation does not undo my intentional change.
44. As the collection owner, I want to have automatic updates continue for facts I did not edit, so that an optional correction does not freeze a whole page.
45. As the collection owner, I want to move or rename a page without losing its identity and resolvable links, so that external organization does not break navigation.
46. As the collection owner, I want to have the newest source version determine current facts, so that an older file imported later cannot revert the wiki.
47. As the collection owner, I want to have newly received changes win only when no reliable source ordering exists, so that updates have a defined fallback without pretending chronology is known.
48. As the collection owner, I want to reimport identical material without creating a false new version, so that duplicate ingestion does not distort knowledge.
49. As the collection owner, I want to have a complete replacement update affected facts and remove unsupported omissions, so that the wiki reflects the current complete source.
50. As the collection owner, I want to have a targeted correction change only the facts it corrects, so that unmentioned facts remain intact.
51. As the collection owner, I want to keep a separate event separate even when it resembles an earlier event, so that new observations do not overwrite different observations.
52. As the collection owner, I want to see one current value for the same fact, so that obsolete alternatives do not clutter the wiki.
53. As the collection owner, I want to have dependent pages, relations, and searches reflect changed facts, so that updates do not leave stale derived knowledge elsewhere.
54. As the collection owner, I want to avoid treating incomplete extraction as source deletion, so that an unread page cannot erase valid knowledge.
55. As the collection owner, I want to have watched-file removal remove the imported copy and solely supported knowledge, so that the collection follows the selected source boundary.
56. As the collection owner, I want to retain knowledge when a watched directory is merely disconnected, so that temporary unavailability is not mistaken for deletion.
57. As the collection owner, I want to keep the last successfully processed content with a clear pending or failed update state, so that I can continue reading while automatic recovery works.
58. As the collection owner, I want to search and navigate processed material, including meaning search, while offline, so that the local collection remains useful without a connection.
59. As the collection owner, I want to add new material offline and have remote processing finish when connectivity returns, so that imports do not require my ongoing attention.
60. As the collection owner, I want to continue processing after closing the window and resume at login, so that supplying material is enough to keep the collection maintained.
61. As the collection owner, I want to stop the background process with Quit, so that I retain control over when the app runs.
62. As the collection owner, I want to resume imports after restart or interruption, so that large collections do not require starting again.
63. As the collection owner, I want to use the collection while a large import is still processing, so that bulk work does not block useful navigation.
64. As the collection owner, I want to handle 250,000 sources and several million facts or relationships on my target Mac, so that the first release fits my intended collection size.
65. As the collection owner, I want to rebuild every disposable index without losing knowledge, corrections, evidence, or saved views, so that caches are replaceable.
66. As the collection owner, I want to have Jev contribute throughout semantic processing, so that the application makes full use of its composable capabilities.
67. As the collection owner, I want to use the selected OpenCode Go models for other generative LLM work, so that the implementation respects my chosen model services.
68. As the collection owner, I want to have automatic processing continue without an application spending cap or per-file cost prompts, so that budget controls do not interrupt my chosen workflow.
69. As the collection owner, I want to have provider interruptions and quotas handled as resumable work, so that temporary service limits do not corrupt the collection.
70. As the collection owner, I want to navigate with keyboard and assistive technology, so that reading and exploration do not depend on a mouse or visual canvas alone.

## Implementation Decisions

The following decisions are selected requirements; proposed module boundaries describe responsibilities rather than existing production components. The repository currently has no application implementation or production test suite. Concrete serialization, rendering, vector-search, and extraction libraries remain engineering choices to validate without weakening the contract.

### Governing decisions and precedence

The earlier confirmed research brief remains the historical record of that interview. The later specification decisions below supersede conflicting passages in it and conflicting research recommendations:

| Earlier idea or unresolved question | Selected product behavior |
| --- | --- |
| Natural-language questions returning generated cited answers | Search returns navigable pages. No generated search answers and no conversational chat. Meaning search remains. |
| Reviewable regeneration, acceptance queues, or manual semantic conflict resolution | Automatic processing and reconciliation, with automatic recovery. No required review queue. |
| Preserving obsolete claims as user-facing revision history | One current value for the same fact. Superseded and removed facts do not remain as alternative wiki values or ordinary search results. Separate real events remain separate. |
| Separate relationship notes or a shared relationship ledger | Granular facts and relationships inside relevant Markdown pages. |
| Runtime not selected | Tauri 2, Rust background engine, TypeScript interface, and rebuildable local indexes. |
| A monthly application spending cap | No application spending cap; Jev is used broadly and other generative LLM work uses the selected OpenCode Go models. Provider limits still require resumable work. |
| Automatically archiving every exploration | Named views are saved only by an explicit Save view action. Last navigation context is restored automatically. |

Research prototypes are evidence for bounded behaviors, not accepted implementation code or architecture merely because they exist.

### Reading, search, and navigation

#### Default experience

Open into a quiet reading space and restore the last page, scope, filters, and navigation context. Related information, evidence, comparison tools, and the visual canvas are available on demand. Moving between reading, research, and canvas presentations preserves the selected item and scope. Back navigation returns to the previous context.

Generated knowledge becomes visible automatically. Source origin, supporting evidence, qualifications, and processing state are inspectable without turning the default reader into a dashboard. The application has no Markdown authoring editor.

#### Search returns pages

Search supports page titles, tags, keywords and filters, semantic descriptions, and relationship queries. Results contain navigable pages with useful existing metadata and source/page excerpts. Search does not generate an answer, a new explanation, or a chat thread above those results.

Examples:

- Search `Riverside` and open a matching page.
- Select `#fieldwork` to narrow to matching pages.
- Search a remembered description such as “notes about the river trip” and receive matching pages.
- Click a link inside a page to open its target.
- Start from a page and find material within a selected number of links, inspecting the connecting path when useful.

Visual relationship queries support multiple hops, typed relationships, AND/OR/NOT, and date, format, and status constraints. Query results must expose their active scope and constraints. A result outside the current view remains reachable through navigation rather than being silently excluded from the collection.

Search over processed material, including meaning search, works offline. It must not depend on a live Jev call to produce usable local results. Online Jev processing can improve retrieval and organization while the local search contract remains intact.

#### Collection organization and granularity

The collection has a grouped overview and selected neighborhoods. Groups expand to their members, and the full collection remains accessible. Automatic topic groups are primary; people, place, date, and format views provide other routes. Items may belong to multiple groups.

Files, pages, and extracted people, places, concepts, events, meaningful claims, questions, and decisions are normal navigation targets. Fine source spans, individual messages, and media details remain searchable and can be exposed in context without making every detail a permanent visible node. The underlying facts and relationships remain granular regardless of display aggregation.

A relationship is inspectable as a typed connection with endpoints, direction where meaningful, qualifications, and evidence. Similarity alone must not silently become a stronger relation such as causation or identity.

#### The eight purposes

| Purpose | Product behavior |
| --- | --- |
| Exploration | Find related pages and traverse useful connections. |
| Explanation | Inspect the path and original evidence for a connection. |
| Orientation | Move between whole-collection groups and local neighborhoods without losing context. |
| Partial-memory finding | Combine remembered meaning, person, place, time, format, tag, and relationship clues. |
| Comparison | Inspect current sources and their relevant passages side by side, including differences that remain meaningful. |
| Change over time | Navigate actual source-described events, reasoning events, and collection activity using distinct date meanings. Replaying superseded or removed wiki facts is excluded. |
| Gaps | Expose unavailable sources, missing extraction, unresolved identities, and uncertain or unsupported connections through inspectable states rather than user review assignments. |
| Trails | Follow and optionally assemble an ordered route through existing mixed material, add/remove/reorder stops, and return to source context. Persist a chosen route only when explicitly saved. |

These purposes need not become eight permanent panels. Timelines allow selection of activity, reasoning, or source-event date meaning and user-controlled ascending/descending sorting. Approximate dates retain their uncertainty; unknown dates are not fabricated from import timestamps. Distinct real observations at different dates remain separate events even when the older source arrived first.

#### Saved views

Save view explicitly persists a named search, filters, relationship scope, or useful navigation arrangement as Markdown. Reopening it evaluates its criteria against the current collection. Every transient query or view does not become a new permanent Markdown item. Restoring the last session position does not require an explicit saved view.

### Automatic ingestion and current knowledge

#### Inputs and source access

Support direct file selection/drop, chosen watched directories, and explicitly supplied URLs. Copy original files into the collection without changing their format. Keep the original available for inspection even when some content is not yet extractable.

Links found inside imported material become searchable references. Their destinations are not automatically acquired. A URL supplied directly for ingestion is an input in its own right. Linked-only material cannot provide evidence about content that has not been acquired.

Folder ingestion includes the supplied material without a required classification or curation step. Work resumes after interruption. The interface remains usable during large imports and prioritizes user navigation and recent changes while bulk work continues.

For new material received offline, perform available local copying and extraction and make extracted content searchable. Queue steps requiring remote services and finish them automatically when connectivity returns. Clearly distinguish copied, partially processed, and fully processed material.

#### Source ordering

For the same fact about the same entity or event, prefer the newest source version rather than the latest import time. Use available source version and reliable date evidence. When no reliable ordering can be established, newly received changes win as the selected last resort. Reimporting an identical previously seen file does not make it a new version.

Different observations, entities, and events must not be merged just because their names or fields look alike. A later observation is not automatically a replacement for an earlier observation.

#### Update rules

| Incoming material | Required effect |
| --- | --- |
| New complete replacement for the same source | Apply additions and changed facts. Remove facts that disappeared and depended solely on the replaced content. Keep unrelated knowledge. |
| Explicit correction or targeted supplement | Update the facts it changes; retain unmentioned facts. |
| Separate observation or event | Add its own facts and relationships without overwriting the earlier event. |
| Older version imported later | Do not revert the current facts or restore removed old-only facts. |
| Identical previously seen content imported again | Do not manufacture a new revision or duplicate its knowledge merely because it was imported again. Preserve source provenance where acquisition contexts differ. |
| Partial extraction of a replacement | Missing extracted text is not evidence of a source deletion. Recover the missing content automatically before applying omission-based removals. |

Updates operate on affected facts and relationships. Unchanged content should not be rewritten unnecessarily. Dependent pages, tags, relationships, saved-view results, and search indexes must converge to the current knowledge. The wiki does not present competing obsolete values for the same fact.

The confirmed example is a report changing 12 visits to 15: replace the count. Remove a duration omitted by a complete replacement when it has no remaining support; retain the duration when the new note explicitly corrects only the count. A report of 8 visits on another day remains a separate observation.

This is a current-knowledge product. Permanent user-facing history of superseded or removed wiki facts is excluded. This does not authorize modifying historical research artifacts or personal originals outside the application's collection. Storage needed for safe interrupted writes or recovery must not reintroduce obsolete wiki values into ordinary navigation.

#### Deletions and failures

Deleting a file from a watched directory removes its imported copy and knowledge supported only by that source. A disconnected, unreadable, or temporarily unavailable directory is not treated as a deletion. Other independent source support or an authoritative user correction must be reconciled before declaring that knowledge solely dependent on the removed source.

If a newer source cannot be processed, temporarily retain the last successfully processed content with a small update-pending or update-failed indication. Retry automatically. Do not silently label old content as current. No failure path requires the user to approve a semantic merge or edit the wiki to make normal operation continue.

Missing or uncertain information remains missing or uncertain. Model confidence alone is not permission to invent facts, authenticate a source, or treat a caption as an observation of image pixels.

### Markdown, original assets, and external editors

All lasting knowledge is Markdown: pages, granular facts and relationships, annotations, evidence references, authoritative corrections, and saved views. Original assets remain in their original formats. Search, graph, and vector indexes, thumbnails, and work caches are derived and replaceable; they cannot be the sole record of lasting knowledge.

Use readable pages with structured sections for granular records. Preserve stable identities, meaningful relation types, source locations, origin, and qualifiers. Ordinary links and human-readable names are required alongside any internal identifiers. The exact serialization schema is an implementation deliverable to validate, not the narrow prototype parser adopted unchanged.

Deleting all derived stores and rebuilding from Markdown and retained assets must preserve knowledge identities, relations, evidence, external corrections, and saved views. Operational job state can accelerate resumption, but losing it cannot mean losing durable knowledge or permanently forgetting that inputs need reconciliation.

Markdown works in ordinary Obsidian without a custom plugin. Obsidian must support independent reading and navigation, ordinary links/backlinks, appropriate properties, supported media, and source references. The app must not sacrifice its own capabilities to fit Obsidian. Equivalent user interfaces in the two applications are not required.

External authoring is optional and authoritative. A manual correction to a fact remains protected from later automatic source updates; other unedited facts continue refreshing. Changes in wording, metadata, moves, and renames must be reconciled without losing the user's work. Detection and interpretation of arbitrary external edits require validation; protecting an edited value is not proof that the current parser can reliably identify every edit.

Automatically repair resolvable links after moves and renames. Do not silently choose between ambiguous duplicate identities. Malformed files and unresolved targets remain inspectable while automatic recovery proceeds. Concurrent editor/application writes, atomic saves, crashes, and interrupted reconciliation must not cause lost user edits or partially committed knowledge.

### Jev and supporting models

Jev is the primary semantic decision component throughout the application. Every semantic workflow must assess whether it can be expressed through Jev choices, scores, and yes/no judgments, including compositions with ordinary code, extraction tools, and the selected generative models.

| Area | Intended Jev contribution |
| --- | --- |
| Input interpretation | Identify semantic structure and useful source spans; recover organization from extracted content. |
| Granular knowledge | Select candidate facts, match entities, classify relations and qualifications, and preserve evidence context. |
| Updates | Distinguish replacement, correction, and separate event; align changed fields; check affected dependencies. |
| Organization | Classify topics and tags, assess membership, and choose useful groupings and related-page suggestions. |
| Retrieval | Interpret search intent, select useful candidates, rank pages, and judge whether relevant evidence is present. |
| Navigation | Select relevant connections and route stops from existing material without generating search answers. |
| Validation and recovery | Check specific fields and source support, select additional evidence or a different processing route, and guide bounded automatic escalation. |
| Model routing | Choose among allowed helpers using measured capability, task needs, and availability. |

Independent questions over the same evidence can run together. A subsequent request is justified when an earlier result changes the evidence or available actions. Exact copying, calculations, identifiers, workflow control, and file mutations belong to code. Jev's structured output is an interface guarantee, not a truth guarantee.

Use Jev selection plus code assembly where that can preserve original words and structure. When new wiki prose or another generative task is needed, use OpenCode Go with the user's selected DeepSeek v4.1 and GPT-6 Luna models. Current documented IDs are `deepseek-v4.1-flash` and `gpt-6-luna`. Do not silently substitute a different general-purpose LLM or provider. Local embeddings, file decoding, OCR, and transcription are supporting capabilities, not replacements for the selected generative-model policy; their implementations still need capability validation.

There is no application spending cap and no per-file cost approval flow. Continue to avoid redundant work for correctness, latency, and capacity. Provider quotas, outages, and missing model capabilities remain real: queue work, retry with backoff, use an appropriate allowed alternative when possible, and expose processing state. The no-cap decision does not establish unlimited provider throughput or account availability.

OpenCode Go documents direct APIs and the selected models, but describes typical coding-agent traffic. Suitability of autonomous personal-knowledge processing is a validation dependency. Image/audio handling through the selected route also remains to be established. This specification does not claim that an untested endpoint supports every required medium.

### Application architecture

The selected foundation is Tauri 2 with a Rust background engine and TypeScript interface. Local indexes include SQLite for keyword search and structured queries, plus a rebuildable local meaning-search index. Choose concrete UI, graph-rendering, vector, and extraction libraries through bounded engineering validation; none of the throwaway research libraries is automatically accepted.

Separate responsibilities:

1. Source acquisition and reconciliation detect stable changes, retain originals, and distinguish deletion from unavailability.
2. Extraction produces source-located evidence and explicit coverage states for each format.
3. Semantic processing uses Jev and permitted helpers to propose granular knowledge operations.
4. The knowledge writer applies valid changes while preserving authoritative external edits and current-source dependencies.
5. Derived indexes serve lexical, semantic, and relational queries without requiring the entire collection in memory.
6. The reading and visualization interface requests bounded results and preserves navigation context.

The background process continues after the last window closes, appears quietly in the menu bar, and starts at login. Choosing Quit stops it. It resumes and reconciles after restart or sleep. The user should not need to run a terminal or manually restart individual jobs.

The full collection remains on disk. Use incremental indexing, bounded work batches, lazy previews, and scoped visualization so 250,000 files and millions of facts do not require a full in-memory graph or a full scan before every search. The first import is progressive and resumable; already processed pages remain usable.

## Testing Decisions

### Main test boundary

Use one primary application-level boundary: supply or change a temporary source collection, exercise public import/search/navigation/lifecycle operations, and observe the resulting Markdown, retained originals, query results, and visible state. This tests acquisition, extraction, semantic processing, reconciliation, writing, indexing, and navigation together. Do not make private parser internals, prompt wording, database rows, or a particular graph library the product contract.

No existing production seam can be reused because there is no application implementation yet. The prior research fixtures provide scenarios and failure examples, not production integration tests. Use deterministic recorded provider responses for repeatable recovery and integrity tests, and a separate authorized live evaluation with independently fixed labels for semantic quality. A replay passing cannot be reported as live model accuracy.

Use the same knowledge outcomes for packaged Mac UI checks and real ordinary-Obsidian checks. Those external surfaces require their own drivers, but not different definitions of correct knowledge. Test only externally observable behavior; add lower-level tests where a specific integrity invariant cannot be exercised reliably at the higher boundary.

### Concrete update scenarios

Starting state for each independent scenario: a complete report, revision 1 dated May 18, 2024, describes observation V17 on May 17: observer Maya, location Riverside, 12 visits, duration 10 minutes. No other source supports these facts unless the scenario explicitly adds one.

| Scenario and exact distinguishing input | Expected current knowledge |
| --- | --- |
| Complete revision 2 dated May 20 explicitly replaces revision 1 and states Maya, Riverside, V17, May 17, and 15 visits; it omits duration. | V17 has 15 visits; duration is removed; observer, location, and event date remain. |
| A newer correction says: “The visit total should read 15, not 12. This note corrects the count only.” It explicitly refers to V17. | Only count becomes 15. Maya, Riverside, May 17, and the 10-minute duration remain. |
| A new report explicitly describes separate observation V18 on May 18 with 8 visits. | V17 retains its own count; V18 receives 8. The wiki has one count per observation. |
| Import revision 1 after revision 2 was processed. | Do not restore 12 or its removed duration. |
| Import the same previously seen bytes again. | No manufactured revision and no duplicated knowledge. |
| Only part of a complete replacement has been extracted; the missing part contains duration. | Fetch/extract missing content before any omission-based deletion. A missing extraction is not a missing fact in the source. |
| Manually correct a fact in an external editor, then update a different source-supported fact. | Retain the manual correction and apply the unrelated source update. |
| Disconnect a watched directory, reconnect it, then actually remove a source file. | Disconnection preserves knowledge; actual deletion removes the imported copy and solely supported knowledge. |

These are test inputs for implementation. The explicit first three were exercised in the development experiment; the complete production workflow is not claimed to exist.

### Observable acceptance criteria

- [ ] File selection/drop, watched directories, and explicit URL ingestion produce navigable knowledge and retain inspectable originals in their original formats.
- [ ] Links occurring inside sources remain references unless supplied directly for ingestion; unseen destinations supply no factual evidence.
- [ ] Processing needs no semantic approval queue, routine curation, or in-app authoring.
- [ ] Title, tag, keyword, meaning, and relationship searches return pages; no search action emits a generated answer or opens chat.
- [ ] Multi-hop typed queries support the selected Boolean and date/format/status constraints; visible scope and connecting paths explain results.
- [ ] Whole-collection groups expand to their members, all items remain reachable, and changes of presentation preserve selection, filters, and back navigation.
- [ ] Required extracted kinds and fine source details remain accessible at the selected granularity.
- [ ] All eight visualization purposes are available without eight mandatory permanent panels.
- [ ] Timeline date meanings and sorting remain distinct; unknown/approximate dates retain their uncertainty.
- [ ] An explicitly saved view or trail persists in Markdown and evaluates against current knowledge; transient searches do not create permanent items.
- [ ] Reopening restores the last navigation context without requiring a saved view.
- [ ] The three primary update scenarios, older-import scenario, and duplicate-import scenario produce the specified current state.
- [ ] Complete replacements remove only facts that no longer have support; targeted corrections do not delete unrelated fields; separate events remain separate.
- [ ] Dependent pages, relations, tags, and indexes converge after updates and removals; superseded values do not return in ordinary search or historical replay.
- [ ] Incomplete extraction never masquerades as source omission; failed updates retain last successfully processed content with an accurate visible state.
- [ ] Watched-directory disconnection does not trigger deletion, and actual removal respects independent support and authoritative manual corrections.
- [ ] Concurrent app/editor saves, atomic replacement, crashes, malformed metadata, duplicate identities, and interrupted reconciliation do not silently lose user edits or commit partial knowledge.
- [ ] External manual corrections remain authoritative while unedited facts continue refreshing.
- [ ] Moves and renames preserve identity and repair resolvable ordinary links; ambiguous identity is not silently merged.
- [ ] Ordinary Obsidian works without a custom plugin for the required reading/navigation, properties, links/backlinks, media, edits, moves, and original opening, without removing app capabilities.
- [ ] Source references open the actual supporting original location or clearly disclose a location fallback; source qualifications and origin are preserved.
- [ ] Removing every derived index and cache and rebuilding preserves knowledge, source references, manual corrections, saved views, and durable identities.
- [ ] Processed material remains searchable by meaning and navigable offline; new inputs receive available local processing and resume remote work automatically.
- [ ] Window close, login startup, Quit, restart, and sleep/resume implement the selected background lifecycle without requiring a terminal.
- [ ] The packaged application handles 250,000 source files and several million facts/relations on the target 16 GB Apple Silicon Mac with integrated search, updates, expansion, navigation, and rebuild, not just synthetic drawing.
- [ ] Large imports are progressive and resumable; already processed material remains usable and work does not require the full graph in memory.
- [ ] Keyboard and assistive-technology navigation cover the reading, search, and visualization outcomes, including focus and return paths.
- [ ] Required media/file classes are exercised on representative inputs; extraction gaps are visible and originals remain accessible.
- [ ] Jev is evaluated throughout semantic workflows; other general-purpose LLM calls use only the selected OpenCode Go models; exact calculations and writes stay under code control.
- [ ] Provider failures, quotas, unavailable models, and unsupported capabilities produce recoverable states, not corruption, undisclosed substitutions, or app-budget prompts.
- [ ] Model quality is evaluated with independently fixed labels for retrieval, source support, identity, updates, and provenance; typed responses and positive verifier judgments alone do not count as correctness.

### Prior evidence and remaining gates

The selected behaviors above are required. Existing research establishes only the bounded observations below. Passing an older fixture does not close the corresponding production gate.

| Area | Evidence already obtained | Required validation |
| --- | --- | --- |
| Granular source updates | Three explicit authored examples repeated three times; all expected field selections. Boundary cases and a recovery fixture also selected expected operations. | Independently labeled mixed real documents, unclear versions, multi-source pages, stable record identities, and correct propagation through dependent pages/indexes. |
| Semantic verification | A broad verifier accepted a deliberately incorrect deletion; broad more-content signals falsely flagged complete inputs. | Narrow support tests plus deterministic invariants and recovery. No claim that repeating a verifier guarantees correctness. |
| Durable representation | Three small representations round-tripped eight relation fields; 26 controlled disk assertions passed. | A general supported Markdown schema and full rebuild of the complete knowledge record. |
| Watchers and external edits | Seven bounded Mac watcher assertions passed; directory rescan recovered an append missed by a single-file watcher. | Concurrent writes, crash/restart, atomic saves, invalid metadata, duplicate identities, moves/renames, directory replacement, and large collection reconciliation. |
| Ordinary Obsidian | Disposable vault and a test procedure exist; no actual Obsidian UI test was completed. | Real plugin-free viewing, properties, links/backlinks, media, edits, internal rename, external move, and original opening. |
| Search fidelity | Small semantic retrieval tests exposed both missed candidates and inappropriate selections. | Page retrieval by title/tag/meaning/relations over representative files, including no-result cases and incomplete-source states. No generated-answer feature. |
| Wiki fidelity | Prior generated wiki/answer experiments lost qualifications or promoted a constructed caption into false provenance. | Preserve source meaning, uncertainty, origin, exceptions, and exact evidence locations in generated wiki knowledge. |
| Scale | Separate drawing fixture reached 50,000 synthetic nodes and 400,000 edges. | Integrated 250,000 source files and several million facts/relations on the target Mac: search, update, expand/collapse, label/media handling, restart, cold rebuild, and responsive access to every item. |
| UI and accessibility | Three controlled compositions exist; scripted interactions and rendering measurements are retained. | Actual reading/navigation tasks, keyboard and screen-reader access, focus/back behavior, clutter, and context preservation. Agent scripts are not a human usability study. |
| Format coverage | Bounded text/container checks and selected restricted samples exist. | Real photos, PDFs including difficult scans, office/presentation material, recordings/video, and the other required personal-data classes; retained-original access and explicit coverage gaps. |
| Provider and app delivery | Documented APIs and plausible packaging routes. | Allowed service use, actual selected-model capabilities, quotas/retries, local meaning search, bundled helpers, and a normally installable Mac application. |

Performance validation must distinguish first import, incremental update, cold rebuild, normal startup, local search, optional online refinement, and graph interaction. Record end-to-end latency and peak memory on the target machine rather than citing raw drawing-submission time. Specific numeric latency and throughput thresholds require representative workloads; the interview did not establish measured values or promise a fixed full-import duration.

Unresolved validation cannot silently reduce scope or add user curation chores.

## Out of Scope

- Generated search answers and conversational chat. Automatic source-grounded wiki creation remains in scope.
- An in-app Markdown authoring editor and required user review/curation queues. Optional external editing and explicit saving of navigation views remain in scope.
- Permanent user-facing history or replay of superseded/removed wiki facts. Distinct real past events and temporary interrupted-write recovery remain in scope.
- Automatic crawling of links merely found inside imported material.
- Automatically archiving every search, comparison, or transient view as a permanent item.
- An application spending cap, per-file cost approvals, or silent substitution of unselected general-purpose LLM services.
- Broad distribution, mobile clients, remote access, and collaboration in the first personal Mac release.
- A custom Obsidian plugin or reducing app capabilities to fit Obsidian.
- Publishing private research assets, raw histories, precise source mappings, or acquiring conversation URL destinations beyond the recorded research access boundary.
- Treating prototypes, documentation, synthetic benchmarks, or model confidence as proof of completed production behavior.

## Further Notes

### Publication and current state

This is a project-level feature specification, not a report of an existing UI defect. There is no production build, known regression range, or fixed production failure to reproduce. The concrete scenarios above specify expected behavior for the implementation. Screenshots and visual attachments are not applicable to this publication; no prototype image is offered as production evidence.

At publication preparation, the repository contained engineering guidance and an empty domain vocabulary but no tracked application implementation or production tests. The three research collections and the later source-update experiment are local, unpublished working-tree artifacts. Their relevant observations and limitations are summarized here, and the critical update inputs are included above, so implementation does not require access to a local path or the originating chat. This issue does not pretend those local artifacts are hosted attachments.

The selected target is the owner's 16 GB Apple Silicon Mac, observed on macOS 26.4.1 during the specification session. This is a release target, not the environment of a nonexistent production build. Historical experiments have their own recorded dates and environments; their results must not be generalized to other platforms.

### Research evidence boundaries

The controlled update experiment froze inputs and expected outcomes before 29 successful live Jev requests. All 13 source-relationship decisions and 65 field-operation selections across main cases, boundary cases, and recovery matched the supplied candidate policy. The three main examples each matched in three repeats. These authored inputs already contained explicit source roles, source timestamps, and candidate fields; date extraction, candidate discovery, complex multi-source pages, and source authority were not established.

The subsequent whole-record verifier caught two of three deliberately bad proposals but accepted a correction that wrongly deleted the observer, selecting “consistent” with probability 0.68. It also rejected a correct selective-summary update. At the frozen 0.5 diagnostic cutoff, the broad more-content question falsely flagged all 12 complete evaluations. One bounded recovery exposed prewritten additional text; it was not a general real-PDF retrieval loop. The supplied omission policy was confirmed by the user after these experiments. Repeated authored fixtures are not a general accuracy estimate.

Earlier cited-answer experiments are retained only as evidence of source-origin and qualification failures relevant to automatic wiki construction. They do not reinstate generated search answers. In one case, a constructed caption was incorrectly promoted into a real photograph's date even though a verifier accepted it. Exact quotes and valid citation IDs alone are insufficient.

Keep original personal files and setups unchanged during development research. Restricted copies, private URLs, exact mappings, and raw histories stay outside the repository. Conversation URLs remain linked occurrences without destination acquisition under the recorded research boundary. The future app's lack of privacy approval prompts does not broaden publication permission.

### Primary technical references and integration limits

- [Tauri architecture](https://v2.tauri.app/concept/architecture/) supports the selected native-engine/web-interface composition; application memory, accessibility, and performance remain to be measured.
- [SQLite FTS5](https://sqlite.org/fts5.html) supplies the chosen keyword-search foundation; semantic and relational correctness require their own complete application paths.
- [TypeSafe workflow guide](https://docs.typesafe.ai/concepts/how-to-build-with-system-one) and [structure recovery](https://docs.typesafe.ai/cookbooks/autoformat) support composing narrow judgments with code and selecting source text instead of unnecessarily rewriting it.
- [OpenCode Go](https://opencode.ai/docs/go/) currently documents the selected models, direct API access, finite provider allowances, and expectations for coding-agent traffic. Validate autonomous knowledge-processing suitability and required media capabilities; no application spending cap does not remove provider quotas. Do not enable a separate paid fallback or substitute an unselected model merely because a request fails.

Concrete schema, model thresholds, extraction tools, graph renderer, and vector implementation are engineering validation work under this contract. The specification is ready for agent work, including resolving these gates; the label does not mean the application is implemented or release-ready.
