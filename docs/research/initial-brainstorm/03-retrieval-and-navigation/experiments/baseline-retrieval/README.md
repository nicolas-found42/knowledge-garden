# Baseline retrieval probes

[Retrieval and navigation](../../README.md) · [Initial brainstorm](../../../README.md) · [All research](../../../../README.md)

These historical probes belong to candidate selection and answerability; the original mixed run remains in knowledge formation.

This folder contains the two retrieval records from the first research pass: selecting a paraphrased rationale over keyword distractors, and recognizing that no passage answers a question.

[results.json](results.json) preserves each request, response, usage, labels and timing. This is an extracted view of the historical [full refined run](../../../01-knowledge-formation/experiments/baseline-jev-wiki/results-refined.json), which remains intact for auditability. It is not an additional experiment run. The original [runner](../../../01-knowledge-formation/experiments/baseline-jev-wiki/refine.py) reproduces these probes as part of its mixed baseline. New retrieval experiments live in sibling directories here.
