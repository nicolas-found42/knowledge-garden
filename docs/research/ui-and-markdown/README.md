# UI and Markdown research

[All research](../README.md) · [Initial collection](../initial-brainstorm/README.md) · [Expanded collection](../expanded-exploration/README.md) · [Confirmed brief](../../../.scratch/ideas/current-specification.md)

Research date: **2026-09-29, America/New_York**. UTC experiment records also fall on September 30. This collection completes an evidence-bounded research wave for a later specification interview. It is **not an accepted project specification or architecture**. The confirmed brief remains unchanged.

Start with the [synthesis](2026-09-29-synthesis.md), [requirement-to-evidence map](requirement-evidence-map.md) and [decision brief for the next interview](decision-brief.md).

| Concern | Inspectable outputs |
| --- | --- |
| [Precedents and stacks](01-precedents-and-stacks/README.md) | 13 behavior-selected apps; SwiftUI/WKWebView, Tauri 2 and Electron comparison; 46 primary-source records |
| [Durable Markdown](02-durable-markdown/README.md) | Three representations; 26 disk invariants; 7 actual Mac watcher assertions; no-plugin disposable vault and explicit missing Obsidian check |
| [Interface concepts](03-interface-concepts/README.md) | Three materially different clickable concepts over one corpus; all eight purposes; actual browser measurements up to 50k nodes/400k edges |
| [Search and timelines](04-search-and-timelines/README.md) | General-file corpus; nine private structural samples kept outside repo; four search modes; 44 measured provider calls; cited-answer audit; three time axes |

## Open the comparisons

From the repository root:

```sh
python3 docs/research/ui-and-markdown/serve.py
```

Then open the [quiet reading concept](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/index.html?variant=reading), [research workspace](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/index.html?variant=workspace), or [visual canvas](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/index.html?variant=canvas). The floating selector preserves selection and current mode; opening each fresh link shows its initial composition. File HTML alone is insufficient because the concepts fetch the shared corpus. The server binds only to localhost. Its optional bounded measurement endpoint records benchmark JSON locally; it exposes no inference endpoint.

The [point benchmark](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/scale.html) and [edge/group benchmark](http://127.0.0.1:8768/docs/research/ui-and-markdown/03-interface-concepts/edges-scale.html) autorun in the current browser. Every captured result has a new timestamp/hash filename. The [retained browser records](03-interface-concepts/browser-records/) contain actual measured runs and verification/failure records. These links depend on a running preview; the saved source and replay commands are the durable artifacts.

## Evidence boundaries

All 27 UI items, 30 relationships and eight tasks are public/controlled research. Relationships, labels and comparison tasks are authored, not measured entity-extraction accuracy or observed human preferences. The source asset manifest records 21 files; eight actual containers/text cases were separately checked. The personal-file probe inspected nine selected files from bounded Documents paths and exposes only safe aggregates. Original files/setups remain unchanged, private copies/mappings stay outside the repository, and conversation destination acquisition attempts are zero.

Actual Obsidian UI checks remain **unavailable because Obsidian is not installed** in the inspected environment. No seamless-compatibility claim follows from documentation or the disk fixture. The disposable vault/checklist preserves this validation requirement.

The [usage ledger](usage.json) records 44 successful measured requests and **$0.005773986 reported router cost**. Browser chooser/helper work and coding agents are excluded. The [validation record](validation.json) distinguishes artifact checks, actual browser interactions, documentation and remaining limits. No project implementation, branch, commit, issue or accepted ADR was created.
