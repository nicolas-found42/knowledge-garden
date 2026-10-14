# Generated wiki prose compared with source structuring

This experiment treats readable wiki generation as a separate capability. It compares structured source records with source-linked capsules produced from three full-evidence bundles and three deliberately lossy concise-observation variants. All six variants were saved before any calls. These are selected public/controlled excerpts, not private user history or whole-media perception inputs.

The bundles are the actual controlled detector change and timed measurement, Darwin OCR plus public DEAP article/code and an authored note, and three constructed method histories with actual failed/corrected code execution plus exceptions. Every original source record carries an artifact SHA-256 and source selector where available. Lossy variants deliberately remove provenance, important negative qualifiers and/or method exceptions while retaining valid original source IDs. Their omissions are part of the test, not a claim that a real perception model produced them.

| Artifact | Role |
| --- | --- |
| [portable-fixtures.json](portable-fixtures.json) | Frozen six generation inputs and full original source records. Includes scan/OCR source attribution and actual method outputs; no source files modified. |
| [build_fixtures.py](build_fixtures.py) | Reads existing saved artifacts and constructs portable fixtures; no fetches/model calls. Source selections are explicit. |
| [run.py](run.py), [results.json](results.json) | Six GPT-4.1-mini generation requests and six Jev verification requests. Full bodies, replies, elapsed time, exact fixture hash and parsed capsules retained. |
| [structured-presentation.md](structured-presentation.md) | Deterministic exact previews plus inherited source origins, IDs, selectors/hashes and explicit clipping. No generated narrative. |
| [capsules.md](capsules.md) | All generated capsule prose, including flaws. Source IDs/statuses shown; these are experimental outputs, not accepted wiki pages. |
| [analyze.py](analyze.py), [analysis.json](analysis.json) | Offline structural checks, researcher sentence/qualifier audit, full ledger and deterministic stale-prose demonstrations. |

Generation was requested as at most four short sentences, each with valid source IDs and recorded/tentative/speculative status. Instructions explicitly require constructed origin, bounded claims, uncertainty, exceptions and measured-versus-asserted distinctions. Jev then judged each generated sentence against its **original referenced evidence**, never merely against the lossy observation. It separately scored status appropriateness. Researcher inspection evaluates required qualifiers independently of Jev; it is not an external human annotation or readability study.

All six capsules obeyed the four-sentence bound, used valid source IDs and valid status labels. They produced 22 sentences. Those structural successes did not establish epistemic fidelity: Jev's argmax labels were 14 faithful, five qualifier-missing and three unsupported, but the manual audit identifies both missed errors and overly strict rejections. These labels are retained judgments, not a pooled accuracy metric.

## Findings by bundle

**Motion, full evidence.** The capsule retains the controlled-demonstration framing, grayscale/blur/threshold35 change, no-intent/object-tracking limits and exact t=2.0-second measurement: **11,566 changed pixels out of 442,368**. It presents “to reduce noise compared to” direct RGB comparison without attributing this rationale to the authored assistant statement or explaining that the supplied generation bundle contains no measured before/after accuracy/noise comparison. The media study separately measured before/after changed-pixel counts, which do not establish accuracy or reduced noise against ground truth. That wording can describe a goal rather than a proven gain; the audit treats it as ambiguous comparative rationale and missing attribution, not conclusive invention of an accuracy result. Jev flagged the sentence unsupported. Source structuring retains the separate assistant statement and real output, allowing a reader to distinguish them directly.

**Motion, lossy observations.** The capsule omits controlled origin, no-intent/tracker limits and comparative-evidence limits removed during conversion. It also changes “at2seconds” into **“over2seconds”**, suggesting interval aggregation where the saved output is a timestamped consecutive-frame difference at5fps. Jev nevertheless calls that sentence faithful (0.58). Correct copied numbers alone do not preserve measurement semantics. This is a generator timing distortion that the verifier misses, alongside conversion-driven qualifier omissions.

**Book, full evidence.** The four-sentence capsule summarizes Darwin and DEAP but never represents the controlled note, its unresolved question or the speculative application bridge. It uses three of the four available source IDs. Longer code-rich input may have drawn attention away from the short note; this is a plausible explanation, not a tested causal diagnosis. No false historical influence is asserted. The capsule also omits the fact that its book passage is derived OCR, with the actual scan/page retained as primary evidence. Jev calls the book sentence faithful despite that missing provenance.

