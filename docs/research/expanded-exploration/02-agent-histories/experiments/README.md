# Agent-history experiments

[Concern report](../2026-09-29-agent-history-investigation.md) · [URL experiment](../2026-09-29-conversation-url-linking.md)

This folder contains reproducible standard-library Python runners, explicitly synthetic controlled histories, opaque frozen real labels, metadata and aggregate outcomes. It intentionally contains no private source history, private URL, exact source mapping, raw API state/reply or credential. No SDK installation is required by these runners.

| Artifact | Purpose |
| --- | --- |
| [inspect_sources.py](inspect_sources.py) | Read-only metadata-stratified inventory and source projection; uses actual observed stores and private output directory. Current parser includes the later OMP tool-call correction. |
| [url_occurrences.py](url_occurrences.py) | Local HTTP(S) candidate capture, offsets and private identity/hostname adjacency; no destination acquisition code. |
| [test_url_occurrences.py](test_url_occurrences.py) | Seven deterministic tests using synthetic reserved-domain references; no network. |
| [run_jev.py](run_jev.py) | Frozen live typed quality/adoption/verification evaluation; every exact request and response stays private. Uses the documented wire endpoint with the existing environment credential. |
| [followup.py](followup.py) | Paired real representation loss and two separate proposition-verification diagnostics. |
| [synthetic.json](synthetic.json) | Twelve controlled threads: eight development and four frozen holdout cases; three representations each. |
| [labels.json](labels.json) | 52 frozen opaque real labels; two remain ambiguous. No source path/title mapping. |
| [sample-metadata.json](sample-metadata.json) | Opaque initial sample sizes, roles and ranks. Initial parser counts are preserved. |
| [adapter-diagnostic.json](adapter-diagnostic.json) | First-parser versus corrected OMP tool-call counts; conversion failure is not hidden by replacing initial results. |
| [aggregate-results.json](aggregate-results.json) | Exact usage/cost/model ledger, opaque failure IDs, representation metrics and final URL aggregates. |

Private evidence is under `~/.local/share/knowledge-garden/research/expanded-2026-09-29/agent-histories/`, permission `0700`, with files tightened to `0600`. It contains the initial inventory/extracted sources, 39 selected raw JSONL snapshots and hashes, selected Hermes records, frozen prepared cases, initial/follow-up replies, latency and usage, corrected OMP projections, full occurrence/adjacency maps, and retention measurements. This path is intentionally outside Git; moving public aggregates does not recreate the private evidence.

Offline checks:

```sh
python3 -m unittest discover -s docs/research/expanded-exploration/02-agent-histories/experiments -p test_url_occurrences.py
python3 -m py_compile docs/research/expanded-exploration/02-agent-histories/experiments/*.py
```

Local re-inventory, with an explicitly private output location:

```sh
python3 docs/research/expanded-exploration/02-agent-histories/experiments/inspect_sources.py --private-dir /absolute/private/research-directory
python3 docs/research/expanded-exploration/02-agent-histories/experiments/url_occurrences.py --private-dir /absolute/private/research-directory
```

Live runs require authorization for selected personal excerpts and API usage plus the existing `OPENROUTER_API_KEY`. The run scripts refuse to overwrite initial/follow-up result files. They capture service errors as private records and do not silently retry. The initial 114 calls had no service failures; semantic and representation failures remain in the aggregates. Exact replay uses the frozen private request sets: source files are mutable, and the current OMP parser intentionally differs from the preserved initial projection. Reusing the selected source mappings on another machine without the actual histories is unsupported.

The baseline is a code-owned schema-aware empty/test/control filter plus substantive-output/tool presence checks. It is a meaningful cheap ingestion comparator, not an attribution verifier. Controlled adoption comparisons use a deliberately simple keyword baseline in the runner but do not claim it is a capable alternative for contextual decisions. No production importer, generated-summary baseline or whole-history branch reconstruction was validated.
