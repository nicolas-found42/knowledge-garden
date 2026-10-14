# Actual agent histories: ingestion, interpretation and recovery

Date: 2026-09-29. Scope: read-only discovery of Codex, Claude Code, OMP and Hermes source histories and configuration formats; on-demand ingestion quality; thread/message/tool representations. No source session or setup was modified. No architecture decision is recorded here.

## Availability and actual formats

These are observed local installations, not assumed universal defaults. Counts are point-in-time observations; running applications can append to histories. The [aggregate record](experiments/aggregate-results.json), [sample metadata](experiments/sample-metadata.json) and private snapshots are the primary evidence for this table.

| Application | Discovered history/configuration | Availability observed | Adapter facts and limits |
| --- | --- | --- | --- |
| Codex | `~/.codex/sessions/` JSONL; `state_5.sqlite`; `thread_history_1.sqlite`; `config.toml` | 565 rollout JSONLs; 528 thread metadata rows in a separate read | `session_meta`, `event_msg`, `response_item`, `turn_context` and other events coexist. Readable messages and function/custom-tool records occur under `response_item.payload`. Do not count an event echo and response message twice. SQLite thread metadata exposes title, intent preview, origin, parent/spawn context, revision-related fields and rollout paths. |
| Claude Code | `~/.claude/projects/` JSONL; `history.jsonl`; `settings.json` | 433 project JSONLs | Top-level `user` and `assistant` records contain messages, `uuid` and `parentUuid`; `custom-title`, queue/control and attachment records coexist. Tool calls/results are content blocks, sometimes inside a user-role record. The role field alone does not establish human authorship. Five sampled smallest files contained no readable messages; three were only local `/clear` controls. |
| OMP | `~/.omp/agent/sessions/` JSONL; `history.db`; `agent.db`; `config.yml` | 3,181 session JSONLs | Session/title/version, model/thinking changes, parent-linked message records, compaction and other state records share the stream. Tool outputs use role `toolResult`; assistant blocks use `toolCall` and `thinking`. `history.db` has history/title/recap tables. Eight smallest samples contained no messages. |
| Hermes | `~/.hermes/state.db`; `sessions/` request dumps; `config.yaml` | 109 SQLite sessions; 19,597 messages, comprising 402 user, 7,579 assistant and 11,616 tool messages | SQLite is the useful discovered store, rather than the five JSON request dumps being mistaken for complete history. Messages expose stable IDs, roles, tool linkage, active/compacted flags, compressed summaries and display/protocol fields. Sessions expose titles, parent IDs, source, timing, model usage and completion state. |

Configuration inspection read format/key names only; credential-file contents and values were neither displayed nor exported. Observed settings distinguish model/effort, compaction/compression, memory, tool integrations, skills and hooks. These fields can explain how a thread was produced, but importing entire configuration files into knowledge would bring irrelevant setup and secret risk. Credential-shaped blocks were withheld from API input; a post-run pattern scan found zero recognized key-token or bearer-token strings in both frozen request sets. This pattern scan is an explicit check, not proof that arbitrary sensitive strings are impossible.

All SQLite connections used `mode=ro`. No SQLite files were copied wholesale. For selected JSONL histories, 39 raw files totaling 23,461,606 bytes were saved later as local private snapshots with hashes; initial extracted sources and frozen API states remain separate. Hermes selected message records were preserved privately. The later snapshots are not claimed to be an atomic four-application snapshot.

## Sampling and frozen labels

The initial inventory covered 4,288 file/session units. It did not ingest or semantically label every unit. Per application, files/sessions were sorted by byte size or message count, with the eight smallest units plus five ranks at approximately the 10th, 25th, 50th, 75th and 90th percentiles selected: **13 per application, 52 total**. This intentionally finds empty starts, short meaningful interactions and richer work; it is a purposive sample, not a prevalence estimator.

Title/intent availability is a separate adapter concern from byte length. Codex titles were joined from thread metadata, Claude titles came from `custom-title` records, OMP titles can be joined through JSONL session IDs to `history.db.session_titles` (522 title rows observed), and Hermes titles came from session metadata. Exact title mappings remain private; populated counts are in the aggregate record. A title can keep an intended task understandable when the transcript is tiny or interrupted, and substantial conversation is still required to establish its findings or adopted decisions. The inference suite deliberately judged message evidence without adding titles, so it does **not** establish a title-plus-thread classifier result. A production on-demand importer should present title, intent and substantial conversation as one navigable unit, with a separate pending-intent state when no result exists; byte size alone cannot choose that unit's knowledge value.

