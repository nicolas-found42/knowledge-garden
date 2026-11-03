# Source removal and migration constraints

This is a design note, not an implemented guarantee. The current `withdraw_watched_source` removes the source bundle after reconciliation. It does not meet recoverable removal. Watch scans can import that origin again, and semantic inference can finish outside the collection lock.

The durable lifecycle needs a source generation and an active or removed state outside both the source bundle and the disposable index. A removal must first copy the complete retained bundle into a non-indexed recovery area, verify the copied bytes, and persist the removal intent. It must then reconcile supported knowledge through the publication journal. Recovery must finish interrupted removal before serving active retrieval. Every in-flight job must compare its claimed generation with the current generation before publication. Explicit re-add creates a new generation, even for identical bytes.

Watched origins need durable suppression keyed by watch and relative path. Ordinary scans, changed bytes, overlapping watches, and restarts must not clear suppression. Explicit source re-add can clear it. Unregistering a folder or repository must never delete the original filesystem contents. Sources independently acquired elsewhere need separate acquisition handling.

Migration must first create and verify a recoverable collection backup. It must snapshot authored corrections and owner-only material as immutable, qualified source evidence while removing their ongoing authority over generated facts. Old pending publication transactions must be resolved before the snapshot. Future external page edits must not promote knowledge, including edits that arrive during publication. Preserve displaced bytes in recovery storage.

Test each crash boundary, stale job after removal and same-byte re-add, changed watched file, overlapping watches, independent claim support, index exclusion, migration restore, and external edits during publication. The final test must use the application with isolated collections. No part of this note authorizes destructive testing against an existing collection.
