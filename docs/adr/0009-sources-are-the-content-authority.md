---
status: accepted
---

# Sources are the content authority

The owner adds and removes sources. The application derives and maintains pages, facts, tags, relationships, summaries, and search indexes from source evidence. It does not offer commands that edit derived knowledge. Temporary navigation and automatically remembered presentation preferences do not change the collection's knowledge.

An external edit to generated Markdown is not an authoritative correction. Migrate existing authored corrections and owner-only content into recoverable, immutable source evidence before retiring the old behavior. Verify a backup and a restore path before changing an existing collection. Removal must be recoverable and must reconcile knowledge against remaining independent evidence.

Use local Qwen inference for generative interpretation. Jev is the only model permitted through OpenRouter and receives only evidence needed for its judgments. Because Jev is remote, the system is not fully offline. Missing models, partial extraction, and unsupported formats remain visible rather than silently falling back to another cloud model.

This product decision supersedes the manual-edit authority in ADR 0001 and ADR 0002, and the other cloud-model choices in ADR 0004. It also supersedes any interpretation of ADR 0008 that treats an external correction as authoritative. The migration, provider minimization, and removal implementation are still being verified; this decision does not assert they are already complete.
