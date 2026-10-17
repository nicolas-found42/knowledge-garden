# Repository evaluation labels

Prepared before the live application run. Only `garden/` is supplied to the importer.

| Acquired file | Expected knowledge | Exact supporting lines |
| --- | --- | --- |
| `src/query/cache.rs` | A Cache concept describing parsed query plans, including the test-only qualification | 1–2 |
| `src/http/cache.rs` | A distinct Cache concept describing HTTP responses | 1 |
| `docs/adr/0001-local-plans.md` | A decision “Keep query plans local” with the test-only qualification | 4–5 |
| `README.md` | An unanswered question “Should cache eviction use LRU?” | 5 |

The remote repository contributes no acquired content or knowledge. No symbol identity is inferred across the two Cache files. Every evidence span must resolve to the retained original's acquired content revision, including after restart and unchanged repeat import. This set qualifies these explicit supported constructs only; it measures neither arbitrary-language understanding nor general semantic accuracy.
