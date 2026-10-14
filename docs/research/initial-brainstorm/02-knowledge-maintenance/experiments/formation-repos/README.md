# Repository-inspired maintenance probes

[Knowledge maintenance](../../README.md) · [Initial brainstorm](../../../README.md) · [All research](../../../../README.md)

This suite studies maintenance decisions, source-dependent refresh, evidence acquisition and recurring procedures. Its historical directory name is retained.

Four synthetic families test maintenance versus lookup, refresh impact of edited sources, selection of the next evidence fetch, and recurrent procedural draft eligibility. No private logs or external repository code are executed.

Read [the deep-dive report](../../2026-09-29-repository-deep-dive.md) for source findings, results and limitations. [summary.json](summary.json) contains compact totals. [cases.json](cases.json) preserves the 40 labels serialized before the calls; [label-audit.json](label-audit.json) identifies ambiguous labels without changing original scoring.

The plain and evidence-aware variants each ran twice: 160 successful requests, 240 labeled fields, 231 exact matches, reported cost $0.004321968. This is a development probe with repeated synthetic cases, not a held-out evaluation. The 0/10 context baseline is a constructed topical distractor negative control, not implemented lexical retrieval.

[results-initial.json](results-initial.json) and [results-initial.jsonl](results-initial.jsonl) contain exact state, questions and full successful provider responses including costs, tokens, model and timing. [source-snapshots.json](source-snapshots.json) records pinned source commits and inspected licenses.

From the repository root, with the existing environment key:

```sh
/tmp/knowledge-garden-jev-venv/bin/python docs/research/initial-brainstorm/02-knowledge-maintenance/experiments/formation-repos/build_cases.py
/tmp/knowledge-garden-jev-venv/bin/python docs/research/initial-brainstorm/02-knowledge-maintenance/experiments/formation-repos/run.py --output docs/research/initial-brainstorm/02-knowledge-maintenance/experiments/formation-repos/results-new-run.json
```

The runner refuses to overwrite result/journal files. Reruns incur live charges. Install `typesafe-sdk==0.7.2` in a Python environment if the temporary virtual environment is unavailable. The SDK supplies `httpx2`, used only to capture successful raw responses; credentials are supplied as SDK options and are never serialized into result records.
