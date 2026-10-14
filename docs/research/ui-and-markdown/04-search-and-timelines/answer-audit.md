# Researcher audit of six cited answers

Research date: 2026-09-29 (America/New_York); audit timestamp is recorded in [answer-audit.json](experiments/answer-audit.json). This reviews the six `a1`–`a6` answers in [run-20260930T004745Z](experiments/run-20260930T004745Z/), its frozen labels, the [corpus](experiments/corpus.json), structural checks and already-recorded Jev verification. No new API calls were made.

The core questions receive appropriate bounded answers or abstentions, but **one answer adds false photograph provenance despite an exact quote**, and four sentence quotes fail literal-source checking. Jev marks all thirteen sentences supported; the contextual audit disagrees with two of those judgments. Constructed-origin disclosure is a separate weakness: only the unacquired-link answer explicitly retains it.

## Rubric and limits

This is a second researcher reviewing another researcher's authored development fixtures after generation. It is not blind review, an independent holdout, user labeling or human usability evidence. I checked the supplied corpus text and full source context, not the actual photograph pixels, PDF pages or video in this audit. The JSON stores input hashes, original prose/quotes, recorded checks, Jev probabilities and sentence-level findings so the review can be revisited.

Four axes remain separate:

- **Structural:** every citation ID exists; a quote must be an exact, case-sensitive contiguous substring in one cited source. A stitched quotation fails even if its component passages are faithful.
- **Semantic context:** `supported` means the supplied context warrants the prose at its stated scope; `qualified` means a main idea is related but a limit was lost or broadened; `unsupported` means a substantive assertion is unwarranted or contradicted. Exact quotes do not establish this axis.
- **Provenance:** preserve controlled/constructed/public-reuse origins as explicitly requested by the generator instruction. Omission is recorded separately; turning fictional metadata into the real public file's history is a semantic failure.
- **Completeness:** address the question and its uncertainty, then check whether additional prose undermines the correct answer. Frozen substring labels alone cannot establish completeness.

| Check | Observed count | Meaning and limit |
| --- | --- | --- |
| Valid citation IDs | 13/13 sentences | Each ID exists; this is not evidence that its source supports the claim |
| Exact quote in one cited body | 9/13 sentences | Four quotations are concatenated or altered |
| Context supported | 11/13 sentences | Researcher assessment conditional on supplied source text; provenance omissions remain separately recorded |
| Context qualified | 1/13 sentences | a3 sentence 3 broadens “does not establish” into “no information” |
| Context unsupported | 1/13 sentences | a3 sentence 2 promotes controlled metadata into true photograph date |
| Jev `supported` decisions | 13/13 sentences | Two disagreements with this audit; probabilities do not prove support |
| Core question addressed | 6/6 answers | a3 nevertheless becomes misleading through its extra prose |
| No substantive contextual failure | 5/6 answers | Quote/origin issues can still require repair |
| Constructed origin explicitly retained | 1/6 answers | a4; the other answers omit source origin despite the explicit instruction |

Do not pool these axes into a single accuracy percentage or extrapolate from these thirteen short, correlated development sentences.

## Findings for every generated sentence

Sentence numbers below are one-based; JSON indexes are zero-based. The JSON preserves the complete generated text and exact source quotes.

