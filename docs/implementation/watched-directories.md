# Watched directories

The reader's **Watch directory** action selects a source boundary once. A bounded
polling watcher repeatedly reconciles it through the ordinary import pipeline.
Polling also supplies restart, sleep, missed-event, and atomic-replacement recovery;
filesystem event delivery is not required for correctness.

Watch records and per-file receipts live in readable Markdown under `watches/`.
They retain directory identity, a scan generation, a directory frontier, completed
scan progress, and stable source identities scoped to the selected directory and
relative pathname. A restarted incomplete directory is enumerated again, while
receipts prevent reimporting files already processed in that generation. Completed
directories and retained sources do not need to be processed again to resume.

Only successful, complete scans can supply absence evidence. A missing file must
be absent in two consecutive completed scans before its retained source is removed.
Before removal, the watcher checks directory identity and the nearest existing
parent again. Missing roots, changed device/inode identity, unreadable subdirectories,
and file-read failures expose unavailability and prevent removal. A fresh empty
mount point at the same path is not evidence of deletion. Selecting a directory
again explicitly accepts its current filesystem identity.

Symlinks and collection-owned directories are not traversed. Each worker turn has
a bounded discovery/removal budget and releases the application lock between turns.
Existing pages remain readable as new material is discovered. This is not a claim
of validation at the parent specification's 250,000-source scale; high-volume
scheduling and background login/menu-bar lifecycle have separate tickets.

Confirmed removal withdraws only that source's fact, relationship, and tag support.
Independent support and external manual corrections survive. A recoverable
publication transaction writes reconciled Markdown before removing source assets;
restart completes an interrupted publication before rebuilding disposable indexes.
Manual pages with no remaining source remain searchable with unavailable evidence.
