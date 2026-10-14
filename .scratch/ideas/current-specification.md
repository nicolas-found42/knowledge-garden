# Knowledge Garden — current working specification

Status: research brief confirmed by the user on 2026-09-29 after the completed interview. This is the working specification requested by the user, not a completed implementation specification or an accepted architecture.

## Purpose and next sessions

Knowledge Garden should help its owner access, understand, search and navigate all kinds of personal data through many kinds of relationships. General personal data is central: random photos, downloads from arbitrary places, homework, saved videos, presentations, PDFs and other formats, as well as projects, repositories and agent conversations. Project and chat examples in previous research do not limit the product's scope.

The requirements interview for the next research wave is complete and the user has confirmed the shared understanding. A fresh agent will conduct the next research wave using this document and previous research. A subsequent agent will use all those findings in another grilling session to produce the project specification. UI, stack and implementation architecture remain open until that process resolves them.

This file is the durable record of the present interview. The final handoff belongs in the OS temporary directory and should reference this file and existing research rather than duplicate them.

## Confirmed requirements

### Markdown is the durable knowledge record

The user selected the following contract: all lasting knowledge is Markdown. Pages, relationships, annotations and saved views live in `.md` files. Disposable indexes, caches and thumbnails may exist elsewhere if they can be rebuilt. Such derived state must not be the only record of durable knowledge.

Original files are copied into the collection and retained in their original formats. These source assets are distinct from generated knowledge. The exact asset layout, deduplication, identity and update/version behavior require research.

Research experiments may create HTML, code, images, data files and other evidence artifacts. The Markdown contract applies to the future app's lasting knowledge; it does not prohibit runnable research prototypes. The application implementation stack has not been chosen.

### Local first on Mac

The primary platform and deployment direction is local first on macOS. Offline behavior, remote access, mobile access and sharing have not been specified.

### External editing and Obsidian

The user wants to edit Markdown files only in an external editor. The app should not include a Markdown authoring editor. Purposeful app actions may write Markdown: saving collections/searches, accepting/rejecting/correcting proposed relationships, applying labels, and generating or refreshing pages.

External edits are authoritative. Investigate update strategies that preserve them and make proposed changes inspectable. The precise merge/review mechanism is open.

Markdown files must work seamlessly in an ordinary Obsidian installation without a custom plugin. Investigate ordinary viewing, links/backlinks, properties, supported media embeds and source references. **The app must not sacrifice capabilities in any way to achieve Obsidian compatibility.** Preserve the app's full interaction and feature requirements while developing the compatible file representation. A particular app feature need not acquire an equivalent Obsidian interface; Obsidian remains an independent way to view and navigate the Markdown knowledge.

Test actual interoperability, including external edits, moves and renames, rather than relying only on renderer or syntax documentation. Exact Markdown/link/metadata conventions remain to be researched.

### A varied but unobtrusive UI

The user wants all three modes explored: a quiet reading space; a research workspace with related information; and a visual canvas. The experience should remain as unobtrusive as possible. The user has no particular reference app in mind and wants research to discover compelling examples.

Required areas of UI exploration include:

- A clean, readable wiki with links between Markdown files.
- A visually engaging, easy-to-navigate interactive visualizer supporting relationships of many kinds.
- A whole-collection visualization, alongside other useful scales of navigation. Investigate both an individual-item view and expandable grouping; preserve access to the full collection.
- Files/pages and extracted people, places, concepts, events, claims, questions and decisions as navigable items. For example, a person appearing in a photograph, mentioned in a PDF and discussed in a conversation can connect those sources. Keep supporting originals inspectable.
- All eight visualizer purposes listed below.
- Timelines for activity, reasoning and events described in source material, with sorting. The date semantics and exact sort/filter interactions remain open.
- Keyword search and filters, meaning-based search, visual queries built from relationships, and natural-language questions returning cited answers. All four are wanted. **Conversational follow-up questions/chat are excluded from the requested app experience.** This does not limit the separate agent interviews used to design the product.
- Additional UI capabilities to narrow through interview and research.

No single default home view, layout, visualizer engine or application framework has been selected.

### Eight visualizer purposes

The user wants all of these investigated:

1. Exploration: discover useful or surprising connections.
2. Explanation: inspect how things connect and what evidence supports the relationship.
3. Orientation: understand the shape of a topic, a subset or the whole collection.
4. Finding from partial memories: combine clues about content, format, time and relationships to recover an item.
5. Comparison: inspect overlapping documents, versions, competing claims and different explanations.
6. Change over time: replay new sources, changing relationships, reversals and emerging topics.
7. Gaps and unfinished understanding: find unanswered questions, missing evidence, inaccessible sources and speculative connections.
8. Trails: assemble a meaningful route through mixed material for studying, revisiting or explaining something.

