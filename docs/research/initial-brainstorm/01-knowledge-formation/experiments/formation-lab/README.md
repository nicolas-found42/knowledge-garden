# Formation lab

[Knowledge formation](../../README.md) · [Initial brainstorm](../../../README.md) · [All research](../../../../README.md)

This suite studies how conversations and source passages become supported claims, entities and candidate topics.

Synthetic composition experiments using TypeSafe SDK 0.7.2 and OpenRouter Jev 1.13. See [findings](../../2026-09-29-formation-experiments.md).

The main run freezes 96 fixtures, including 24 holdout fixtures, before API evaluation. Two variants yield 192 requests. Full states/questions/raw replies and billing usage are saved. Inputs are public-style synthetic material; the environment key is never serialized. The separate 16-request diagnostic was prompted by observed results and is development evidence.

```sh
python -m venv /tmp/formation-lab-venv
/tmp/formation-lab-venv/bin/pip install -r docs/research/initial-brainstorm/01-knowledge-formation/experiments/formation-lab/requirements.txt
# Set OPENROUTER_API_KEY securely in the process environment.
/tmp/formation-lab-venv/bin/python docs/research/initial-brainstorm/01-knowledge-formation/experiments/formation-lab/run.py --output /tmp/formation-new-results.json
# Offline audit reads the committed results.json; no credentials or calls needed.
/tmp/formation-lab-venv/bin/python docs/research/initial-brainstorm/01-knowledge-formation/experiments/formation-lab/analyze.py
```

Run refuses to overwrite an existing result file. It writes fixtures.json from the immutable fixture definitions; archive results before changing definitions. Diagnostic also refuses to overwrite its stored result, and is not needed to reproduce the main run. The existing SDK environment used in this study is `/tmp/knowledge-garden-jev-venv`.

`analysis.json` includes disagreements, threshold tradeoffs, minimal lexical controls, offline view changes and deterministic invariants. `summary.json` distinguishes fixtures, calls, questions and labeled checks. None of these scripts mutates a knowledge corpus or constitutes a production importer.
