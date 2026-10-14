# Connection and method experiment records

[Concern](../README.md) · [Experiment report](../2026-09-29-connection-experiments.md)

All source/conversation-like fixtures here are constructed controls or captured public implementation files. No private conversations, private URL destinations or secret-bearing configuration are stored. The configured OpenRouter key is read from the environment and never written into records. Placeholder `example.invalid` URLs are not fetched.

| Artifact | Purpose and provenance |
| --- | --- |
| [sources/manifest.json](sources/manifest.json), [sources/trees.json](sources/trees.json) | Four new public repo commits, captured files/digests and original tree inventory. Scope is named sections, not whole-repo audit. |
| [sources](sources/) | Commit-pinned code snapshots and current TypeSafe official documentation, retrieved 2026-09-29. |
| [build_cases.py](build_cases.py), [cases.json](cases.json) | 30 authored pair fixtures; 25 development/5 holdout written before evaluation. Unique IDs can be used by an evidence browser. |
| [coding-demo](coding-demo/), [method-demo.json](method-demo.json) | Actual failed/corrected Python source and raw output underpin three explicitly constructed histories; fourth unresolved question. |
| [run.py](run.py), [results.json](results.json) | 34 initial API evaluations, exact request/reply bodies and elapsed times. Jev first-pass pairs and method selection; identical-pair GPT baseline plus draft. |
| [baseline-model.json](baseline-model.json) | Live OpenRouter Models API metadata verifying baseline availability before its first request. |
| [analysis.json](analysis.json), [analyze.py](analyze.py) | Original frozen-label comparisons plus conservative deterministic checks. |
| [diagnostic.py](diagnostic.py), [diagnostic-results.json](diagnostic-results.json) | 13 evaluations: six new citation-context cases, six post-hoc identity diagnostic calls, one GPT citation batch. Extracted source helper execution intentionally bypasses caller preconditions. |
| [candidates.py](candidates.py), [candidate-results.json](candidate-results.json) | 60-node flat corpus, lexical/generative ten-edge budget, one generation plus ten Jev support/utility reviews. Known-pair recall is a lower bound, not exhaustive edge truth. |
| [control-failures.json](control-failures.json) | Retained AST code failure before any API call and its repair. |
| [audit.py](audit.py), [audit.json](audit.json) | Complete 58-request ledger, label ambiguity, generated false specificity and status/provenance failures. |

Replay deterministic case/code generation with Python 3.11 or newer (used system Python here):

```sh
python3 docs/research/expanded-exploration/03-connections-and-methods/experiments/build_cases.py
python3 docs/research/expanded-exploration/03-connections-and-methods/experiments/analyze.py
python3 docs/research/expanded-exploration/03-connections-and-methods/experiments/audit.py
```

`build_cases.py` executes only the six small fixture programs under coding-demo and regenerates their outputs. Analyze/audit are offline. The live runners use Python standard-library HTTP, following the current [OpenRouter TypeSafe compatibility contract](https://openrouter.ai/docs/guides/community/typesafe-sdk), and require `OPENROUTER_API_KEY`. They were executed with explicit user authorization in this research handoff. They do not use or log SDK default credentials. No model SDK is required. Research replay that should preserve the original outputs should copy the directory to a new location before running live scripts: the current scripts deliberately write their named result files and are not archival append-only controllers. Do not rerun to inflate sample counts.

The raw requests include field-specific question definitions and exact controlled source text. Display source_status from fixture records alongside any generated status; a generator's “recorded” label means an asserted relation within an authored fixture, not a verified real user event. The method draft's source IDs resolve in method-demo.json; each tool result points into coding-demo. Pure function source extraction is recorded in diagnostic.py and preserves the pinned snapshot.
