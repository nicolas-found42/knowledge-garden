---
status: accepted
---

# Rebuild derived stores through a resumable, digest-checked stage

Reconstructing a collection's derived stores is a whole-collection operation that the owner can interrupt at any point: closing the reader continues maintenance, Quit stops it mid-turn, a crash loses the process, and a sleeping Mac suspends it for hours. The rebuild therefore has to be a resumable sequence of bounded steps whose partial progress can never be mistaken for finished derived state.

Stage rebuild rows in dedicated SQLite tables keyed by a generation, and advance a singleton progress row containing the generation, a cursor, and a completion flag. A staged generation becomes live only when the whole walk finishes and the staged rows are validated against the durable Markdown they came from. An interruption leaves the previous live rows in place, so search and navigation keep returning the last consistent snapshot rather than a half-built one.

Bind each generation to a digest of everything the rebuild observed: every readable source path, its summary fields, and its source versions. Resume a stored generation only when the recomputed digest matches it. A source that changed during a rebuild, or a staged row that disagrees with the Markdown it was derived from, discards the generation and starts over instead of publishing a mixed result. This is what keeps a rebuild from resurrecting a superseded value or dropping a correction.

Walk the collection in bounded batches so memory use does not scale with the number of sources, and use temporary file-backed SQLite tables for the intermediate sorts that would otherwise accumulate in Rust memory. Ordering work through the same durable records that ordinary reconciliation reads means a rebuild is not a separate code path with its own knowledge rules: it replays the records, and reconciliation decides what is current.

Durable knowledge is never destroyed by a rebuild. Malformed or ambiguous records stay on disk exactly as written and surface as explicit recovery gaps, while healthy knowledge around them is rebuilt and searchable. The operational job state that a rebuild discards is replaceable by re-reading Markdown, so losing it cannot permanently forget a pending source change.
