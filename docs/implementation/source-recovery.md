# Source recovery

> **Development status:** A late external save can race the current copy-and-delete removal path. A fix using atomic displacement is under isolated verification along with migration recovery. Do not use the current development checkout to remove sources from a valuable collection until [the acceptance gate](../pivot-acceptance.md) passes.

Removing a source creates a recovery copy before removing active evidence. The copy is separate from whole-collection backups and is not indexed. The original file outside the collection is not deleted.

Each copy is stored at `.recovery/sources/<uuid>/bundle/`. It contains the retained source bundle, including original assets, version assets, source metadata, and acquisition records. The copy is compared byte for byte with the source bundle before the removal record is written. `removal.md` records the source, its origins, the source generation, any watched receipt, and a digest of the copied files. The digest is checked before removal and restore. `completed` marks a finished withdrawal. A `restore-pending` file records an interrupted restoration. The application finishes pending removal and restore records when it opens a collection and before desktop commands.

The application stores source generations in `.recovery/generations/`. Semantic jobs carry the generation they claimed and cannot publish after it changes. Explicit removal stores suppressed origins in `.recovery/suppressed-origins/`. Watched scans skip those origins until an explicit re-add or restore. Missing watched files are backed up before their acquisition is withdrawn, but their origins are not suppressed.

The source list provides recovery copies. Restore reinstates the complete retained source bundle and its version history. It queues the pending version, if one exists, or the most recently published version for knowledge processing. The recovery copy remains available.

The backup uses the collection filesystem. A separate whole-collection backup is needed to recover from loss of that filesystem.

Each source version records the origin, acquisition method, and registration identity that explicitly supplied its bytes. When a watched origin disappears, the application removes that origin from the version records. It withdraws knowledge whose active version has no remaining recorded origin and queues a supported remaining version for processing when its order is known. A withdrawn version stays in recovery storage and cannot be opened through an active version link. Older records without origin attribution remain unknown until reacquired.

Watched directories and repositories have separate registration identities. New acquisitions record their registration, so removing one registration withdraws only its own support. The application journals an unregister operation and resumes it on collection open. It does not delete the selected directory or repository. Another registration or a separately picked file can keep the same source active.

For older repository records, the application exposes `legacy_explicit` only when the repository root, repository identity, source identity, member path, and the single recorded legacy acquisition agree. It exposes other legacy records as `legacy_ambiguous` and does not remove them automatically. Older watched acquisitions that lack a registration identity remain preserved because their ownership cannot be assigned safely.
