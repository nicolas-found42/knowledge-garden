# Expanded exploration: evidence, connections and readable knowledge

[Collection index](README.md) · [Previous research](../initial-brainstorm/README.md)

Research date: 2026-09-29. The two-round interview was already complete. This wave used its recorded permissions and open questions, rather than restarting architecture selection. It adds real local history inspection, actual public media and code demonstrations, new implementation research, comparisons with deterministic and generative baselines, and a local comparison of five navigation experiences.

## What the new evidence supports

**Preserve source structure before generating a narrative.** This is the strongest research direction, not an accepted storage design. Across history, media and wiki experiments, deleting context often removes exactly the material needed to distinguish a decision, a quotation, an observed outcome and an interpretation. A model can abstain correctly from an impoverished summary while almost all useful conclusions have vanished. A generated page can contain valid source IDs while omitting the origin or qualifications that make its claims understandable.

**Use Jev where semantic judgment adds value, with source-specific checks.** Current documentation still describes [text-only state](https://docs.typesafe.ai/concepts/state); [pre-parsed extraction](https://docs.typesafe.ai/cookbooks/pre_parsed_value_extraction_cookbook) chooses among supplied values rather than creating missing candidates. The experiments support compositions that retain exact IDs, spans, revisions and conversion provenance in code and use semantic judgments for support, relevance, identity, exceptions or candidate selection. Neither a typed interface nor a concentrated probability distribution establishes factual truth.

**Broad candidate acquisition, conversion fidelity and judgment quality are different problems.** A false caption persuaded Jev until independent OCR was supplied. Candidate generation missed known useful links before review even began. One generated fork-parent relation lacked source support, but Jev still rated it 0.82. These findings argue for measuring original-source fidelity and candidate coverage separately from classification agreement. They do not justify a universal confidence threshold.

## Concrete results

| Investigation | New evidence | Result and practical limit |
| --- | --- | --- |
| Real agent histories | Actual Codex, Claude Code, OMP and Hermes stores; 4,288 inventory units, 52 metadata-stratified samples | Jev agreed with 49/52 frozen quality labels; a schema-aware deterministic baseline agreed with 50/52. Tiny sessions are deliberately overrepresented and two labels are ambiguous; no population accuracy claim. |
| Message/tool representation | Twelve controlled threads and twelve paired real-source follow-ups | Controlled summaries retained 5/24 underlying conclusions despite 24/24 evidence-relative answers, all through abstention. Conversation without tools retained 15/24; the original full view retained 22/24. Tool omission is an evidence-loss decision, not simply a page-clutter decision. |
| Conversation links | 5,131 lexical HTTP(S) occurrences from the 52 selected histories, private occurrence adjacency and conservative URL keys | 1,936 parseable identities, 230 exact-hostname buckets; 24 identities and 25 hostnames cross selected threads. All remain merely linked; destination acquisition attempts are zero. Forty-six invalid candidates remain inspectable without being merged. |
| Media interpretation | Real book scans, fruit photograph, TIFF, AVI/MP4, six timed video frames, Python/RST, CSV/XML and rendered output | OCR retains important written qualifications but loses visual content. Vision recovers some content and also creates attribution, negation and localization failures. No region-IoU or precise event-timing benchmark was completed. |
| Typed connections | Thirty frozen authored pairs; deterministic, Jev and generative baselines; separate follow-ups | Initial agreement: Jev 22/25 development and 3/5 holdout; generative 25/25 and 4/5. One holdout label conflicts with its rubric. Post-hoc improvements are not new holdout evidence. |
| Candidate generation | Flattened 60-node corpus and ten-edge output budget | Lexical generation found 4/14 known useful pairs; generative generation found 8/14. Additional proposals lack exhaustive truth labels, so precision is not established. |
| Reusable method | Three constructed coding histories backed by six actual failing/corrected executions; one unresolved exception | A bounded preservation-of-identity method can be nominated with source-linked exceptions. The histories are not evidence that the user has repeatedly used it in real work. |
| Generated wiki | Three source bundles, full and deliberately lossy variants, six generated capsules and original-evidence review | All 22 sentences have valid IDs/status syntax; nevertheless drafts omit provenance, exceptions or a whole personal-note source. A timestamp became an interval and the verifier accepted it. No human readability study. |
| Provenance and navigation | Three original passages with eleven controlled mutations each; portable disk controls; 24-card, 22-edge five-view demonstration | Exact/context matching reveals a recovery/abstention tradeoff. Authored question collections retain required IDs with fewer visible cards, but those oracle sets do not establish automated curation or usability. |

The detailed reports preserve failures and label ambiguities: [histories](02-agent-histories/2026-09-29-agent-history-investigation.md), [URLs](02-agent-histories/2026-09-29-conversation-url-linking.md), [media](01-source-interpretation/2026-09-29-media-interpretation.md), [connections/methods](03-connections-and-methods/2026-09-29-connection-experiments.md), [navigation/provenance](04-navigation-and-provenance/2026-09-29-navigation-and-provenance.md) and [wiki generation](04-navigation-and-provenance/experiments/wiki-lab/README.md).

## Three richer demonstrations

**What changed, and why?** A [controlled four-turn coding conversation](01-source-interpretation/experiments/media-lab/controlled-coding-thread.json) adopts grayscale, blur and threshold 35. The [actual changed file](01-source-interpretation/experiments/media-lab/controlled_demo.py) runs on a pinned public OpenCV sample and produces [a source-time screenshot](01-source-interpretation/experiments/media-lab/controlled-demo-screenshot.png), measurements and [a playable video](01-source-interpretation/experiments/media-lab/controlled-demo.mp4). Both before and after versions were run. Sparser masks are measured; better accuracy or removal of genuine noise is not established. The thread supplies the constructed rationale; the screenshot cannot establish intent by itself.

**What useful connection was never written down?** A [real historical book page](01-source-interpretation/experiments/media-lab/book-pdf-page-84.png), a public DEAP tutorial and selection implementation are compared with an [authored personal note](01-source-interpretation/experiments/media-lab/controlled-personal-note.md) about losing alternatives under changing workloads. Changed environments, speed/strength tradeoffs and mutable-reference hazards motivate testable links. They remain hypotheses. The scan is a surrogate for a photographed page; camera distortion/glare were not tested. The DEAP repository is an established public project, not evidence about the user's own old work or historical inspiration.

**Which method recurs, with what exceptions?** The [method demonstration](03-connections-and-methods/experiments/method-demo.json) joins CSV identifier, URL query-key and tenant/policy cache-scope failures with their corrected programs and outputs. A fourth history retains the signed-URL question. The proposal is to preserve exact identity/scope, reproduce the collision, make a bounded correction and rerun an invariant. Numeric fields and genuinely public immutable resources have different conditions. Method nomination, executable rehearsal and actual skill activation are separate capabilities; no method was installed.

The [local evidence comparison](04-navigation-and-provenance/experiments/navigation-lab/index.html) makes these public/controlled records inspectable through all five requested views. It is a small research artifact, not a production importer. The [structured source presentation](04-navigation-and-provenance/experiments/wiki-lab/structured-presentation.md) and [generated capsules](04-navigation-and-provenance/experiments/wiki-lab/capsules.md) expose the prose tradeoff directly.

## Capability map: the five requested experiences and five additions

| Experience | What it would add | Evidence/status in this wave |
| --- | --- | --- |
| Readable wiki with media, sources and backlinks | Concise explanations grounded in navigable evidence | Local authored pages and separately measured generation; qualifier retention remains weak. |
| Source explorer | Filter original/derived records by project, person, topic, format and time | Implemented for the public/controlled corpus; real-history adapters investigated separately. |
| Reasoning timeline | Proposals, experiments, adoption, reversal and unresolved work in context | Controlled event sequence; actual cross-thread chronology and active-branch resolution remain unfinished adapter research. |
| Connection map/neighborhoods | Inspect typed relationships and their evidence/status | Authored local one-hop map plus live connection/candidate experiments. |
| Focused question collections | Bring complementary media, code and history together | Authored local collections; their reduced visible-card counts are an oracle comparison. |
| **Failure-aware starting checklist** | Before a task, surface earlier failure mechanisms and verified corrections relevant to its conditions | Supported by new failure-triage implementation research and bounded method examples; no live user workflow trial. |
| **Method rehearsal bench** | Turn a reviewed method into a reproducible exercise with a failing variant and exception case | Three actual bounded failure/correction pairs; generalization from natural histories remains untested. |
| **Answer-change inbox** | Separate new evidence with the same answer from changed conclusions and unresolved conflicts | Primary implementation evidence; proposed experience, not an implemented monitor or default interruption. |
| **Counterfactual decision workbench** | Change a recorded assumption and compare observed consequences, predictions and unknowns | Proposed falsifiable experiment; no implemented counterfactual engine or causal validity claim. |
| **Analogy laboratory** | Test a speculative cross-interest bridge, its limits and a disconfirming experiment | Actual media/book and connection judgments; no user usefulness ratings. |

The [additional-capabilities report](03-connections-and-methods/2026-09-29-capabilities-and-implementations.md) specifies required sources/relations, Jev's role and a falsifiable experiment for each addition. These are materially different activities, rather than renamed reading/navigation views.

## New implementation evidence

This wave continued from the existing repository inventory instead of rerunning its catalogs. New targeted inspections cover [Stanley, Jev research pipeline, Skillbox and Discern](03-connections-and-methods/2026-09-29-capabilities-and-implementations.md), plus the [Hypothesis anchoring implementation](04-navigation-and-provenance/2026-09-29-navigation-and-provenance.md). The media study additionally pins and examines the relevant OpenCV and DEAP implementation material. Manifests distinguish captured files, inspected functions and actual executions; acquiring a repository tree is not an audit of all its files.

Useful concrete ideas include separating log-window omissions from failure judgments, distinguishing evidence addition from answer change, keeping source/span/hash and judgment/policy identities separate, checking method-package integrity, and selecting procedures only after deterministic eligibility checks. The source-support substring shortcut experiment also exposed an adaptation hazard: using an isolated helper without the original Claim/Unit preconditions creates quotation/negation failures. That is not a demonstrated end-to-end defect in the original application.

## Source access and record handling

Original personal sessions, setup files and source files were read without modification. Relevant selected, cleaned excerpts were used with the authorized configured API. Raw private histories, titles, exact mappings, API states/replies and URL occurrences remain outside the repository in restricted local storage. The repository receives aggregate records, opaque sample identifiers and controlled examples. This is separate from the eventual product's on-demand ingestion behavior.

Conversation URL occurrences never enter an acquisition callback. A public documentation fetch that returned 403 is `could_not_retrieve`; a URL discovered in a conversation is `merely_linked`. Known URL identity or shared host does not manufacture destination content, establish relevance or imply an endorsement. The capture experiment does not cover relative links, non-HTTP schemes or naked domains, and not every lexical candidate has been manually validated.

The actual source formats exceed a Markdown-only narrative: JSONL and SQLite histories, PDF scans and OCR, images, timed video, code, article markup and structured records all retain their roles. Tested support is bounded. Audio/speech/tone, handheld photography, handwriting, rich office documents, arbitrary archives, long-video continuity and multilingual extraction remain unsupported or untested here. Native-video transport success does not establish correct temporal grounding. The failed OMP parser and generated-region contracts are reasons to make conversion coverage inspectable.

## Usage and cost

The [reconciled ledger](usage.json), rebuilt from each concern's authoritative summary, records **322 successful provider requests**, **549,589 input tokens**, **50,420 output tokens**, and **$0.047275485 reported router cost**—about **4.73 US cents**.

| Role | Requests | Input tokens | Output tokens | Reported router USD |
| --- | ---: | ---: | ---: | ---: |
| Jev | 286 | 430,153 | 18,207 | 0.018066426 |
| Supporting perception and generative models | 36 | 119,436 | 32,213 | 0.029209059 |
| Total | 322 | 549,589 | 50,420 | 0.047275485 |

Jev resolved to `typesafe/jev-1.13-20260917`. Supporting responses identify Gemini 2.5 Flash Lite, GPT-4o-mini and GPT-4.1-mini, without immutable dated builds. Media responses include 16 BYOK calls and **$0.026221 upstream-inference metadata** across BYOK and router-paid requests. It overlaps the router figures and must not be added as if it were a separate complete bill. The total excludes coding-agent work, local extraction/runtime, external BYOK account charges, previous research and browser-chooser usage that the runner does not expose. It is observed response accounting, not total project cost.

Successful transport includes retained malformed/truncated generations. Eight of sixteen initial media generations were invalid/truncated; later repairs are separate records. No provider transport failures occurred in the measured suites. Fixture, adapter, anchoring and browser-control failures are logged separately. The small cost does not establish general quality, latency at scale or a complete multimodal cost model.

## Concrete next experiments and unresolved decisions

1. **Read-only on-demand source dossier.** Let a user select one actual thread as title/intent plus substantial conversation, then inspect selected claims, tool evidence and URL occurrences together. First validate branch/parent chains, Hermes compression/active state and corrected OMP tool calls. Reserve naturally selected whole episodes and independent labels; evaluate whether a short meaningful conversation survives and whether a hypothetical suggestion is ever promoted into an adopted decision.
2. **Original-region recovery for unseen media.** Reserve handheld book photographs, diagrams, audio with tone and longer videos. Independently label regions/events and compare exact extraction, bounded source-ID observations and interpretive proposals. Measure missed facts, false claims, region/time validity and the cost of escalating to original media.
3. **Five-view task comparison.** Ask a reader to recover an exact qualifier, explain a reversal, find an unresolved question and reject a misleading link using the same evidence in each view. Measure source-recovery success, additional items inspected and time. Evaluate speculative-link visibility and focused collections with actual preference data, rather than adopting the laboratory default.
4. **Method rehearsal and separate wiki drafts.** Nominate a method from natural successful/failed histories, reproduce its outcomes and retain exceptions. Generate a separate page with host-owned origin/status inheritance and source-specific checks; test source revision and filter changes explicitly. Valid IDs alone are insufficient.

Storage/index technology, source retention, message granularity, automatic knowledge promotion, speculative display defaults, proactive interruptions, confidence thresholds and the default home experience remain open. No ADR records an accepted product architecture from this research.
