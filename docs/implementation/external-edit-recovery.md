# External Markdown reconciliation and recovery

Knowledge page identity lives in YAML `page_id`, independently of filename and
`title`. Keep that identity when renaming or moving a page within `pages/`.
A duplicate identity has no canonical winner. Malformed owner files are retained
without parser-driven rewrites. The reader's External file diagnostics lists
ambiguous identities, unreadable metadata, unresolved Markdown targets, and exact
owner saves retained with uncertainty; each listed Markdown file can be opened
in its ordinary external application.

The desktop worker polls every five seconds, visiting at most 256 directory
entries per scan step. It follows directories, never symbolic links. Each complete
scan reconciles ordinary Markdown links and page summaries, captures supported
manual corrections, then rebuilds the disposable search/navigation indexes.
Watcher notifications are unnecessary: atomic editor saves and directory
replacement are eventually discovered even when no event is delivered. A scan
interrupted by changed file bytes is retried. Full index reconstruction still
uses the existing whole-collection implementation; this work does not establish
250,000-source performance qualification.

`.reconciliation/paths.md` is a readable identity/location ledger. Preserve it
with the collection: it records the last unambiguous location needed to repair
links after a move while the application is closed. Losing `.derived/` does not
lose that mapping. Pages and originals remain the authoritative content.
Resolvable inline and reference-style relative Markdown links retain their
fragment/query suffix. Spaces are percent encoded; outgoing links are rebased
when the containing page moves. External URLs and targets outside the collection
are preserved. Missing targets stay explicit rather than acquiring guessed IDs.

Source publication and reconciliation use `.staging/transactions/`. A synchronized
manifest retains proposed bytes and the base observed before generation. On the
macOS release target, `renamex_np(RENAME_SWAP)` exchanges an existing destination
with a durably named file in that transaction. The displaced save survives a
crash even between replacement and conflict inspection. Creation of a previously
missing destination uses exclusive hard-link creation. Recovery completes pending
writes before rebuilding derived indexes; completed entries are checked against
their published bytes, so an owner save during an interruption is still seen.

When a destination differs from its base, preserve the exact conflicting save in
`owner-edits/<content-digest>.md`. Supported fact-row edits retain manual authority
while unedited facts take the new source values. Owner wording and metadata are
retained; changes outside that grammar remain explicitly uncertain. Recovery
snapshots are excluded from ordinary search, as in the manual-correction contract.
The owner need not approve semantic work. Continually changing destinations leave
publication pending for an automatic retry instead of silently discarding a save.

These guarantees concern ordinary file saves, including atomic replacement.
They do not claim arbitrary Markdown structure interpretation, interoperability
qualification with every Obsidian plugin, or live semantic-model quality.

## Validation environment

Validated on October 16, 2026 on Apple Silicon arm64, macOS 26.6 (25G72),
using the repository's pinned dependencies and bundled native/model assets.
Public Application fixtures use deterministic semantic responses. The mounted
reader also exercises the real Rust Application through its process adapter.
Coverage includes page moves and display names, atomic conflicting saves,
malformed and duplicate identities, directory replacement, ordinary links and
literal Markdown, legacy journals, and injected publication/reconciliation
interruptions. Commands: `npm test`, `npm run check:rust`,
`npm run format:check`, and `npm run build`. This is automated application
validation, without an installed-app/Obsidian or live semantic-quality claim.
