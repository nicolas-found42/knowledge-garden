# Audio recordings: implementation and qualification

[Issue #14](https://github.com/nicolas-found42/knowledge-garden/issues/14) uses the bundled local transcription route selected in [ADR 0007](../adr/0007-bundle-local-audio-transcription.md). Original recording bytes remain authoritative. Transcripts are searchable, machine-generated evidence, with estimated timestamps and unidentified speakers. The reader opens the retained recording in QuickTime Player and discloses manual seeking; it does not promise sample-accurate playback.

## Acceptance evidence

The application checks below live in `src-tauri/tests/audio_workflow.rs`; reader checks live in `src/reader.test.ts`. They exercise public application operations on temporary collections and the reader's desktop-command boundary.

| Issue criterion | Evidence and limits |
| --- | --- |
| Import representative formats, retain originals, inspect source pages and play audio | `pinned_local_whisper_import_retrieval_evidence_restart_and_seek_fallback` imports real PCM WAV; `pinned_local_whisper_accepts_aac_m4a_through_application_import` imports AAC/M4A. Both compare original bytes. Playback delegates to macOS file handling; other containers/codecs are not qualified by these two fixtures. |
| Preserve timestamps, source origin, wording and speaker uncertainty | The frozen two-voice correction fixture checks independently authored utterances and timestamp bounds within 250 ms. Transcript segments have no authenticated speaker or calibrated confidence. The source page discloses those limits and the quiet gap. |
| Link facts, questions, decisions and reasoning to supporting passages, with seek fallback | `acquired_speech_questions_decisions_and_reasoning_publish_with_segment_evidence` checks speech-act formation with recorded semantic responses. The real Whisper correction test checks qualified knowledge and search. Timestamp links now include the retained version and asset; the replacement test and reader version test protect navigation after replacement/restart. Existing source-only links remain readable. Semantic recordings establish integrity, not fresh live semantic accuracy. |
| Expose silence, overlap, noise, partial decoding and unsupported coverage | `real_local_acoustic_challenges_remain_qualified_searchable_and_retained_after_restart` runs four frozen synthetic conditions. Coverage stays partial even when processing is complete. `native_undecodable_recording_keeps_exact_original_and_explicit_failed_coverage` and invalid-envelope tests protect failed input. The recognizer cannot distinguish silence from missed speech; overlap can omit a voice. |
| Bound and resume long work without duplicate knowledge | `imports_original_then_processes_bounded_resumable_audio_with_stable_passages` exercises three windows with an interruption after a durable claim; restart retains segment identities. Failure and interruption budgets are durable per version. This is recovery evidence, not a many-hour performance measurement. |
| Validate actual capabilities and retain resumable work on failure | Real ASR uses the pinned bundled whisper.cpp runner/model with native AVFoundation decoding. No remote audio capability or runtime download is assumed. Missing-model, malformed semantic-response and failed-segment tests leave retained originals and explicit processing state. Semantic service availability is separate from local audio transcription. |
| Use labeled recording fixtures for retrieval, time location, uncertainty, recovery and coverage | `frozen-two-speaker-uncertain-correction.labels.md` supplies statement/correction/quiet-gap labels before ASR. `heldout-spoken-acts.labels.md` and `audio-challenges/independent-labels-before-asr.md` add frozen source-only expectations. These are synthetic, agent-authored labels, not human acoustic ground truth or a broad diarization benchmark. |

## Version-bound audio navigation

New source transcript and knowledge-page timestamp links carry the source identity, millisecond offset, source version identity and retained asset name. The reader calls `open_original_version`, whose application path resolution validates the version and asset within the collection. The first acquisition resolves to the contained current original while its identity matches; after replacement, its archived original supplies that version. It reports an opening failure instead of substituting another recording. Existing links without version metadata keep their original current-recording behavior; old owner-edited Markdown is not silently rewritten.

Successful semantic publication refreshes the source body and knowledge links after promoting the replacement transcript, so displayed words and timestamp links agree with the published recording. An uncertain replacement keeps the published transcript and retained candidate separate across restart, as checked by `uncertain_audio_replacement_preserves_published_transcript_and_candidate_after_restart`. Audio opening explicitly selects QuickTime Player; this avoids Music library import/onboarding when Music is the default handler. Other original formats retain normal macOS file handling.

The external player opens at its own position. The reader displays the requested minute/second and explicitly says precise seeking is unavailable. Timestamp alignment of the transcript is a separate check from external-player seek control.

## Validation run — October 16, 2026

Environment: Apple Silicon `arm64`, macOS 26.6 (25G72), 48 GiB RAM, Node.js 24.21.0, Rust/Cargo 1.99.0. This host is larger than the specification's 16 GB release target; these checks do not establish target-memory qualification.

The debug application bundle was built with `npm run tauri build -- --debug --bundles app` and launched with `KNOWLEDGE_GARDEN_COLLECTION` pointing at a disposable collection. This qualifies an unsigned development bundle, not a release installation.

Native observations:

- The macOS picker imported `frozen-two-speaker-uncertain-correction.wav`. The retained WAV matched all 502,486 input bytes (SHA-256 `1ffb366754ade2d06555106b9bbbf23d3779fac18f3ce6899d341ca7a7b64223`). Bundled local Whisper produced the two expected passages at 0–4300 ms and 5440–11380 ms. The reader displayed the 4300–5440 ms gap, estimated-time wording, unavailable confidence, and unidentified speakers.
- Searching for `count could be 15` returned the source and its uncertain passage. After stopping and relaunching the bundle against the same collection, the source and transcript reopened.
- Opening the second timestamp displayed “Seek to 0:05 in your audio player; precise seeking is unavailable in this reader.” QuickTime opened the retained collection's `original.wav`, with an 11-second duration. Playback advanced to six seconds before being paused. This establishes playable retained audio and the disclosed fallback, not automatic seeking to 5440 ms.
- The initial native run exposed a missing first-version archive and the default Music onboarding dependency. Application regression tests reproduce the path failure; version resolution and explicit QuickTime routing correct those opening paths.
- The fresh semantic run recorded `typesafe/jev-1.13-20260917` and produced a qualified recording anchor with no facts or relationships. Speech-act/fact formation evidence in the criterion table uses deterministic semantic responses; this native run does not establish fresh live semantic extraction accuracy.
- The final bundle also imported the AAC/M4A companion through the native picker in a fresh collection. All 112,782 retained bytes matched the fixture (SHA-256 `f0b46171207bdd9ca3318a44aa20d45106eaa366dff14d4f5b47f2557cb5d189`). The two transcript passages and restored **Knowledge → Recorded speech** navigation appeared. Opening the second timestamp showed the same `0:05` fallback and opened the retained `original.m4a` in QuickTime; playback advanced to eight seconds. The knowledge link opened the recording anchor with a return link to its source.

Final automated checks:

- `npm test` ran 41 UI tests and 163 Rust tests successfully, including all 22 audio tests. The final rustdoc stage then failed with `E0463: can't find crate for tauri` after an overlapping package build reused the target directory. The isolated rerun, `cargo test --manifest-path src-tauri/Cargo.toml --doc`, passed with zero doctests.
- `npm run format:check`, `npm run check:rust`, and the frontend build/typecheck in `npm run tauri build -- --debug --bundles app` passed. The final bundle contains both the navigation and replacement-publication fixes.

Standards review found no documented-rule violations; its duplication finding was consolidated. Spec review found the declined-replacement promotion bug; the new regression reproduced it and passed after the publication guard. Both follow-up reviews found no new actionable issues. The final Jev gate verified its five completion claims but escalated on confidence (`safe_to_apply: 0.49`, test-gap confidence `0.17`), so automatic acceptance is not claimed. A stronger-model manual review then inspected the final containment, version, publication, navigation and test paths and accepted them without a remaining actionable defect; it relied on the execution evidence above rather than rerunning checks.

## Remaining quality limits

The ASR model produces best guesses without diarization or calibrated per-segment confidence. The overlap fixture intentionally retains the observed voice omission as a limitation, rather than treating processing completion as complete acoustic coverage. A cropped recording cannot establish an unrecorded continuation. Source statements and tentative corrections remain qualified; import time is not a source-event date. Live semantic quality, human-speaker authentication, broad real-world ASR accuracy, many-hour throughput, 16 GB memory behavior, signing and notarization are outside the evidence established here.
