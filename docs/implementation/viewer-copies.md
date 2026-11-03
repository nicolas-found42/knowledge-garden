# Verified viewer copies

Opening an original in an external viewer first checks the retained bytes against the source version digest. The app opens a private, read-only copy under `.derived/opened-originals/<digest>/`. That directory is outside the collection's source and page scans. Reopening the same content and format reuses the verified copy.

Copies remain available after the app closes because the external viewer can open the path after the launch command returns. The app does not currently delete these copies automatically. They may occupy disk space up to the total size of distinct opened originals. They are disposable and can be removed when no external viewer still needs them. A changed retained original is not opened as evidence; the observed changed bytes are retained in `.recovery/external-originals/` and the source reports an integrity failure.
