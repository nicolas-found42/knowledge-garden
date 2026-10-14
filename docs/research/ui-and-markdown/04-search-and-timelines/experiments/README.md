# Reproduce the corpus, search and timeline experiments

[Report](../2026-09-29-search-and-timelines.md) · [Concern](../README.md)

Run commands from the repository root. Ordinary offline search and time checks require only Python 3 with SQLite FTS5:

```sh
python3 docs/research/ui-and-markdown/04-search-and-timelines/experiments/search_lab.py
python3 docs/research/ui-and-markdown/04-search-and-timelines/experiments/timeline_lab.py
```

These regenerate deterministic offline outputs. Copy the experiment directory before rerunning measured extraction/corpus generation if preserving every timing matters. `search_lab.py` refuses a mismatch between existing frozen labels and the corpus. The label file was written before the first provider request. The authored 16 questions and six answer cases are development fixtures, with no independent holdout.

`build_corpus.py` requires `reportlab` and `python-pptx`, and the retained earlier public media files. It constructs controlled files and copies five earlier public/controlled artifacts without modifying their originals. `public_asset_checks.py` requires `pypdf`, Pillow and `ffprobe`. The local bundled interpreter used was `/Users/Nicolas/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3`; use the app's workspace-dependencies discovery if that path changes. Do not silently rebuild the frozen corpus before interpreting its recorded model outputs.

The authorized live run used the configured `OPENROUTER_API_KEY` only in the Authorization header:

```sh
python3 docs/research/ui-and-markdown/04-search-and-timelines/experiments/search_lab.py --live
```

Every live replay creates a fresh timestamped directory, retains complete public requests/responses and errors, performs no automatic retries, and logs usage and elapsed time. `run-20260930T004745Z/` is the original run. Four retrieval workers ran concurrently; answer generation and verification ran in dependent sequence. No claims about isolated provider latency or bill reconciliation follow. A missing model or network failure remains a failure; do not overwrite the previous run to repair it.

`private_format_probe.py` is an opt-in **personal-file read** covered by this session's recorded research boundary. Do not run it in a different authorization context merely to reproduce public results. It scans bounded non-Git Documents paths, chooses at most one eligible file per extension by hashed path, retains exact mappings/copies only in a mode-0700 `KnowledgeGardenResearch` directory under the user's Application Support, and writes only structural aggregates to this repository. Selected copies are mode 0600. The original-byte hash is checked again after the probe. No private text goes to an API. The public aggregate is sufficient to understand the limits; replay naturally changes as personal files change.

`sources/manifest.json` records direct primary-document acquisition and failure. The OpenRouter compatibility guide was successfully read with the web tool even though direct HTTP returned an error. Two TypeSafe cookbook requests failed in the web tool and succeeded by direct public Markdown capture. Neither failure implied a provider/API failure. Conversation URL destination acquisitions remain zero.
