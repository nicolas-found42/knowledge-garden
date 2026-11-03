# Resumable plain-text processing

Plain UTF-8 sources up to 32 MiB use 32 KiB provider windows with up to 1 KiB of preceding context. The complete original is copied and hashed in fixed-size buffers. HTML acquired from a URL, office documents, containers and timed media retain their separate projection routes.

Each accepted window is validated against its exact retained bytes before its evidence offsets are rebased to original UTF-8 byte positions. Immutable records are fenced by source version, source generation and the active claim. A restart reconstructs the contiguous checkpoint; committed windows do not need another provider call. An interrupted final publication can resume from the completed chain without calling the provider on empty text.

Window results remain staging data until every range is present. Final reconciliation uses a separate buffer capped at 32 MiB and at most 8 MiB of serialized proposals. It enforces the existing 500-entity, 2,000-fact and 2,000-relationship limits, plus 2,000 tags and 4,096 decisions. The reader preview remains limited to the first 2 MiB. Retained originals and evidence positions cover the complete accepted source.

A replacement publishes only after complete staging and validation. Prior originals remain retained, while generation-named current assets are installed without overwriting an existing different file. Recoverable metadata publication activates those immutable bytes. An interrupted operation may leave an unused original; it cannot activate a partial semantic generation.

Sources beyond 32 MiB retain their exact originals and report incomplete coverage. Invalid later UTF-8, excessive output, conflicting source-update judgments and cross-window targeted-correction proposals also remain incomplete; they cannot silently truncate a result or withdraw prior knowledge. Context overlap helps with boundary passages but does not establish semantic completeness or provider accuracy. Cross-window targeted corrections require a future complete-context judgment.

The integration tests use deterministic providers and disposable collections. They cover text beyond the former preview boundary, restart checkpoints, UTF-8 boundary overlap and global evidence positions, removal/re-add generation fences, changed retained bytes, resource limits and complete replacement. Real-model extraction quality and throughput at the upper size bound require separate qualification.
