# Source anchoring laboratory

[Report](../../2026-09-29-navigation-and-provenance.md) · [Concern](../../README.md)

Three real public source excerpts, eleven controlled mutations each, three deliberately simple deterministic resolvers. No live model calls. This is not the Hypothesis runtime; pinned Hypothesis files informed the questions.

- `source-manifest.json`: independently selected public documents and pinned implementation files, acquisition status, hashes and local cache names. No conversation links feed this manifest.
- `results-initial.json`: full cached document experiment, preserved.
- `portable-fixtures.json` and `results-portable.json`: short frozen public excerpts, with original extraction hashes. Same transformations and metrics; not independent extra evidence.
- `audit.py` and `audited-results.json`: separate recovered targets, abstentions and ambiguity; actual disk-backed source lookup with corruption/missing/offset controls.
- `snapshots/`: three small public excerpts keyed by content hash.
- `fault-injection/`: deliberately corrupt test copies, isolated from originals.
- `development-failures.json`: the fixture-construction error that was caught before any result was written.

Offline replay from the repository root:

```sh
python3 docs/research/expanded-exploration/04-navigation-and-provenance/experiments/provenance-lab/run.py --portable --output /tmp/knowledge-garden-anchor-replay.json
python3 docs/research/expanded-exploration/04-navigation-and-provenance/experiments/provenance-lab/audit.py
```

The runner refuses an existing output file. `audit.py` deterministically rebuilds derived audit records and tiny snapshots. Python standard library only. To replay against the full originally captured text, omit `--portable` and retain the public-provenance cache named in the manifest. Optional `--fetch-public` accesses only the three source URLs in the manifest, validates their historical byte digests and refuses changed pages. Reacquiring a mutable page may require a new dated source manifest; it must not rewrite the old run.

The initial in-memory original slice check is not a durability test. The disk experiment tests only these three short public snapshots and nine deliberately invalid references. Internal whitespace mutations concern exact character sequences. Whole-document duplication invokes a conservative uniqueness policy, not an oracle claim about semantic identity.