Frozen real-source labels comprise **20 spam, 24 meaningful, 6 unfinished and 2 ambiguous**. Here “meaningful” includes a useful finding, intermediate observation or failed investigation, even without task completion. “Unfinished” means a concrete intended task without a substantive result yet. “Ambiguous” preserves missing referenced material or unresolved context. Labels were written before inference, based on local excerpts and available tool evidence. The real examples were not rewritten to make model judgments easier; URL/path masking, boilerplate exclusion and declared selection/truncation are input transformations. All real cases were frozen evaluation cases; the paired follow-up reuses 12 of them, so it is not another independent holdout.

Concrete discoveries undermine length-based heuristics:

- Codex and Hermes contained ordinary two-message “test” exchanges.
- Hermes also contained two-message unprocessed requests of roughly 9–12 KB, largely skill text. They have pending intent, not 10 KB of useful findings.
- A short user “continue” can belong to a rich task history. Its usefulness depends on surrounding history, not the utterance length.
- Interrupted work sometimes contains useful observations in tool records. Removing all tools and retaining an interruption notice loses those observations.
- OMP assistant records with thinking/tool calls often have no ordinary text. “No assistant text” is not “no work happened.”

The deterministic baseline filters empty/test/control-only interactions, retains substantive assistant text or substantial tool outputs, and otherwise labels a concrete user request unfinished. It is schema-aware and does not treat byte length as the quality judgment. It matched **50/52** frozen real labels; both remaining disagreements involved the ambiguous labels. Jev matched **49/52**: **34/37** full-readable cases and **15/15** selected-message cases. Different cases used different views, so those two ratios cannot establish that selection improves quality.

The Jev errors were an incomplete pasted request labeled spam, an underspecified command labeled unfinished instead of ambiguous, and a skill/handoff-only intent labeled spam instead of unfinished. These are retained in the aggregate error ledger. The two ambiguous labels admit reasonable alternatives; do not turn the two-point baseline advantage into a general superiority claim. There is no evidence here that a semantic model should replace cheap deterministic rejection of obvious empty/control starts.

## Representations and evidence loss

The initial real run used all cleaned readable/tool message text when at most 24,000 characters; 37 cases met that bound. For 15 longer cases it used the first four and last four messages, each clipped to 2,500 characters. “Full readable” is the declared readable source projection, not hidden reasoning, all protocol metadata or the whole agent execution environment. The adapter must preserve original records separately.

Twelve authored controlled threads tested user adoption and actual verification separately. Eight were development cases and four frozen domain-shift holdouts. Each used three representations: the full short readable thread; a last-assistant-message extractive summary; and conversational messages with tool evidence omitted. These are extractive summaries, not a test of a supporting generative summarizer.

| Controlled representation | Correct against evidence available in that representation | Matches underlying full-source conclusions | Interpretation |
| --- | --- | --- | --- |
| Full readable thread | 22/24 | 22/24 | Preserves actors, modality and measured outcomes; two initial errors remain. |
| Extractive summary | 24/24 | 5/24 | All 24 outputs were `unknown`. Good abstention establishes almost no original conclusions. |
| Conversation without tools | 23/24 | 15/24 | User adoption mostly survives; all 12 actual-result judgments become unknown. |

The full-source failures were an assistant recommendation treated as unknown where the label regarded the user's no-implementation request as explicit non-adoption, and a disabled scheduler mistakenly called failed even though the target proposition was “scheduler is disabled.” The first remains a rubric ambiguity. Two follow-up diagnostic calls asked whether tool evidence supported the exact proposition irrespective of desirability: the scheduler case and a genuine failed-importer case both matched the labels. Those are prompt diagnostics, not repaired holdout metrics; the initial failure stays recorded.

A separate deliberately simple user-keyword adoption comparator matched **4/12** full-thread adoption labels; Jev matched **11/12**. With summaries, the keyword comparator also scored 12/12 against evidence-relative unknown labels because no user messages remained. This is an illustrative attribution baseline, substantially weaker than the schema-aware quality comparator; it shows why superficially directive words cannot establish accepted decisions, rather than proving that every contextual judgment requires inference.

