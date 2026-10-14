# Specification #1 implementation tickets

Parent: https://github.com/nicolas-found42/knowledge-garden/issues/1

Each ticket includes a complete behavior, observable criteria, a concrete scenario, scope, evidence limits, and native blocking relationships.

| Slice | Ticket | Blocked by |
| --- | --- | --- |
| 01 | [#2 — Import a local text source into a durable Markdown reader](https://github.com/nicolas-found42/knowledge-garden/issues/2) | None |
| 02 | [#3 — Turn a text source into evidence-grounded facts and linked pages with Jev](https://github.com/nicolas-found42/knowledge-garden/issues/3) | [#2](https://github.com/nicolas-found42/knowledge-garden/issues/2) |
| 03 | [#4 — Reconcile complete source replacements using version order and independent support](https://github.com/nicolas-found42/knowledge-garden/issues/4) | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 04 | [#5 — Apply targeted corrections while keeping distinct observations separate](https://github.com/nicolas-found42/knowledge-garden/issues/5) | [#4](https://github.com/nicolas-found42/knowledge-garden/issues/4) |
| 05 | [#6 — Recover incomplete source updates without deleting unextracted facts](https://github.com/nicolas-found42/knowledge-garden/issues/6) | [#4](https://github.com/nicolas-found42/knowledge-garden/issues/4) |
| 06 | [#7 — Preserve external manual corrections while other facts refresh automatically](https://github.com/nicolas-found42/knowledge-garden/issues/7) | [#5](https://github.com/nicolas-found42/knowledge-garden/issues/5) |
| 07 | [#8 — Keep external moves, renames, and concurrent edits safe and navigable](https://github.com/nicolas-found42/knowledge-garden/issues/8) | [#7](https://github.com/nicolas-found42/knowledge-garden/issues/7) |
| 08 | [#9 — Ingest watched directories and distinguish deletion from disconnection](https://github.com/nicolas-found42/knowledge-garden/issues/9) | [#7](https://github.com/nicolas-found42/knowledge-garden/issues/7) |
| 09 | [#10 — Ingest explicitly supplied URLs while preserving uncrawled link references](https://github.com/nicolas-found42/knowledge-garden/issues/10) | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 10 | [#11 — Read PDFs and scanned documents with located evidence and coverage recovery](https://github.com/nicolas-found42/knowledge-garden/issues/11) | [#6](https://github.com/nicolas-found42/knowledge-garden/issues/6) |
| 11 | [#12 — Turn documents and presentations into knowledge with passage and slide provenance](https://github.com/nicolas-found42/knowledge-garden/issues/12) | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 12 | [#13 — Form photo knowledge without confusing pixels, metadata, and captions](https://github.com/nicolas-found42/knowledge-garden/issues/13) | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 13 | [#14 — Ingest audio recordings with time-located transcripts and qualified knowledge](https://github.com/nicolas-found42/knowledge-garden/issues/14) | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 14 | [#15 — Ingest video with synchronized visual and spoken evidence](https://github.com/nicolas-found42/knowledge-garden/issues/15) | [#13](https://github.com/nicolas-found42/knowledge-garden/issues/13), [#14](https://github.com/nicolas-found42/knowledge-garden/issues/14) |
| 15 | [#16 — Import repositories as connected source material with precise code and document evidence](https://github.com/nicolas-found42/knowledge-garden/issues/16) | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 16 | [#17 — Ingest agent conversations with message provenance and separate reasoning events](https://github.com/nicolas-found42/knowledge-garden/issues/17) | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 17 | [#18 — Find pages by title, tag, keyword, and visible filters](https://github.com/nicolas-found42/knowledge-garden/issues/18) | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 18 | [#19 — Search remembered meaning locally while offline](https://github.com/nicolas-found42/knowledge-garden/issues/19) | [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18) |
| 19 | [#20 — Explore degrees of separation and inspect evidence for connecting paths](https://github.com/nicolas-found42/knowledge-garden/issues/20) | [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18) |
| 20 | [#21 — Compose visual multi-hop relationship queries with Boolean constraints](https://github.com/nicolas-found42/knowledge-garden/issues/21) | [#20](https://github.com/nicolas-found42/knowledge-garden/issues/20) |
| 21 | [#22 — Browse automatic collection groups and expand a whole-collection canvas](https://github.com/nicolas-found42/knowledge-garden/issues/22) | [#20](https://github.com/nicolas-found42/knowledge-garden/issues/20) |
| 22 | [#23 — Compare current sources and relevant passages side by side](https://github.com/nicolas-found42/knowledge-garden/issues/23) | [#20](https://github.com/nicolas-found42/knowledge-garden/issues/20) |
| 23 | [#24 — Inspect knowledge gaps without creating review chores](https://github.com/nicolas-found42/knowledge-garden/issues/24) | [#6](https://github.com/nicolas-found42/knowledge-garden/issues/6), [#20](https://github.com/nicolas-found42/knowledge-garden/issues/20) |
| 24 | [#25 — Navigate distinct source-event, reasoning, and collection-activity timelines](https://github.com/nicolas-found42/knowledge-garden/issues/25) | [#5](https://github.com/nicolas-found42/knowledge-garden/issues/5), [#17](https://github.com/nicolas-found42/knowledge-garden/issues/17) |
| 25 | [#26 — Save named views in Markdown and reopen them against current knowledge](https://github.com/nicolas-found42/knowledge-garden/issues/26) | [#21](https://github.com/nicolas-found42/knowledge-garden/issues/21), [#22](https://github.com/nicolas-found42/knowledge-garden/issues/22), [#25](https://github.com/nicolas-found42/knowledge-garden/issues/25) |
| 26 | [#27 — Assemble mixed-material trails and persist them only when explicitly saved](https://github.com/nicolas-found42/knowledge-garden/issues/27) | [#26](https://github.com/nicolas-found42/knowledge-garden/issues/26) |
| 27 | [#28 — Continue ingestion through menu-bar, login, close, Quit, and sleep lifecycles](https://github.com/nicolas-found42/knowledge-garden/issues/28) | [#9](https://github.com/nicolas-found42/knowledge-garden/issues/9) |
| 28 | [#29 — Accept new sources offline and resume remote processing automatically](https://github.com/nicolas-found42/knowledge-garden/issues/29) | [#19](https://github.com/nicolas-found42/knowledge-garden/issues/19) |
| 29 | [#30 — Rebuild all derived stores from Markdown and retained assets without knowledge loss](https://github.com/nicolas-found42/knowledge-garden/issues/30) | [#8](https://github.com/nicolas-found42/knowledge-garden/issues/8), [#19](https://github.com/nicolas-found42/knowledge-garden/issues/19), [#27](https://github.com/nicolas-found42/knowledge-garden/issues/27) |
| 30 | [#31 — Verify real plugin-free Obsidian reading, edits, media, moves, and originals](https://github.com/nicolas-found42/knowledge-garden/issues/31) | [#8](https://github.com/nicolas-found42/knowledge-garden/issues/8), [#11](https://github.com/nicolas-found42/knowledge-garden/issues/11), [#12](https://github.com/nicolas-found42/knowledge-garden/issues/12), [#15](https://github.com/nicolas-found42/knowledge-garden/issues/15), [#16](https://github.com/nicolas-found42/knowledge-garden/issues/16), [#27](https://github.com/nicolas-found42/knowledge-garden/issues/27) |
| 31 | [#32 — Preserve navigation context and accessible reading across every presentation](https://github.com/nicolas-found42/knowledge-garden/issues/32) | [#23](https://github.com/nicolas-found42/knowledge-garden/issues/23), [#24](https://github.com/nicolas-found42/knowledge-garden/issues/24), [#27](https://github.com/nicolas-found42/knowledge-garden/issues/27) |
| 32 | [#33 — Keep large watched imports progressive, resumable, and responsive](https://github.com/nicolas-found42/knowledge-garden/issues/33) | [#9](https://github.com/nicolas-found42/knowledge-garden/issues/9), [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18) |
| 33 | [#34 — Qualify semantic workflows with independently fixed labels and failure cases](https://github.com/nicolas-found42/knowledge-garden/issues/34) | [#10](https://github.com/nicolas-found42/knowledge-garden/issues/10), [#11](https://github.com/nicolas-found42/knowledge-garden/issues/11), [#12](https://github.com/nicolas-found42/knowledge-garden/issues/12), [#15](https://github.com/nicolas-found42/knowledge-garden/issues/15), [#16](https://github.com/nicolas-found42/knowledge-garden/issues/16), [#24](https://github.com/nicolas-found42/knowledge-garden/issues/24), [#27](https://github.com/nicolas-found42/knowledge-garden/issues/27), [#29](https://github.com/nicolas-found42/knowledge-garden/issues/29) |
| 34 | [#35 — Qualify integrated search, updates, navigation, and rebuild at target collection scale](https://github.com/nicolas-found42/knowledge-garden/issues/35) | [#28](https://github.com/nicolas-found42/knowledge-garden/issues/28), [#29](https://github.com/nicolas-found42/knowledge-garden/issues/29), [#30](https://github.com/nicolas-found42/knowledge-garden/issues/30), [#33](https://github.com/nicolas-found42/knowledge-garden/issues/33) |
| 35 | [#36 — Deliver and verify the complete installable personal Mac release](https://github.com/nicolas-found42/knowledge-garden/issues/36) | [#31](https://github.com/nicolas-found42/knowledge-garden/issues/31), [#32](https://github.com/nicolas-found42/knowledge-garden/issues/32), [#34](https://github.com/nicolas-found42/knowledge-garden/issues/34), [#35](https://github.com/nicolas-found42/knowledge-garden/issues/35) |

## User-story coverage

| Parent story | Tickets |
| --- | --- |
| 1 | [#11](https://github.com/nicolas-found42/knowledge-garden/issues/11), [#12](https://github.com/nicolas-found42/knowledge-garden/issues/12), [#13](https://github.com/nicolas-found42/knowledge-garden/issues/13), [#14](https://github.com/nicolas-found42/knowledge-garden/issues/14), [#15](https://github.com/nicolas-found42/knowledge-garden/issues/15), [#36](https://github.com/nicolas-found42/knowledge-garden/issues/36) |
| 2 | [#16](https://github.com/nicolas-found42/knowledge-garden/issues/16), [#17](https://github.com/nicolas-found42/knowledge-garden/issues/17), [#36](https://github.com/nicolas-found42/knowledge-garden/issues/36) |
| 3 | [#2](https://github.com/nicolas-found42/knowledge-garden/issues/2) |
| 4 | [#9](https://github.com/nicolas-found42/knowledge-garden/issues/9) |
| 5 | [#10](https://github.com/nicolas-found42/knowledge-garden/issues/10) |
| 6 | [#10](https://github.com/nicolas-found42/knowledge-garden/issues/10), [#17](https://github.com/nicolas-found42/knowledge-garden/issues/17) |
| 7 | [#2](https://github.com/nicolas-found42/knowledge-garden/issues/2), [#12](https://github.com/nicolas-found42/knowledge-garden/issues/12), [#13](https://github.com/nicolas-found42/knowledge-garden/issues/13), [#14](https://github.com/nicolas-found42/knowledge-garden/issues/14), [#15](https://github.com/nicolas-found42/knowledge-garden/issues/15) |
| 8 | [#2](https://github.com/nicolas-found42/knowledge-garden/issues/2), [#11](https://github.com/nicolas-found42/knowledge-garden/issues/11) |
| 9 | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 10 | [#2](https://github.com/nicolas-found42/knowledge-garden/issues/2) |
| 11 | [#32](https://github.com/nicolas-found42/knowledge-garden/issues/32) |
| 12 | [#20](https://github.com/nicolas-found42/knowledge-garden/issues/20), [#32](https://github.com/nicolas-found42/knowledge-garden/issues/32) |
| 13 | [#22](https://github.com/nicolas-found42/knowledge-garden/issues/22), [#32](https://github.com/nicolas-found42/knowledge-garden/issues/32) |
| 14 | [#32](https://github.com/nicolas-found42/knowledge-garden/issues/32) |
| 15 | [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18) |
| 16 | [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18) |
| 17 | [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18) |
| 18 | [#19](https://github.com/nicolas-found42/knowledge-garden/issues/19) |
| 19 | [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18) |
| 20 | [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18) |
| 21 | [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18) |
| 22 | [#20](https://github.com/nicolas-found42/knowledge-garden/issues/20) |
| 23 | [#20](https://github.com/nicolas-found42/knowledge-garden/issues/20) |
| 24 | [#21](https://github.com/nicolas-found42/knowledge-garden/issues/21) |
| 25 | [#18](https://github.com/nicolas-found42/knowledge-garden/issues/18), [#21](https://github.com/nicolas-found42/knowledge-garden/issues/21) |
| 26 | [#22](https://github.com/nicolas-found42/knowledge-garden/issues/22) |
| 27 | [#22](https://github.com/nicolas-found42/knowledge-garden/issues/22) |
| 28 | [#22](https://github.com/nicolas-found42/knowledge-garden/issues/22) |
| 29 | [#22](https://github.com/nicolas-found42/knowledge-garden/issues/22) |
| 30 | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 31 | [#11](https://github.com/nicolas-found42/knowledge-garden/issues/11), [#12](https://github.com/nicolas-found42/knowledge-garden/issues/12), [#13](https://github.com/nicolas-found42/knowledge-garden/issues/13), [#14](https://github.com/nicolas-found42/knowledge-garden/issues/14), [#15](https://github.com/nicolas-found42/knowledge-garden/issues/15), [#16](https://github.com/nicolas-found42/knowledge-garden/issues/16), [#17](https://github.com/nicolas-found42/knowledge-garden/issues/17) |
| 32 | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3), [#20](https://github.com/nicolas-found42/knowledge-garden/issues/20) |
| 33 | [#23](https://github.com/nicolas-found42/knowledge-garden/issues/23) |
| 34 | [#25](https://github.com/nicolas-found42/knowledge-garden/issues/25) |
| 35 | [#25](https://github.com/nicolas-found42/knowledge-garden/issues/25) |
| 36 | [#24](https://github.com/nicolas-found42/knowledge-garden/issues/24) |
| 37 | [#27](https://github.com/nicolas-found42/knowledge-garden/issues/27) |
| 38 | [#26](https://github.com/nicolas-found42/knowledge-garden/issues/26) |
| 39 | [#26](https://github.com/nicolas-found42/knowledge-garden/issues/26) |
| 40 | [#26](https://github.com/nicolas-found42/knowledge-garden/issues/26), [#27](https://github.com/nicolas-found42/knowledge-garden/issues/27), [#30](https://github.com/nicolas-found42/knowledge-garden/issues/30) |
| 41 | [#31](https://github.com/nicolas-found42/knowledge-garden/issues/31) |
| 42 | [#31](https://github.com/nicolas-found42/knowledge-garden/issues/31), [#36](https://github.com/nicolas-found42/knowledge-garden/issues/36) |
| 43 | [#7](https://github.com/nicolas-found42/knowledge-garden/issues/7) |
| 44 | [#7](https://github.com/nicolas-found42/knowledge-garden/issues/7) |
| 45 | [#8](https://github.com/nicolas-found42/knowledge-garden/issues/8) |
| 46 | [#4](https://github.com/nicolas-found42/knowledge-garden/issues/4) |
| 47 | [#4](https://github.com/nicolas-found42/knowledge-garden/issues/4) |
| 48 | [#4](https://github.com/nicolas-found42/knowledge-garden/issues/4) |
| 49 | [#4](https://github.com/nicolas-found42/knowledge-garden/issues/4) |
| 50 | [#5](https://github.com/nicolas-found42/knowledge-garden/issues/5) |
| 51 | [#5](https://github.com/nicolas-found42/knowledge-garden/issues/5) |
| 52 | [#4](https://github.com/nicolas-found42/knowledge-garden/issues/4) |
| 53 | [#4](https://github.com/nicolas-found42/knowledge-garden/issues/4) |
| 54 | [#6](https://github.com/nicolas-found42/knowledge-garden/issues/6) |
| 55 | [#9](https://github.com/nicolas-found42/knowledge-garden/issues/9) |
| 56 | [#9](https://github.com/nicolas-found42/knowledge-garden/issues/9) |
| 57 | [#6](https://github.com/nicolas-found42/knowledge-garden/issues/6) |
| 58 | [#19](https://github.com/nicolas-found42/knowledge-garden/issues/19), [#29](https://github.com/nicolas-found42/knowledge-garden/issues/29), [#36](https://github.com/nicolas-found42/knowledge-garden/issues/36) |
| 59 | [#29](https://github.com/nicolas-found42/knowledge-garden/issues/29) |
| 60 | [#28](https://github.com/nicolas-found42/knowledge-garden/issues/28), [#36](https://github.com/nicolas-found42/knowledge-garden/issues/36) |
| 61 | [#28](https://github.com/nicolas-found42/knowledge-garden/issues/28), [#36](https://github.com/nicolas-found42/knowledge-garden/issues/36) |
| 62 | [#9](https://github.com/nicolas-found42/knowledge-garden/issues/9), [#28](https://github.com/nicolas-found42/knowledge-garden/issues/28), [#33](https://github.com/nicolas-found42/knowledge-garden/issues/33) |
| 63 | [#32](https://github.com/nicolas-found42/knowledge-garden/issues/32), [#33](https://github.com/nicolas-found42/knowledge-garden/issues/33) |
| 64 | [#33](https://github.com/nicolas-found42/knowledge-garden/issues/33), [#35](https://github.com/nicolas-found42/knowledge-garden/issues/35), [#36](https://github.com/nicolas-found42/knowledge-garden/issues/36) |
| 65 | [#30](https://github.com/nicolas-found42/knowledge-garden/issues/30), [#35](https://github.com/nicolas-found42/knowledge-garden/issues/35) |
| 66 | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3), [#34](https://github.com/nicolas-found42/knowledge-garden/issues/34) |
| 67 | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3), [#34](https://github.com/nicolas-found42/knowledge-garden/issues/34) |
| 68 | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3) |
| 69 | [#3](https://github.com/nicolas-found42/knowledge-garden/issues/3), [#29](https://github.com/nicolas-found42/knowledge-garden/issues/29), [#34](https://github.com/nicolas-found42/knowledge-garden/issues/34) |
| 70 | [#32](https://github.com/nicolas-found42/knowledge-garden/issues/32), [#36](https://github.com/nicolas-found42/knowledge-garden/issues/36) |
