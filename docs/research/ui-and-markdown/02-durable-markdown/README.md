# Durable Markdown and ordinary Obsidian

[UI and Markdown collection](../README.md) · [Working brief](../../../../.scratch/ideas/current-specification.md)

The [research report](2026-09-29-durable-markdown.md) compares three representations, tests disk rebuild/external-edit/identity/proposal behavior, and provides an ordinary-plugin-free Obsidian fixture. These are research alternatives, not an accepted architecture.

- [Experiment instructions](experiments/README.md), [frozen assertions](experiments/expected.json), [26-case results](experiments/runs/20260929-204708.json).
- [macOS watcher experiment](experiments/watch_macos.mjs), [second actual event stream and 7/7 checks](experiments/runs/watch-2026-09-30T00-49-28.761Z.json).
- [Three actual Markdown representations](experiments/representations/), [comparison results](experiments/representation-results.json).
- [Disposable vault start](experiments/disposable-vault/START%20HERE.md), [official source manifest](experiments/primary-source-manifest.json).

Actual Obsidian behavior remains **untested**: the root researcher found no installed Obsidian application. The fixture and explicit live checklist preserve the requirement for a later check; successful custom disk tests do not substitute for it.