**Book, lossy observations.** The capsule includes the note's diversity question and keeps the benefit uncertain. However, it does not say the note is authored for the experiment, or that book text comes from OCR. The note's generalized “may result” claim is labeled recorded. Jev marks that sentence qualifier-missing, while accepting the uncertainty sentence. The output avoids an invented Darwin/DEAP influence claim; uncertainty retention does not compensate for missing source origin.

**Methods, full evidence.** The capsule preserves bounded corrections, the signed-URL unresolved question and numeric-field/public-immutable-resource exceptions. It omits that all three coding histories are constructed, even though the full evidence explicitly includes that origin and the failed/corrected executions. Jev detects missing qualifiers on the first two sentences. A reader who encounters the capsule alone can mistake controlled demonstrations for observed recurring personal work. The exception sentence is itself substantially faithful; Jev's missing-qualifier answer there may concern global origin rather than an unsupported exception.

**Methods, lossy observations.** The signed-URL question and all exceptions are absent from the input and remain absent from the capsule. Provenance is also missing. The URL sentence distorts collision direction into **“keys … collapsed into distinct views”**, where the actual failure collapsed distinct destinations into one key; Jev correctly rejects that sentence. Conversely, Jev rejects the CSV string-preserving correction as unsupported (0.59), even though the original program/output supports it. The manual audit labels its main defect omitted constructed provenance, making qualifier-missing the better diagnosis. This is a verifier overstatement distinct from the genuine URL wording error.

## Structured records and readable prose serve different tests

The full structured presentation preserves original source IDs, origin, artifact hashes, exact selected text and relevant exceptions without asking a generator to remember them. It is longer and less immediately readable; no user comprehension/time study was run. Lossy structured inputs themselves lack some required evidence, so generation cannot reliably reconstruct it. The capsules are shorter, but brevity creates source selection and qualifier risks even from full inputs. This experiment supports keeping those artifacts distinct and deriving status/provenance from records in code; it does not settle the page format or automatic promotion policy.

Four offline stale-prose examples demonstrate another boundary. Filtering out the note or method exception sources leaves the stored capsule text unchanged, including sentences whose references are now hidden. Removing the note from the full book input has no visible prose effect because that capsule omitted it already. A separate **hypothetical** mutation changes the t2 source count to20,000 while stored prose still says11,566; that value is explicitly a controlled mutation, not a newly measured output. These tests show that source filtering/revision changes do not automatically rewrite stored prose. They do not measure a deployed refresh system or UI usability. No original sources were changed.

## Exact observed usage

| Engine | Successful requests | Input tokens | Output tokens | Reported cost USD | Median elapsed request |
| --- | --- | --- | --- | --- | --- |
| `openai/gpt-4.1-mini` | 6 | 8,168 | 1,338 | $0.005353920 | 2.481895 s |
| `typesafe/jev-1.13-20260917` | 6 | 23,897 | 1,404 | $0.001003674 | 0.261736 s |
| Total | **12** | **32,065** | **2,742** | **$0.006357594** | Distinct request shapes; no pooled latency |

No provider, parsing or control failures occurred. No retry/repeat runs were used. All replies expose reported usage.cost. Jev verification inputs are much larger than the lossy generation inputs because verification receives original cited evidence; this is a real cost boundary. The ledger excludes coding-agent costs and is not account-billing reconciliation. These six capsules are correlated authored bundle variants, not independent histories. Researcher judgments identify useful failure modes but do not establish general accuracy, calibrated thresholds or human readability.

Offline inspection/replay:

```sh
python3 docs/research/expanded-exploration/04-navigation-and-provenance/experiments/wiki-lab/analyze.py
```

Live replay requires `OPENROUTER_API_KEY` and explicit authorization, reads the key only into an Authorization header, and writes no credentials. The current request schema follows the official OpenRouter TypeSafe compatibility contract already checked for this research wave. Copy this directory before replaying generation to preserve the frozen historical records; fixtures, raw outputs and qualifiers must not be silently overwritten for a new evaluation.
