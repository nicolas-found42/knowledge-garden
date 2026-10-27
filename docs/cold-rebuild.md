# Cold rebuild and recovery gaps

The collection's Markdown records, source versions, retained original assets,
watched-directory records, saved views, trails, owner edits, publication receipts,
and page-location ledger carry durable state. The `.derived` directory contains
disposable lookup and local meaning indexes. Work only on a disposable fixture when
checking cache deletion; never remove stores from an owner's collection as a test.

To exercise a cold rebuild on a temporary fixture, stop its application, snapshot
the durable files and public results, remove that fixture's `.derived` directory,
then reopen it. Reconciliation reconstructs lookup tables from current source and
knowledge Markdown and rebuilds local meaning vectors when the model is available.
Interrupted publication receipts are recovered before scanning. Explicitly saved
view criteria and ordered trail stops are read from their Markdown files.

Unreadable or mismatched source records are not chosen for indexing or processing.
They are retained and reported through collection diagnostics; a missing source
`index.md` is a separate diagnostic. Healthy sources remain searchable and queued
work remains schedulable. Knowledge whose supporting source is unreadable remains
available as retained Markdown with source-unavailable status. Path exploration
reports missing support instead of fabricating evidence. Moved page identities are
resolved to their current Markdown path before looking up a search evidence span.

The destructive-to-cache fixtures cover retained source bytes, pending work, moved
pages, external manual corrections, source replacement, independent support,
saved criteria and trails. They compare search and evidence outcomes after a cold
restart and verify malformed and missing metadata remain intact and visible as
recovery gaps.

Reconciliation scans directory entries with an explicit per-call budget and the
SQLite index update is transactional. A temporary file-backed SQLite lookup resolves
unique page paths once per rebuild instead of rescanning every page for each source. The final index rebuild and initial open
still complete a whole-collection pass synchronously; this does **not** yet meet a
bounded end-to-end rebuild requirement at release scale. Interruption and large
collection responsiveness require further work and qualification; do not treat
these tests as completion of #30 or the 16 GB scale gate.
