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
`index.md` is a separate diagnostic, and malformed acquisition records name their
exact Markdown path. Healthy sources remain searchable and queued
work remains schedulable. Knowledge whose supporting source is unreadable remains
available as retained Markdown with source-unavailable status. Path exploration
reports missing support instead of fabricating evidence. Moved page identities are
resolved to their current Markdown path before looking up a search evidence span.

The destructive-to-cache fixtures cover retained source bytes, pending work, moved
pages, external manual corrections, source replacement, independent support,
saved criteria and trails. They compare search and evidence outcomes after a cold
restart and verify malformed and missing metadata remain intact and visible as
recovery gaps.

Reconciliation scans directory entries and inspects candidate files with an
explicit per-call budget. It restarts the scan if its validation detects a file
or directory change. Parsed page candidates are checkpointed in the disposable
SQLite index and reused only after a fresh scan matches their generation and
fingerprints. Source diagnostics are recalculated. The final
snapshot and index passes are not bounded. The SQLite index update is transactional.
A temporary file-backed SQLite lookup resolves
unique page paths once per rebuild instead of rescanning every page for each source.
Distinct relationship-support keys are also sorted through a temporary SQLite
table instead of collecting the entire set in Rust memory. Obsolete meaning-vector
keys are removed as they are read, without allocating a list proportional to all
removed pages. The final index rebuild and initial open
still complete a whole-collection pass synchronously; this does **not** yet meet a
bounded end-to-end rebuild requirement at release scale. Interruption and large
collection responsiveness require further work and qualification; do not treat
these tests as completion of #30 or the 16 GB scale gate.
