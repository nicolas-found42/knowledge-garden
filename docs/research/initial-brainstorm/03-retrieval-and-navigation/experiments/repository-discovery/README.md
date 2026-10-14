# Jev-assisted repository discovery

[Retrieval and navigation](../../README.md) · [Initial brainstorm](../../../README.md) · [All research](../../../../README.md)

This repository-discovery experiment supplied leads to all four concerns. The [shared catalog inventory](../../2026-09-29-repository-landscape.md) records the broader discovery coverage.

The previous catalog scan retrieved 1,060 root READMEs. A lexical pass formed 335 public repository cards mentioning memory, Markdown, retrieval, evidence, graphs or related terms. One bounded `jev search` round then ranked the cards to look beyond the original manually chosen shortlist.

This is a discovery experiment, not a labeled benchmark or a source-code review. The complete [input](input.json) and [result](result.json) are retained. Cards are partial views of their READMEs, and file traversal order was not randomized. Relevant code can be missed by lexical preselection, excerpt construction, clipping or judgment.

Command used after writing input.json:

```sh
jev search --max-rounds 1 --top-k 18 --timeout 45 < input.json > result.json
```

The CLI reported `status: ok`, `answer_from_what_we_have`, 306 scored cards and 29 unjudged IDs, with insufficient evidence for a final recommendation. It returned 36 selected IDs despite top_k=18: 18 judged selections plus 18 unjudged fallback entries. Only judged selections were used to direct deeper investigation. This is why the raw result metadata matters. The clipping and local-screen flags are heuristic processing metadata; they do not establish malicious behavior by a repository.

Reported usage: 103,195 input tokens, 12,189 output tokens, $0.004334190. Reported latency: 861 ms; not independently measured end-to-end. The CLI did not return its resolved model ID or individual raw API responses, so this usage is kept separate from the SDK experiment ledgers.

## Judged shortlist, in returned order

- [vbcherepanov/total-agent-memory](https://github.com/vbcherepanov/total-agent-memory) — relevance 0.77
- [libingzheren/Jev-Mem](https://github.com/libingzheren/Jev-Mem) — relevance 0.76
- [bodepudimuneendra-netizen/laya-jev-GraphRAG](https://github.com/bodepudimuneendra-netizen/laya-jev-GraphRAG) — relevance 0.75
- [FreedomIntelligence/Tiermem](https://github.com/FreedomIntelligence/Tiermem) — relevance 0.73
- [kunpengtalk/OmniStudio](https://github.com/kunpengtalk/OmniStudio) — relevance 0.73
- [picaye/jev-compaction](https://github.com/picaye/jev-compaction) — relevance 0.72
- [DeepBlueDynamics/typesafe-arena](https://github.com/DeepBlueDynamics/typesafe-arena) — relevance 0.71
- [zilliztech/memsearch](https://github.com/zilliztech/memsearch) — relevance 0.70
- [hev/reranker](https://github.com/hev/reranker) — relevance 0.70
- [xerj-org/xerj](https://github.com/xerj-org/xerj) — relevance 0.68
- [GhalebDweikat/winnow](https://github.com/GhalebDweikat/winnow) — relevance 0.68
- [MichelKerkmeester/skilled-agent-harness_spec-driven-loops](https://github.com/MichelKerkmeester/skilled-agent-harness_spec-driven-loops) — relevance 0.67
- [EugeneBoondock/jevsql](https://github.com/EugeneBoondock/jevsql) — relevance 0.66
- [compozy/yoshi](https://github.com/compozy/yoshi) — relevance 0.66
- [cpaczek/s1s](https://github.com/cpaczek/s1s) — relevance 0.66
- [Avinash-jetwani/jevmem](https://github.com/Avinash-jetwani/jevmem) — relevance 0.64
- [chunxiaoxx/nautilus-compass](https://github.com/chunxiaoxx/nautilus-compass) — relevance 0.64
- [joelhooks/pi-fast-jev-compaction](https://github.com/joelhooks/pi-fast-jev-compaction) — relevance 0.64

## Implication

This found useful additional leads such as Jev-Mem, TierMem, JevSQL, System One Search and JevMem. Their README claims still require code inspection. A cheap semantic discovery pass can broaden a shortlist, but the omitted and unjudged sets must remain visible; ranking alone is not coverage.
