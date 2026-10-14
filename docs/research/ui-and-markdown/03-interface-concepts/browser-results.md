# Actual browser observations and scale measurements

[Concepts and task protocol](README.md) · [Collection](../README.md) · [Raw records](browser-records/)

Observed on macOS 26.4.1 ARM64. The Codex in-app browser reports Chromium 154, a compatibility `MacIntel` platform string, hardware concurrency 10 and pixel ratio 2. Do not interpret that user-agent platform string as the physical CPU. The separate Jev automation runner launches an owned throwaway Chrome profile and closes it. No everyday tabs were driven.

## Interaction evidence

- The [workspace switch](browser-records/workspace-switch.log) performs one selection, then independently observes `variant=workspace&view=read&item=photo-fruit`. The chosen item and view survive that switch.
- An [initial timeline check](browser-records/timeline-initial.log) verifies only entry into timeline mode. After the prototype title was made to reflect its actual state, the [stronger check](browser-records/timeline-exact.log) performs three actions and independently observes `timeline · events · descending`. The weaker record is retained, not retroactively treated as full verification.
- The [keyword check](browser-records/find.log) fills `slides`, submits Search, and independently observes `keyword · 1 results`. This is the UI's simple substring method, separate from the SQL/model benchmark. The source/data inspection establishes the matching presentation; the runner's final independent assertion is the one-result title.
- CUA's live accessibility tree exposes all 27 map nodes and 30 relationship buttons in the canvas, including complete accessible item labels despite abbreviated visible graph text. A Jev-selected “Expandable groups” action changes the visible scope to `GROUPS` and exposes ten kind groups whose counts sum to 27 (16 files; two decisions; two claims; one each of annotation/person/place/concept/event/question/trail). Selected-photo context remains visible. This checks the overview, not every group expansion/return path.
- At 1280×720, screenshots revealed an active Read label rendered white on a pale background and a task selector overflowing the reading rail. CSS was repaired. A fresh screenshot verifies a green active button and a selector constrained to its rail. Canvas uncertainty text was also lightened, but no numerical contrast audit or screen-reader walkthrough is claimed.

The CLI renderer smoke covers 18 layout/mode combinations, search/time/purpose branches, 27 source inspectors and 30 relationship inspectors. Those are synthetic DOM checks, not browser/user task completion. Browser observations above are narrower but actual. No participant time, error rate, cognitive load or interface preference was measured. Browser chooser calls and its typing helper are not included in the provider ledger because their full usage responses are unavailable.

## Scale measurements

The [point record](browser-records/measurements-20260930T005430Z-76a8ae1f0e.json) retains five measured repeats at each size after one warmup. Values below are medians rounded to 0.1 ms:

| Points | Coordinate generation | Grid index build | 100 bounded hit tests | Draw submission |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | <0.1 ms | 0.1 ms | 0.1 ms | 0.1 ms |
| 10,000 | 0.2 ms | 0.7 ms | 0.1 ms | 0.5 ms |
| 50,000 | 1.0 ms | 3.3 ms | 0.2 ms | 2.4 ms |

Zero/near-zero values reflect the clock resolution, not zero work. This point fixture contains no edges, image decoding or per-item accessible DOM. The grid test executes deterministic probes, not real pointer-event latency. A first successful DOM-visible run preceded a download-tool timeout; its transcribed medians and failure remain separately retained. The later full record uses the local capture endpoint.

The [edge/group record](browser-records/measurements-20260930T005439Z-ab0e6115d4.json) retains three measured repeats after a warmup for each configuration. Full rendering submits every generated line in 5,000-edge path batches, plus all nodes. The summary aggregates the same data into twelve index-based groups; it is not semantic clustering. Directed edges are stored but no arrowheads are drawn. Median timings:

| Nodes | Directed edges | Full draw submission | Full next-frame elapsed | Grouped draw submission |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 2,000 | 0.2 ms | 14.6 ms | <0.1 ms |
| 1,000 | 8,000 | 0.3 ms | 16.7 ms | 0.1 ms |
| 10,000 | 20,000 | 1.3 ms | 16.7 ms | 0.1 ms |
| 10,000 | 80,000 | 2.4 ms | 16.6 ms | 0.1 ms |
| 50,000 | 100,000 | 5.1 ms | 16.6 ms | 0.1 ms |
| 50,000 | 400,000 | 9.4 ms | 28.6 ms | 0.1 ms |

At 50k/400k, edge generation is median 1.5 ms and aggregation 0.6 ms. Next-frame elapsed ranges from 18.6 to 59.4 ms; grouped next-frame elapsed also varies (14.8–30.3 ms). Neither requestAnimationFrame nor submission latency proves GPU completion or an exact FPS. Shared browser scheduling, refresh alignment and other workloads can affect these small samples. Exact typed-array byte counts are retained, but do not include Canvas paths, browser/GPU allocations or a real application's memory footprint.

The inspected full 50k/400k image is a dense field of crossing lines in which individual relationships cannot be read at overview scale. That is researcher visual observation, not a measured human-usability result. Grouping reduces draw work, but the modulo groups have no knowledge meaning. This supports comparing aggregate overview plus drill-down while retaining full access; it does not select a library or declare large-collection navigation solved.

## What the current concepts do not establish

The ordinary interactive graph is 27 items/30 edges and directly draws at most 80 visible nodes; its keyboard/list fallback is not integrated with the separate 50k benchmark. Whole-collection individual-item navigation beyond that small interactive fixture, force layouts, semantic grouping, stable expansion, source-media rendering at scale and accessible 50k navigation remain untested. The research prototype must not silently become an architecture that caps the user's collection at 80 items.

The quiet concept still places several controls above prose, and its floating research variation selector can obscure part of a short viewport. The source inspector is a modal rather than an evaluated persistent evidence flow. Graph labels can become crowded; edge buttons have repeated type-only accessible names. These are concrete design questions for the next comparison, not evidence that the user prefers one composition. Persisted trails/views and live Markdown writing are tested separately in the disk concern, not connected to this browser state.

Reproduce with the collection's `serve.py`, then open `scale.html` and `edges-scale.html`. The optional local POST endpoint saves each completed run under a new timestamp/hash. Plain `python -m http.server` can show the result but cannot retain it automatically. Download-tool failure, public acquisition failure and semantic model failure are kept as different categories.