The paired real follow-up sampled three cases per application, including spam, pending intent and substantive findings. Reusing the initial readable/selected states, extractive summaries matched the original quality label **6/12** times; selected conversational excerpts matched **8/12**. These are source-label retention measures, not evidence-relative classifier error rates: some mismatches correctly reflect genuinely missing evidence. Several real tool-supported reviews collapsed to “unfinished,” “ambiguous” or “spam” after their outcome evidence was omitted.

Across those 12 paired input states, extractive summaries retained **8,064 of 89,815 characters (9.0%) and 9 of 90 message IDs**. Selected conversation retained **17,143 characters (19.1%) and 25 of 90 message IDs**. Neither view retained tool evidence; summaries retained no user messages. They were derived from already selected states in two longer cases, so this is incremental loss, not total loss measured against every original record.

These losses remain recoverable because private original/extracted records and stable message anchors survive. Deleting raw histories would make missing observed errors, decision rationales, exact source qualifiers and command/output linkage irrecoverable. A knowledge item should carry source thread/message/revision anchors and the view used to judge it; navigating to a full history is different from promoting every message/tool call to a wiki item.

## An informative adapter failure

The first OMP readable parser preserved text and `toolResult` messages but missed assistant `toolCall` blocks. The frozen initial study therefore did not contain all execution requests. A separate v2 parser audit, retained as [adapter diagnostic](experiments/adapter-diagnostic.json), adds **206 tool-call records** across the five nonempty OMP samples: 3, 8, 20, 46 and 129. The 88 initial calls and their fixtures were not replaced or relabeled.

The public runner now includes this correction. Re-inventorying with the current parser will produce richer OMP sources; exact replay of this study uses the private frozen request sets. This separates source conversion failure from semantic model failure. Neither the parser nor the study reconstructs the active OMP/Claude branch from parent links or fully resolves Hermes compression/active-state transitions. Those are adapter requirements to validate before production ingestion.

The initial projection contains 544 user, 1,120 assistant, 2,116 tool/tool-result and 35 developer records across selected sources. These counts describe the tested projection, not the complete available histories; the 206 extra v2 OMP calls are excluded. This is concrete tool volume to navigate selectively, rather than evidence that every tool result should become a separate wiki item. Boilerplate and control filtering, branch resolution and completion interpretation remain distinct from choosing a human-readable title.

## Exact live ledger

The [TypeSafe API](https://docs.typesafe.ai/api) and [state guide](https://docs.typesafe.ai/concepts/state) were read live; the latter requires text/structured text inputs. The [OpenRouter compatibility guide](https://openrouter.ai/docs/guides/community/typesafe-sdk) confirms the System One endpoint and extra response usage fields. The [pre-parsed extraction cookbook](https://docs.typesafe.ai/cookbooks/pre_parsed_value_extraction_cookbook) informed code-owned candidate capture. This interpreter lacked the SDK, so the runners used the documented wire API directly with the existing configured credential, never displayed or copied into an artifact.

| Run | Requests/successes | Input tokens | Output tokens | Reported USD | Median / p95 request seconds |
| --- | --- | --- | --- | --- | --- |
| Frozen real quality + controlled representations | 88 / 88 | 147,894 | 7,363 | 0.006211548 | 0.276 / 0.557 |
| Paired real representations + narrow verification diagnostics | 26 / 26 | 20,228 | 1,297 | 0.000849576 | 0.261 / 0.342 |
| Combined | **114 / 114** | **168,122** | **8,660** | **0.007061124** | **0.270 / 0.557** |

Every successful response resolved to **`typesafe/jev-1.13-20260917`**. The initial and follow-up concurrent runs took 7.486 and 1.927 seconds respectively. No service failures occurred and no failed-request billing is unknown in this suite. Reported cost is response accounting, not account reconciliation; coding-agent work and other research concerns are excluded. No supporting generative model was called in this concern.

## Supported next experiment

Prototype on-demand thread selection with a schema-aware spam gate, separate pending-intent state, a readable thread view and lazy original-tool drill-down. Test branch-aware adapters and source-hash/version checks before treating any adopted decision as durable knowledge. Keep the URL occurrence graph independent of knowledge promotion. This is a comparison candidate, not an accepted storage design.

The next evaluation should reserve naturally selected whole task episodes with independent human labels, including reversals spread across multiple threads. Compare a concise generative summary against source-ID selections and original evidence, measuring recoverable qualifiers and incorrect promotion. This suite did not test generated summaries, whole-corpus prevalence, user preference, production durability, multilingual quality or an end-to-end UI.
