# Bulk import workloads and budgets

`src-tauri/examples/bulk_import_workload.rs` is the repeatable workload for the
progressive-import behavior in [#33](https://github.com/nicolas-found42/knowledge-garden/issues/33).
It drives only the public `Application` boundary: it creates a mixed-size corpus, watches
the directory, and then alternates one bounded worker turn with the reader operations a
person performs during a large import.

## What it measures

One run prints one JSON record:

| Field                                                                                           | Meaning                                                                                |
| ----------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `files`, `corpus_bytes`                                                                         | Corpus size and total on-disk bytes                                                    |
| `import_seconds`, `sources_per_second`, `bytes_per_second`                                      | Discovery, copy, extraction, and indexing through the watched workflow                 |
| `worker_turns`                                                                                  | Bounded turns consumed by the import                                                   |
| `concurrent_navigation_samples`, `concurrent_navigation_p50_us`, `concurrent_navigation_p95_us` | Timed keyword search and source opens issued between turns while the import still runs |
| `incremental_update_seconds`                                                                    | Time from rewriting one processed source to its second retained version                |
| `cold_rebuild_seconds`                                                                          | `rebuild_index` over the finished collection                                           |
| `peak_rss_bytes`                                                                                | Resident set size read from `ps` at the end of the run                                 |
| `hardware`, `build_profile`                                                                     | Recorded so a result is never separated from its machine                               |

The harness fails rather than reporting partial evidence: it requires an empty
collection, the full source count after import, and a second retained version of the
revised source.

## Running it

```sh
cargo build --locked --release --manifest-path src-tauri/Cargo.toml \
  --example bulk_import_workload
src-tauri/target/release/examples/bulk_import_workload <collection> <corpus> 2000
```

Use a disposable collection and corpus. The semantic provider is a recorded
deterministic stub, so the numbers describe the application's own bounded work and not
provider latency; a live provider result belongs to
[#34](https://github.com/nicolas-found42/knowledge-garden/issues/34).

## Budgets

The workload is the artifact that records measured behavior. Until a quiet-machine
series exists, the recorded budgets are these:

- Peak memory must not grow proportionally with corpus size between a 500-file and a
  2,000-file run. Both runs must stay within the same order of magnitude.
- `concurrent_navigation_p95_us` must stay below the time of one worker turn, so a
  reader is never blocked behind bulk work for a whole batch.
- `cold_rebuild_seconds` must not scale worse than linearly in source count, and an
  interrupted rebuild must resume rather than restart, per
  [ADR 0008](../adr/0008-rebuild-derived-stores-through-a-resumable-stage.md).

## Recorded results

No accepted result exists yet. Two 60-file runs on a loaded 48 GiB Apple Silicon host
gave 5.69 and 2.85 sources per second, a 2x spread larger than any effect the workload
could resolve, so they are inconclusive and are not used as a budget. Each run also
included the harness's own per-turn search, which is intentional for the latency figure
but contaminates throughput relative to a bare import.

Run the workload on a quiet machine, at least five alternating runs per size, and record
the median and range with the hardware fields from the JSON. The target-machine
qualification at 250,000 sources remains
[#35](https://github.com/nicolas-found42/knowledge-garden/issues/35) and needs the
owner's 16 GiB Mac.
