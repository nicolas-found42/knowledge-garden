---
status: accepted
---

# Publish original assets and source pages as atomic bundles

For the first local text-import slice in [issue #2](https://github.com/nicolas-found42/knowledge-garden/issues/2), publish each retained original and its source page together by renaming a synchronized staging directory into `sources/<source-identity-digest>/`. Keep the initial acquisition context and stable source/page identities in the page's readable YAML frontmatter. Keep additional acquisition contexts in their own Markdown records, so reimport can preserve provenance without rewriting an externally edited page.

Use a source identity derived from the original bytes and normalized file extension, with a persisted UUID for the page. Reimports with the same bytes and extension share a page; the same bytes under another extension keep a separate page because the declared format controls extraction coverage and the retained asset name. This establishes initial acquisition identities, not the later logical-source version policy. Different bytes remain separate until source-version reconciliation is implemented.

Use SQLite only for derived bounded page lookup. Reconstruct it from committed Markdown at collection open; a failure between bundle publication and indexing cannot lose the source. Hold a process lock for the open collection, stream originals, and bound text decoding and page results. Full bulk startup, metadata corruption recovery, and semantic/current-value reconciliation are subsequent work. The representation follows the parent specification's lasting-Markdown and original-asset decisions; the exact contract and limits are documented in [the source contract](../implementation/source-contract.md).
