# Jev feasibility probes for a personal Markdown wiki

[Knowledge formation](../../README.md) · [Initial brainstorm](../../../README.md) · [All research](../../../../README.md)

The original mixed baseline is retained as one historical suite; its two retrieval records are also available as a linked extract in the retrieval and navigation concern.

These are synthetic, hand-labeled development experiments, run on 2026-09-29 with the existing OpenRouter credential. No private coding-agent chat history was submitted. This directory is research equipment, not a production importer.

Create an isolated environment and install `requirements.txt`. Provide `OPENROUTER_API_KEY` through your existing secret configuration; never put its value in a file here.

```sh
python3 -m venv /tmp/knowledge-garden-jev-venv
/tmp/knowledge-garden-jev-venv/bin/pip install -r docs/research/initial-brainstorm/01-knowledge-formation/experiments/baseline-jev-wiki/requirements.txt
/tmp/knowledge-garden-jev-venv/bin/python docs/research/initial-brainstorm/01-knowledge-formation/experiments/baseline-jev-wiki/run.py --output /tmp/wiki-initial-new.json
/tmp/knowledge-garden-jev-venv/bin/python docs/research/initial-brainstorm/01-knowledge-formation/experiments/baseline-jev-wiki/refine.py --output /tmp/wiki-refined-new.json
/tmp/knowledge-garden-jev-venv/bin/python docs/research/initial-brainstorm/01-knowledge-formation/experiments/baseline-jev-wiki/structure.py --output /tmp/wiki-structure-new.json
```

Every execution makes paid live calls. The scripts refuse to overwrite existing result files. Requests use `typesafe/jev-1.13` and `https://openrouter.ai/api`; all recorded responses resolved to `typesafe/jev-1.13-20260917`. The runner batches independent questions about a case into one request and processes four cases concurrently.

The committed JSON records contain complete state, questions, expected labels, actual answers, distributions, timings, model IDs, and token usage. Their checks score Choice labels only; companion Noul/Score values are diagnostic and do not count as independently validated labels. `refine.py` imports the initial fixtures and changes question wording; its six newly authored cases are development checks, not a blinded holdout.

`results-initial.json` predates the raw-response capture hook. SDK serialization retained token counts but omitted OpenRouter cost and provider metadata. Its `reported_cost_usd: 0` with `cost_available: false` means **unknown, not free**. Later runs capture response JSON with the SDK HTTP client's response hook; the runner now emits null for unavailable cost. The initial record remains unchanged as evidence.

Initial exact-label disagreements involving quoted instructions, scope rejection, and unsupported versus contradicted evidence include debatable label boundaries. The isolated affirmative accepted without an antecedent is the consequential unambiguous failure. Structure recovery also misclassified two blocks with numeric array references; the separate structure probe retained identical content and labels while changing how questions pointed at each block.

Read [the research and experiment report](../../2026-09-29-baseline-research.md) for interpretation and limitations. A perfect score on a handful of synthetic cases does not establish calibration, production accuracy, robustness against malicious content, or safe automatic editing.
