# Documentation evidence

This concern performs source acquisition and documentation inspection, not stack execution or an installed-app usability study. See the [report](../2026-09-29-precedents-and-stacks.md) for those limits.

`source-manifest.json` records independently selected public primary sources, acquisition timestamps and SHA-256 hashes of HTTP response bytes. A captured application shell does not establish its content; explicit limitations identify Obsidian/Apple shells and the Photos redirect. Current official Markdown/JSON endpoints or the official documentation source repository were used for content inspection. `could_not_retrieve` records the obsolete Kagi path rather than discarding failure.

Replay acquisition with:

```sh
python3 docs/research/ui-and-markdown/01-precedents-and-stacks/experiments/capture_sources.py
```

The script needs only Python's standard library and network access. It writes the new manifest here and temporary inspection text under `/tmp/kg-precedents`; rerunning replaces current acquisition metadata, so preserve the original manifest before comparing later source revisions. It does not acquire conversation URLs, inspect private files, run downloaded code or touch installed apps. Documentation may change; hashes pin these captured bytes but do not pin application releases.

Initial capture was followed by additional captures for official Obsidian source Markdown and Apple/Kagi content endpoints. The checked-in script includes those endpoints for replay. Inspection annotations in the manifest were manually added after reading the sources and will need to be added again after replay. No captured page body is copied into the repository.