| Answer/sentence | Context verdict | Quote verdict | Finding |
| --- | --- | --- | --- |
| a1/1 — 1.2 visits/minute on 17 May | Supported | Non-exact | Arithmetic and date are supported across the cited table/homework/claim. One quote concatenates homework and claim-rate passages into a nonexistent single passage. Controlled origin is omitted. |
| a1/2 — limited observation, no all-day estimate or rain causality | Supported | Exact | Relevant limits survive. The exact two-sentence quote comes from homework. Controlled origin is omitted. |
| a2/1 — rain disruption leads to morning-and-noon plan | Supported | Non-exact | plan-2 explicitly gives the disruption and replacement; reasoning supplies adoption. The quote splices two sources together. Controlled history is omitted. |
| a2/2 — no better accuracy established | Supported | Non-exact | Both sources explicitly state no measurement/comparison. The quotation is stitched and changes the capitalization of `better`. Controlled origin is omitted. |
| a3/1 — exact photograph day unknown | Supported | Non-exact | Unknown is appropriate. The quote adds `The` and changes `Photographer` to lower case. The controlled caption must not be treated as independent actual provenance. |
| a3/2 — photograph was taken in May 2024 | **Unsupported** | **Exact** | Caption explicitly says “not its true provenance.” It describes fictional fruit preparation, not actual capture date. Valid ID and literal quote do not warrant the sentence. |
| a3/3 — photograph provides no date information | **Qualified** | **Exact** | Source says inspected pixels do not establish a date. The sentence generalizes to the photograph itself and all information, without establishing absence of metadata or visual clues. |
| a4/1 — linked website content unknown | Supported | Exact | Controlled conversation and never-fetched destination remain explicit. No destination content is invented. This is the one answer preserving origin and acquisition limits together. |
| a5/1 — 12 then 8 visits, no rain attribution | Supported | Exact | Table supports dates, counts and withheld causality. Controlled rows are quoted but their origin is absent from prose. |
| a5/2 — activity versus missing observation unresolved | Supported | Exact | Question source explicitly leaves that causal issue unresolved. If asserting the lower count as a standalone fact, cite the table as well; the cited question alone does not establish the count difference. Controlled origin is omitted. |
| a5/3 — rain claim speculative | Supported | Exact | Keeps the speculative status and lack of support; does not turn it into an accepted explanation. Controlled origin is omitted. |
| a5/4 — one observation and interrupted wet visit limit evidence | Supported | Exact | Faithful report summary and reasonable statement of its limits. Does not establish a trend. Controlled origin is omitted. |
| a6/1 — point-in-time changed-pixel count, not interval/accuracy | Supported | Exact | Directly preserves both key measurement distinctions. Additional code citation is unnecessary for this question; reused executed-demonstration origin is omitted. |

There are no fabricated citation IDs. The quotation failures are specific: a1/1 and a2/1 combine multiple source passages; a2/2 combines passages and changes capitalization; a3/1 changes the source wording. The recorded structural checks identify these same four failures. A quotation UI should store each exact excerpt with its own source ID and span, rather than permitting a single `quote` field to impersonate a combined passage.

## The photograph error and verifier disagreement

The caption states that it is **controlled metadata for a public photograph, not true provenance**. The answer nevertheless says:

> The photograph was taken on some day in May 2024, but the precise date is not specified.

Its exact source quote is about Maya preparing fruit in the controlled caption. That source does not establish when the real OpenCV photograph was taken. Jev assigns this sentence `supported` with probability 0.92, despite receiving the provenance qualification in the full supplied context. The correct first sentence (“exact day unknown”) does not neutralize the unsupported month assertion that follows.

The next sentence similarly broadens an inability to establish a date from pixels into absence of any information in the photograph. Jev assigns it `supported` at 0.77; this audit marks it qualified. The repair is to say that the inspected pixels do not establish a capture date in this evidence, without making claims about all metadata or possible clues.

A faithful replacement for the whole answer would say: “The actual capture date of the public photograph is unknown in the supplied evidence. Its accompanying controlled caption describes fruit preparation in May 2024, but explicitly is not the photograph's true provenance.” This is a researcher-proposed repair, not a rerun or new measured generation.

## Completeness and next controls

a1 answers the rate and bounds its inference; a2 gives the reason for revision and rejects an unmeasured accuracy claim; a4 correctly abstains on unacquired content; a5 leaves rain causality unresolved; a6 distinguishes a sampled frame count from interval aggregation and accuracy. a3 addresses the exact-day question but undermines the answer with invented real-world month provenance.

Five answers omit constructed origin even when their domain-level conclusions are faithful. They should be presented as controlled demonstrations or carry an explicit origin label inherited from source records. This audit does not claim that every sentence must repeat that label; an inspectable answer-level label can preserve context. The generated answer payloads here did not supply such a label. Adding origin display is a proposal, not a demonstrated repair.

The observed failures support separate checks for literal quotation, semantic source scope, constructed origin and answer-level contradictions. They do not establish a universal verifier threshold or justify approving a whole answer because every citation resolves. Preserve these failed outputs and record any future corrected run separately.
