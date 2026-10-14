# Public media interpretation laboratory

[Report](../../2026-09-29-media-interpretation.md) · [Concern](../../README.md)

Public sources plus explicitly controlled histories. **25 supporting-model calls; 113 Jev calls.** No private histories, credentials, or conversation URL destinations. Supporting raw records omit duplicated base64 bytes but preserve exact prompts, media references/hashes, responses and usage.

## Inspect without inference

- `sources.json`: exact URLs, hashes, bytes, captured/could-not-retrieve status. Public original AVI/PDF live in `/tmp/knowledge-garden-media-lab/`.
- `derivatives.json`: source parent and code-owned PDF page/printed page/time anchors.
- `book-pdf-page-*-extract.json`: embedded OCR blocks with page point rectangles. `*-ocr.json`: macOS Vision line text and normalized bottom-left rectangles.
- `video-probe.json`: actual original video metadata; `video-contact-sheet.jpg`: six sampled frames.
- `controlled-coding-thread.json`, `controlled-personal-note.md`: authored fixtures, not recovered histories.
- `controlled_demo.before.py`, `controlled_demo.py`, corresponding `.metrics.json`: actual two implementations run over identical frames. `controlled-demo.mp4`, `controlled-demo-screenshot.png`: actual rendered result.
- `frozen-claims.json`, `generated-audit-labels.json`, `diagnostic-judgment-labels.json`: labels, exact supplied state and questions. Some later semantic audit labels are explicitly ambiguous in the report.
- `perception-records/`, `judgment-records/`: complete original responses, including truncated JSON and wrong attribution/negation. Separate bounded repairs never overwrite original failures.
- `summary.json`, `anchor-audit.json`: offline costs, conversion agreement and structural anchor checks; these do not certify region grounding or exact timing.
- `discovery.json`, `discovery-*.txt`, author/code/metadata snapshots: source discovery, docs and failed acquisition trail. Successful acquisition alone is not evaluated support.

## Replay

Use a Python virtual environment with `Pillow`, `PyMuPDF`; `requirements-observed.txt` records the entire inherited research environment. Jev uses the documented HTTP shape directly; the scripts require no SDK installation. FFmpeg and Swift/macOS Vision are also required for full extraction replay. The native OCR stage is macOS-specific.

```sh
python acquire.py
python prepare.py
swift ocr.swift phototest.tif > /tmp/phototest-new-ocr.json
python controlled_demo.py
python analyze.py
python audit_anchors.py
```

`acquire.py` verifies pinned source hashes and refuses to silently replace changed sources. Run from this directory. Original source acquisition failures are not erased by later successful fallback. `prepare.py` re-renders the book pages and samples the video. For native OCR of the other image derivatives, invoke `ocr.swift` separately for each image and keep new outputs outside this historical run.

Live calls were authorized for the research wave. Set `OPENROUTER_API_KEY` securely in the process environment; never put it in source or record files. For a new run, copy this experiment directory to a new output directory or archive existing record folders first. The runners skip existing IDs, preserving this recorded run.

```sh
python run_perception.py
python run_judgments.py
python repair_perception.py
python diagnose_perception.py
python audit_generated.py
python diagnose_judgments.py
python analyze.py
python audit_anchors.py
```

Dependency order matters: initial perception precedes judgments; bounded repairs precede generated-source audit; alternative perception precedes diagnostic judgments. These experiments did not fetch conversation URLs. Only independently selected public source URLs in the pinned manifest are acquired.

## Measurement boundaries

Initial media labels: 23 claims × three representations, plus seven embedded-book-OCR comparisons = 76. Ten cross-source candidates + two contamination cases = 88 initial Jev requests. Seven generated-source audits + fifteen alternative-model judgments + three bounded-repair judgments = 113. Supporting requests: 16 initial + four repairs + four alternative-model + one timed video repair = 25.

Original media truth is manually inspected. When conversion loses information and Jev returns unknown, source-truth disagreement is not automatically a semantic model error. Existing raw initial outputs contain eight truncated invalid JSON responses; repair outputs are separate. Model-proposed regions/times are not trusted merely because they parse or fit numeric bounds. Cost=0 with BYOK is not zero upstream inference cost; see the separate ledger in the report.
