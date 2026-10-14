# Disposable durability laboratory

All original sources here are controlled authored fixtures. No private source mapping, original user data, downloaded private content or provider request is used. Existing research records are read as context and preserved. The fixture is intentionally tiny and its PNG is one pixel; it is not a perception corpus.

## Replay

From repository root:

```sh
python3 docs/research/ui-and-markdown/02-durable-markdown/experiments/run.py
python3 docs/research/ui-and-markdown/02-durable-markdown/experiments/compare_representations.py
```

`run.py` copies the committed fixture into an OS temporary directory. It mutates only that temporary copy and appends a timestamped run result. It freezes the 26 expected assertions in `expected.json` before execution; result records include its SHA-256, fixture digest, platform, Python version, each elapsed time/error, and the final rebuilt index. The final retained run is `runs/20260929-204708.json` (26/26). The earlier 24-case run `runs/20260929-204318.json` remains preserved; two explicit revision-identity assertions were added before the final 26-case run. No failed run occurred. This parser accepts **only** flat frontmatter with JSON-compatible scalar/list values; arbitrary YAML is deliberately untested. The proposal compare-and-swap check is sequential, not an atomic concurrent filesystem primitive.

`compare_representations.py` retains three encodings of one authored record and checks exact recovery. Its append check preserves surrounding text bytes; it does not model property-editor roundtripping. It rewrites only its own generated comparison artifacts on replay. Copy the experiment directory first when retaining another comparison.

Build a new vault without overwriting an existing one:

```sh
python3 docs/research/ui-and-markdown/02-durable-markdown/experiments/build_vault.py /tmp/kg-durable-vault-review
```

The builder refuses an existing target. For ordinary Obsidian checks, open that new directory as a vault and start with `START HERE.md`. It contains no `.obsidian` configuration or community plugin. Obsidian may create local app state in the temporary vault during use; that state is disposable.

## Live checks to retain separately

Record Obsidian/macOS version, existing link-update setting, screenshots and modified Markdown bytes. Do not change global settings to manufacture success.

1. Open `Flower study`; inspect flat properties, ordinary links and backlink panel for `Mira`.
2. Follow `Field observation#^observation`; verify the selected paragraph and no-identification qualifier remain visible.
3. Check standard Markdown image and wikilink image embeds. `.txt` is retained as an original asset but is outside Obsidian's documented native file formats: separately record whether the link opens externally or remains unresolved.
4. Open `Red-flower search`; record whether core Search's query embed works. App-specific saved filters/layout remain readable durable metadata, not a promised matching Obsidian interface.
5. Append a sentence to `Flower study.md` using an external editor; observe refresh and retain bytes.
6. Rename `Mira` inside Obsidian; inspect link rewriting and stable `id`. Then move `Riverside` using the filesystem externally; inspect both app identity recovery (custom script) and actual Obsidian links. Do not conflate these behaviors.
7. Change the evidence heading, edit anchored paragraph, then delete its block anchor in a copy. Record how heading and block links behave.
8. Introduce a duplicate `id`, then a missing original, in another copy; evaluate the custom rebuild conflict flags separately from what Obsidian shows.

No live step has been executed. Root environment discovery found no installed Obsidian. This is an explicit interoperability gap.

## Actual macOS watcher replay

```sh
node docs/research/ui-and-markdown/02-durable-markdown/experiments/watch_macos.mjs
```

This copies the fixture to a new temporary directory, captures directory and individual-file `fs.watch` events, executes seven operation phases, captures debounced/full scan hashes, appends a timestamped result in `runs/`, then removes the temporary copy. `watcher-expected.json` is fixed before execution. Intended append/replacement contents are assembled from original bytes, not post-mutation reads, in the strengthened second run. Both original runs are retained. The event-thinning step is a controlled adapter simulation, distinctly labeled from actual OS behavior.
