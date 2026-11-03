# Pivot acceptance matrix

All cases begin NOT VERIFIED. A case becomes VERIFIED only after a reproducible check against the finished implementation, with its fixture, outcome, and evidence recorded here or linked here. INCONCLUSIVE does not pass.

| Requirement | Required observation | Status |
| --- | --- | --- |
| Local photograph | Correct visual claims and original-region evidence in the application | NOT VERIFIED. `real_photo_visual_claim_becomes_located_knowledge` passed with local Qwen and Jev and produced an attributed orange observation with a whole-image locator. The final desktop evidence interaction remains open. |
| Local audio | Correct audible evidence with an interval | NOT VERIFIED. A standalone Whisper fixture returned timed segments; final application evidence remains open. |
| Local video | Distinct visual and audio facts, both linked to time | NOT VERIFIED. `real_video_publishes_located_audio_and_visual_knowledge` passed with local Qwen, Whisper and Jev, publishing an attributed orange observation and visit transcript with separate video and audio locators. A separate real-model test rejected a fabricated cross-track relation. Final desktop interaction and timing remain open. |
| Extractor coverage | Common document and structured families yield useful content and locations | NOT VERIFIED. XLSX extraction tests pass. In isolated, unintegrated bundles, an email part, ZIP member and cited PDF page opened from evidence links; PDF entity-only and search-result links also opened the cited page. XLSX cell B2 and PPTX slide 2 opened as inert text re-extracted from the retained version; slide preview discloses that visual rendering is unavailable. Final combined coverage remains open. |
| Honest outcomes | Unknown, corrupt, encrypted, empty and partial sources display accurate consistent states | NOT VERIFIED |
| Automatic knowledge | Upload creates useful searchable pages, tags and supported relationships | NOT VERIFIED |
| Deduplication and change | Identical content does not duplicate; changed content updates its dependents | NOT VERIFIED |
| Contradictions | Conflicting evidence stays visible without invented certainty | NOT VERIFIED |
| Removal and recovery | Unsupported claims retract, independent support survives, restart cannot resurrect, recovery and re-add work | NOT VERIFIED. Isolated desktop removal and recovery preserved the external original, and generation-fence tests passed. A late-save race requires the isolated recovery fix before this is safe to ship. |
| Authority | User commands cannot mutate derived knowledge; internal maintenance can | NOT VERIFIED |
| Migration | Verified backup and recovery; existing and new collections remain readable | NOT VERIFIED |
| Failure recovery | Interrupted jobs, outages, malformed output and resource pressure preserve data and report failure | NOT VERIFIED |
| Provider policy | Only Jev reaches OpenRouter; other inference stays local | NOT VERIFIED |
| Idle convergence | Unchanged settled collection stops processing and making Jev calls | NOT VERIFIED |
| Runtime | Reproducible startup and health; measured footprint, workload and responsiveness | NOT VERIFIED. A uniquely identified isolated desktop bundle ran the setup checks: local Qwen reported revision `5ced39dfa4bac325dc183dd1e4febaa1c46b3ea28bce48896c8e69c1e79611cc` and remote Jev reported `typesafe/jev-1.14-20261014`. Final workload peak memory, responsiveness, setup reproduction and integrated app verification remain open. |
| Delivery | Required checks, production build, running app and remote main commit | NOT VERIFIED |
