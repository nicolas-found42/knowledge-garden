# Query and context repository probes

[Query and context](../../README.md) · [Initial brainstorm](../../../README.md) · [All research](../../../../README.md)

This suite studies query operations, source freshness, raw evidence recovery and context retention. Its historical directory name is retained.

Four small synthetic development experiments inspired by inspected source: current-source validity; correctness-aware context retention; typed query operator and argument binding; synopsis-to-raw evidence acquisition. See the [report](../../2026-09-29-repository-deep-dive.md) for pinned citations, failures and evidence limits.

`run.py` freezes 80 cases before submitting requests. Each case batches its independent questions against one state. `results-initial/fixtures.json` preserves the original data and expected labels; `raw.jsonl` preserves full questions, outputs, probabilities, model, usage and latency. Five service errors are retained as error types. `summary.json` reports hidden-truth agreement as raw checks, so **do not interpret its correctness fields as model accuracy when evidence was withheld**. Read the report and audited metrics first.

`followups.py` retries only the five failed original cases, adds ten developmental branch-specific argument-binding probes, and actually makes a dependent raw-source request after each selected fetch. The plan `executed_plan` field is a dictionary-based bound-plan simulation; it does not implement a graph traversal. All ten synopsis actions fetch, including one unnecessary fetch. The acquisition policy thus shows recovery, not selective-fetch savings.

`source-manifest.json` records 10 repository SHAs, inspected file hashes, line counts and immutable URLs. Source/test reading is static; upstream programs and tests were not executed.

## Reproduce

Use Python 3.11+, `typesafe-sdk==0.7.2`, and the existing `OPENROUTER_API_KEY` environment variable. Keep credentials outside artifacts. The SDK key is passed explicitly; base URL is `https://openrouter.ai/api` and model is `typesafe/jev-1.13`.

From repository root:

```sh
/tmp/knowledge-garden-jev-venv/bin/python docs/research/initial-brainstorm/04-query-and-context/experiments/retrieval-repos/run.py --out /tmp/retrieval-probe-new
/tmp/knowledge-garden-jev-venv/bin/python docs/research/initial-brainstorm/04-query-and-context/experiments/retrieval-repos/followups.py --initial /tmp/retrieval-probe-new --out /tmp/retrieval-followup-new
```

Choose new output directories; scripts refuse existing output. Follow-ups retry the failures present in the selected initial run, so request count can differ on replay. Model/provider behavior can change. The initial/follow-up records in this folder are historical observations, not regenerated expectations or held-out evaluation. A failed follow-up may terminate its script; preserve any original records and use a fresh output for a later replay.

105 explicit SDK requests produced 100 successful responses. SDK internal HTTP retries are not counted separately. Observed successful-response cost was $0.002213148; failed/unobserved attempt costs are unknown. Fixtures are public-style synthetic strings containing no private histories. The only threshold used for these counts is Noul 0.5; there is no calibrated deployment threshold.
