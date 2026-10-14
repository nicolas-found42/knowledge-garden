# Source update experiment for specification question 18

Status: research evidence for an unresolved product decision. This is not an accepted specification or architecture. The user explicitly requested live Jev experiments and three concrete examples. All inputs are synthetic; no private research material or conversation-link destinations were accessed.

Question 17 was answered separately: use the newest source version, not the latest import time. Question 18 asks when absent text should remove existing knowledge.

## Proposed behavior and the three examples

The original report describes Maya's observation at Riverside on May 17, 2024: 12 visits over 10 minutes. No other source supports its facts in these fixtures.

| Incoming source | Proposed current wiki result | Actual Jev field selections |
| --- | --- | --- |
| A newer complete replacement for the same report says 15 visits and omits duration. | Change 12 to 15; remove duration; retain observer, location, and observation date. Refresh supporting references where the new report supports them. | Expected result in all three repeats. |
| A newer note explicitly corrects only the count to 15. | Change 12 to 15; retain the other facts and their support. | Expected result in all three repeats. |
| A report explicitly describes a separate May 18 observation with 8 visits. | Retain the May 17 observation and add the May 18 observation. Each observation has one count. | Expected result in all three repeats. |

These rules were supplied to Jev as candidate policy. Agreement demonstrates application of that supplied policy; it does not establish that the user has accepted it.

Actual deterministic renderings of the selected operations:

- [Replacement diff](run-20260930T042342.450903Z/offline-audit/replacement.diff)
- [Correction diff](run-20260930T042342.450903Z/offline-audit/correction.diff)
- [Separate observation diff](run-20260930T042342.450903Z/offline-audit/separate_event.diff)

## Method

Expected results and inputs were frozen before model calls. Independent questions about source relationship and individual fields were batched. Code copied the selected candidate values and evidence references into a proposed record. A subsequent Jev request checked that composed record. Three intentionally incorrect records tested whether that check could catch mistakes.

Additional authored cases covered an older revision imported later, a selective summary, and incomplete extraction. For the incomplete case, code exposed a frozen second page containing the missing duration and made a dependent classification and verification request. This was one bounded recovery step, not actual PDF retrieval or a demonstrated general retrieval loop.

The input contained explicit source labels, complete source text for complete cases, and pre-extracted candidate fields. Source timestamps were compared in code. The run did not extract dates, discover candidate fields from real files, or resolve difficult source identities.

## Observed results

- 29 successful live requests to `typesafe/jev-1.13-20260917` through OpenRouter.
- Nine main trials: three examples, three repeats each. With boundary cases and recovery, there were 13 classification evaluations.
- All 13 source-relationship labels, 65 individual field operations, and 13 composed proposals matched the frozen expectations.
- The broad verification question rejected two of three intentionally incorrect records. It **accepted an incorrect record that deleted Maya from a count-only correction**.
- The same verifier also rejected the expected selective-summary update.
- At the frozen diagnostic cutoff of 0.5, the broad question asking whether more content was needed falsely flagged **all 12 complete evaluations**. It correctly flagged the one incomplete evaluation. After recovery, the complete case still returned a false flag of 0.71.
- Reported router usage: 70,057 input tokens, 6,621 output tokens, USD 0.002942394. This excludes agent work and is not an estimate of full application cost.

The supported finding is narrow: field-by-field selection worked on these explicit authored cases under the proposed policy. Broad semantic checks were not reliable enough to establish safe autonomous updates. Repeated small fixtures do not provide a general accuracy estimate.

## Remaining validation gaps

This experiment does not establish real document extraction, source-version discovery, semantic alignment of ambiguous revisions, or the authority of one independent source over another. The summary fixture expressly grants the newer summary authority to update the matching count; chronology alone does not establish that authority.

The record builder starts from one source's supplied candidates. It does not exercise a real wiki page with multiple supporting sources, stable persistent record identities, competing source versions, or concurrent external edits. Its Markdown renderings are audit artifacts, not application exports or tested Obsidian edits.

Completeness metadata, allowable field operations, and preservation of unrelated records need deterministic checks. Jev can help identify source relationships and proposed changes, but a second broad model judgment is not an integrity guarantee. This observation does not introduce a human review requirement; the confirmed product requirement remains automatic operation.

## Reproduction and evidence

- [Experiment runner](experiment.py): standard library only; freezing and live execution are separate operations. `--live` makes paid model requests using an existing `OPENROUTER_API_KEY` without persisting the key.
- [Frozen inputs and expectations](frozen-inputs.json)
- [Live run summary](run-20260930T042342.450903Z/summary.json)
- [Offline audit](run-20260930T042342.450903Z/offline-audit/analysis.json)
- [Offline analysis script](analyze.py): makes no model calls.

The run directory retains individual request, raw response, combined result, and evaluation files. Frozen-input SHA-256: `d31a4073f335293836850e8d03f50f7cef0feaa288d5802ba8dc9d09f3833e5e`. Summary SHA-256: `d7603dc40d9b08c4aacf5897ce57d2d706bd24f221c6cd00c4e8be20dfbbd8af`.

The method uses TypeSafe's documented [Choice primitive](https://docs.typesafe.ai/primitives/choice) and [selection from pre-parsed candidates](https://docs.typesafe.ai/cookbooks/pre_parsed_value_extraction_cookbook.md), through the documented [OpenRouter TypeSafe endpoint](https://openrouter.ai/docs/guides/community/typesafe-sdk). The fixtures cannot demonstrate choices whose candidates were never supplied.