The five additions are desired research scope, not a requirement to present eight separate tools or permanent panels. Compare combinations that keep the interface unobtrusive.

### Required research outputs

The user accepted all three proposed output types:

1. A broad survey of compelling apps and interaction patterns. Find useful references; the user has not supplied a preferred app. Explain which specific behaviors fit or conflict with these requirements.
2. Several materially different clickable concepts over the same varied dataset. Compare quiet reading, research workspace and visual canvas experiences, including ways they can coexist unobtrusively. Three concepts is the current recommended starting point; meaningful differences matter more than the count.
3. Working technical experiments on Markdown, actual Obsidian interoperability, external editing, search, whole-collection visualization and timelines. Keep implementation focused on answering research questions and preparing the later specification interview.

Investigate suitable stacks against these requirements and observed behavior. Preserve the full requested capabilities when testing tradeoffs. If a combination cannot yet be demonstrated, report the specific gap and alternatives rather than silently reducing scope.

Use tasks and samples representative of general personal data: random downloads, ordinary photos, learning/homework material, PDFs, saved videos, presentations and other useful formats, alongside existing project/chat examples. The earlier media/history corpus can be reused as evidence but is not enough to represent this scope on its own. Report tested, partial and untested format support explicitly.

## Existing evidence to carry forward

Use the existing reports and records; preserve them. Start with:

- [Main research index](../../docs/research/README.md).
- [Expanded findings and capability map](../../docs/research/expanded-exploration/2026-09-29-synthesis.md).
- [Navigation and provenance experiments](../../docs/research/expanded-exploration/04-navigation-and-provenance/2026-09-29-navigation-and-provenance.md).
- [Existing five-view laboratory](../../docs/research/expanded-exploration/04-navigation-and-provenance/experiments/navigation-lab/index.html).
- [Generated wiki fidelity experiment](../../docs/research/expanded-exploration/04-navigation-and-provenance/experiments/wiki-lab/README.md).
- [Original collection](../../docs/research/initial-brainstorm/README.md).

The five-view laboratory contains a small authored corpus and has not established human usability, large-collection performance or a preferred interface. Generated prose can have valid source IDs while losing qualifications. Hiding source evidence and deleting it are different operations. These findings should inform the next research tasks without becoming unapproved architecture.

The previous wave's proposals for a starting checklist, method rehearsal, answer-change inbox, counterfactual workbench and analogy laboratory remain proposals. They are distinct from the five additional visualizer purposes now accepted for investigation above.

## Questions deliberately left to research and later specification

- Asset and Markdown layout, copying/versioning behavior, deduplication, portability, and missing/moved original handling.
- Stable identities and provenance represented in Markdown, plugin-free Obsidian behavior, and app-specific interpretation of durable metadata without hiding knowledge in a cache.
- External-edit detection, merge/review behavior, and safe regeneration of knowledge the user has edited.
- General-file ingestion interaction. The previous brief requires direct on-demand ingestion to be available; watched-folder or broader background behavior remains open.
- Relationship vocabulary, granularity, source grounding, and presentation of recorded, tentative and speculative material. Distinguish acquisition state from epistemic status using the previous research.
- Whole-collection scale and performance, grouping/layout, selecting and expanding neighborhoods, relationship filters and preservation of context when moving between views.
- Timeline sorting/filtering across activity time, reasoning history and source-described events, including unknown or uncertain dates. The UI must not fabricate chronology.
- Search entry points and result/answer presentations without conversational follow-ups.
- The arrangement of reading, research and canvas modes; default home experience; persistent versus temporary controls; keyboard and other navigation interactions.
- Local-first runtime and stack, offline behavior, supporting-model use, indexing, rebuildability, latency and cost.
- Concrete comparison tasks, visual precedents, accessibility, clutter and navigation evaluation. Separate researcher-authored task coverage from observed human usability.

The next agent should investigate these alternatives and report evidence for the subsequent specification interview. It should not treat an unresolved implementation choice as a reason to restart this requirements interview.

## Research access boundaries carried from the previous wave

Prior authorized research allowed read access throughout `$HOME/Documents` and to the discovered Codex, Claude Code, OMP and Hermes histories/setups, with relevant selected, cleaned excerpts through the configured API. Raw private histories, private source assets, private URLs, exact source mappings and sensitive setup data belong in restricted local research storage outside the repository. Original personal files and setups remain unchanged.

URLs found in conversations are saved as occurrences and linked without acquiring their destinations. Their acquisition status remains merely linked. Separately selected public research sources and public datasets may be acquired. Any change to these boundaries should be recorded explicitly in the interview/handoff.

Follow repository `AGENTS.md` and its domain, issue and triage instructions when the corresponding work arises. All subagents must explicitly use `gpt-6.1-sol` or `gpt-6-luna` with `low` or `medium` reasoning.
