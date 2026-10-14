# Synthetic retrieval laboratory

[Retrieval and navigation](../../README.md) · [Initial brainstorm](../../../README.md) · [All research](../../../../README.md)

This suite studies candidate recall, complementary evidence, bounded graph navigation and stopping. Read the [experiment report](../../2026-09-29-retrieval-experiments.md) alongside its records.

Eight bounded-judgment hypotheses for a personal Markdown knowledge garden. These are development experiments, not product code or an accuracy benchmark. All sources and user situations in the corpus are invented. No private user history was sent.

From the repository root, with Python 3.11+:

```sh
python -m venv /tmp/kg-retrieval-lab-venv
/tmp/kg-retrieval-lab-venv/bin/pip install -r docs/research/initial-brainstorm/03-retrieval-and-navigation/experiments/retrieval-lab/requirements.txt
# Provide OPENROUTER_API_KEY through your environment; never put it in a file.
/tmp/kg-retrieval-lab-venv/bin/python docs/research/initial-brainstorm/03-retrieval-and-navigation/experiments/retrieval-lab/run.py --output /tmp/retrieval-new-initial.json
```

`run.py` refuses to overwrite results. `followup.py` was written after the initial run and expects its saved records at the packaged paths; it refuses to overwrite `results-followup.json`. To reproduce the complete live sequence without modifying checked-in records, copy this directory to a temporary directory, remove only that copy's `results-initial.json` and `results-followup.json`, run `run.py --output <copy>/results-initial.json`, then `followup.py` and `facets.py` in the copy. Run `analyze.py` to regenerate `summary.json` offline without API calls. It imports the fixture definitions and never executes `run.main()`.

The request runner explicitly selects OpenRouter, the environment key, and `typesafe/jev-1.13`. It captures successful response JSON before SDK serialization can discard reported billing cost. Saved requests include state, model and exact questions; responses include probabilities, resolved model, usage and reported cost. Errors retain only exception class, not potentially sensitive exception text. Six concurrent requests; graph hops are sequential where earlier observations determine the next state.

- `fixtures.json`: source IDs, corpus, query labels and lexical rankings.
- `results-initial.json`: 224 successful requests, including ten graph hops.
- `results-followup.json`: 30 successful development ablation requests.
- `summary.json`: 257 calls / 547 questions, usage, fixture counts, outcomes, raw disagreements and coverage annotations.
- `results-facets.json`: three-hop explicit-facet graph extension; routing still fails, and the all-facet rule prevents declaring success.
- `source-snapshot.json`: publicly fetched implementation/documentation excerpts read as data; no foreign implementation was executed.
- `run.py`, `followup.py`, `facets.py`, `analyze.py`: authored runner, focused follow-ups, offline analysis.

Labels were defined before the relevant calls. Initial prompts and labels remain unchanged. The initial raw answerability checks compare against **corpus** availability, including on a lexical shortlist. The offline summary additionally computes **supplied-state** availability, so an accurate no-answer verdict on omitted evidence is not mistaken for a model failure. The final report uses these corrected metrics. Thresholds of 0.5/0.7 are development rules, not validated production calibration.
