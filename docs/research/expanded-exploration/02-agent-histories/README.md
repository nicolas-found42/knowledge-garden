# Agent histories and conversation links

[Expanded exploration](../README.md) · [All research](../../README.md)

Research date: 2026-09-29. Four actual local history formats, 52 metadata-stratified samples, 12 controlled threads, and 114 successful live Jev requests were investigated. This is research evidence, not an accepted ingestion or storage architecture.

The main result is that **usefulness filtering can begin with schema-aware code, while retained source context is indispensable for interpreting decisions and recovering tool evidence**. Jev matched 49/52 frozen real-source labels; the deterministic baseline matched 50/52. The sample deliberately overrepresents tiny sessions and does not establish population accuracy. In controlled comparisons, an extractive summary answered every evidence-relative check correctly by abstaining, while retaining only 5/24 underlying source conclusions.

- [Private history investigation](2026-09-29-agent-history-investigation.md): availability, adapters, quality labels, representation loss, uncertainty and the exact inference ledger.
- [Conversation URL experiment](2026-09-29-conversation-url-linking.md): 5,131 candidate occurrences, conservative identities, source anchors and zero destination acquisition.
- [Experiments and replay](experiments/README.md): public runners, synthetic fixtures, opaque labels, aggregate outcomes and private evidence boundaries.

Raw histories, URLs, original source mappings, API states/replies and raw failure records are stored outside the repository in a private local directory. Public artifacts contain aggregates, opaque identifiers and explicitly synthetic reserved-domain examples.

Related earlier studies: [formation](../../initial-brainstorm/01-knowledge-formation/README.md) supplies synthetic source-support and adoption tests; [query and context](../../initial-brainstorm/04-query-and-context/README.md) supplies summary-to-source recovery experiments. This investigation adds actual local source-format evidence and preserves the earlier records.
