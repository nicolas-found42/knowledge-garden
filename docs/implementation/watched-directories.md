# Watched directories

The reader's **Watch directory** action selects a source boundary once. A bounded
polling watcher repeatedly reconciles it through the ordinary import pipeline.
Polling also supplies restart, sleep, missed-event, and atomic-replacement recovery;
filesystem event delivery is not required for correctness.

Watch records and per-file receipts live in readable Markdown under `watches/`.
They retain directory identity, a scan generation, a directory frontier, completed
scan progress, stable source identities, and acquisition origins for relative
pathnames. Identical content shares the existing importer's source identity.
A restarted incomplete directory is enumerated again, while
receipts prevent reimporting files already processed in that generation. Completed
directories and retained sources do not need to be processed again to resume.

Only successful, complete scans can supply absence evidence. A missing file must
be absent in two consecutive completed scans before its retained source is removed.
Before removal, the watcher checks directory identity and the nearest existing
parent again. Missing roots, changed device/inode identity, unreadable subdirectories,
and file-read failures expose unavailability and prevent removal. A fresh empty
mount point at the same path is not evidence of deletion. Selecting a directory
again explicitly accepts its current filesystem identity.

For macOS disk volumes, a persisted volume UUID permits device-number changes on
remount. Nested mount points are tracked too, so an empty mount point inside a
watched directory cannot supply deletion evidence. Filesystem services that cannot
provide a volume UUID retain the conservative device/identity fallback.

Symlinks and collection-owned directories are not traversed. Each worker turn has
a bounded discovery/removal budget and releases the application lock between turns.
Existing pages remain readable as new material is discovered. This is not a claim
of validation at the parent specification's 250,000-source scale; high-volume
scheduling and background login/menu-bar lifecycle have separate tickets.

Discovery continues past unreadable files, while the incomplete scan is barred
from withdrawing sources. Watching an already imported pathname adopts its source
identity and preserves any independent acquisition origins. Visible Sources and
open pages refresh as reconciliation progresses.

Confirmed removal withdraws only that source's fact, relationship, and tag support.
Independent support and external manual corrections survive. A recoverable
publication transaction writes reconciled Markdown before removing source assets;
restart completes an interrupted publication before rebuilding disposable indexes.
The running application also finishes pending publication intents before accepting
later imports, directory reconciliation, or semantic/audio work, so an old removal
cannot replay over a newly reimported source on a subsequent restart.
Manual pages with no remaining source remain searchable with unavailable evidence.

The existing importer and source-withdrawal path rebuild the collection index and
reconcile affected Markdown while holding the application lock. Discovery is
bounded and resumable, but individual imports/removals can still delay reader
commands; collection-scale latency and incremental index publication remain
unvalidated here and belong with high-volume scheduling.

Validation environment: macOS 26.6 (25G72), Apple M5 Pro / Mac17,9, 48 GiB RAM,
Rust 1.99.0, Node 24.21.0. The integration fixtures use temporary collections and
deterministic semantic-provider responses, with HFS+ disk images for real mount,
disconnect, empty-mount-point, and remount checks at both root and nested paths.
No live model-quality evaluation, physical USB-volume test, or 16 GiB/250,000-source
performance result is claimed by this slice.
