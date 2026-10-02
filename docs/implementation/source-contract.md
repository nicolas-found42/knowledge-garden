# Durable source contract, schema 1

This is the minimum source/page representation for [issue #2](https://github.com/nicolas-found42/knowledge-garden/issues/2), following the Markdown/original and Tauri decisions in [specification #1](https://github.com/nicolas-found42/knowledge-garden/issues/1).

```text
collection/
  sources/<source-identity-digest>/
    index.md
    original.<supplied-extension>
    acquisitions/<context-hash>.md   # Additional acquisition contexts, when present
  .derived/lookup.sqlite            # Disposable lookup index
  .staging/                        # Uncommitted import work
  .collection.lock                 # Process lock, contains no knowledge
```

`index.md` has ordinary YAML frontmatter and a readable body. Frontmatter records schema version, source/page identities, original name, retained asset link, content SHA-256, byte length, format, extraction coverage, line count, and the initial acquisition path/method/receipt time. Receipt time is Unix epoch milliseconds; it is not a source-described event date. An ordinary relative Markdown link opens the original independently of the app.

For supported text the body contains an explicit original line range and a fenced verbatim transcription. Fences are longer than any run of backticks in the source, keeping source headings and YAML from becoming generated metadata. UTF-8 BOMs are omitted from the displayed transcription, while the asset preserves them. No semantic facts, relationships, model confidence, or inferred dates are created by this importer. `text_preserved` means text preservation only; coverage explicitly states that semantic fact extraction has not run. Other states are `unsupported`, `invalid_utf8`, and `too_large`.

The first source identity is `source-<source-identity-digest>` and its page gets a persisted UUID. The identity digest is SHA-256 of the normalized extension, a NUL separator, and the lowercase hexadecimal SHA-256 digest of the original bytes. The separate `sha256` field records the original bytes alone. In this acquisition slice, identical bytes with the same extension share the first source/page even if selected through another path. Different extensions stay separate because format changes extraction coverage and the retained asset name. Later source-version reconciliation must preserve these established identities while adding reliable logical-source/version ordering; a changed file currently imports as a separate source. A repeated path/method context adds no new acquisition record. A differing context gets its own small Markdown record linked to `index.md`. Reimport never rewrites the source page, including its external annotations or wording. Extra acquisition records are provenance, not new source pages or facts.

Copying and hashing use 64 KiB chunks. Text decoding stops at 2 MiB; larger originals still copy in full. Source-page reads have a roughly 6 MiB bound, and page lists have 50 entries with an explicit continuation. Native disk operations run on a background worker. This is bounded memory and result handling, not a measured throughput claim for 250,000 sources. Index reconstruction scans bundles one at a time at collection open; progressive bulk startup/reconciliation remains future work.

The original and source page are written and synchronized inside a temporary bundle. A directory rename publishes them together, then the lookup index is updated. If a process stops before publication, staging content is excluded from navigation. If it stops after publication but before indexing, the next collection open reconstructs the index from the published Markdown. Stale staging directories are not treated as committed sources. Failed imports return an error without replacing the currently displayed page.

The SQLite index contains only source summaries. Delete `.derived` while the app is stopped and reopen to reconstruct it from Markdown. This does not rewrite pages, originals, or provenance. A collection lock prevents cooperating application processes from writing simultaneously. The app reads an externally edited valid page when opening it; arbitrary metadata corruption, renames/moves, source-version reconciliation, and concurrent external-editor recovery remain later lifecycle work. Retained originals must be treated as originals; opening them uses macOS's file handling.
