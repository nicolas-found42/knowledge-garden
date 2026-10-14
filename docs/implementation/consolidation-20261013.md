# Consolidated work checkpoint — October 13, 2026

This checkpoint consolidates the previously integrated application and the unfinished work from the extra branches and worktrees. It is a cleanup and preservation checkpoint, not completion of specification #1. Open tickets remain open.

The current implementation incorporates pending PDF extraction, audio validation fixtures, repository ingestion, Boolean queries, source-event timelines, provider availability recovery, and unique moved-page lookup. These slices retain their original tests and fixtures; complete acceptance, native release, scale, concurrency, watcher, Obsidian, and model-quality qualification remains outstanding.

Superseded October 3 provider, reconciliation, and photo prototypes are preserved through history-only merges. Their changes remain accessible at the snapshot commits below without replacing the newer implementations or changing the selected helper provider. Historical stashes are also retained as reachable parent commits. The vendored Whisper package-version edit is preserved as `archives/vendored-whisper-local-edits.patch`; the pinned upstream submodule is unchanged.

## Preserved working-copy commits

| Original checkout | Snapshot commit |
| --- | --- |
| knowledge-garden | `035d9d746e3d8b1eb3ddf59824563dfd6abd91bc` |
| ollama-cloud-provider | `01d85d0b1498c27045db75ba238f3cd16cd4d3c7` |
| ticket-12-role-followup | `b90623d23a3430f0b7f6ad110a608b0b80e38a5a` |
| ticket-13 | `7f8fcb82aa310bffeed83b8a7690d2d7bd995f3f` |
| ticket11 | `596448435e6c041baf061e3e75d9abd3fed3469e` |
| ticket14 | `7d2eab45fa0f89e45ac2ecec5a607f6d2cb915d8` |
| ticket16 | `80ca71c5b72aa1afc30fce3b86b1b1bbac3aac84` |
| ticket21 | `a28a99abcb0fbead85e2dbdbaecd75bd0563b7dc` |
| ticket25 | `f451e1817ac9b87dfa1763db12d551bb22755d80` |
| ticket29 | `a5d7d377c51d32a7be9962d612b3efc865e2af0b` |
| ticket8 | `755b6344a3b386309a8af64a966ad8f310cd32c9` |

## Preservation and cleanup

Before removal, all branches and stashes were saved in a verified Git bundle under `Documents/knowledge-garden-cleanup-20261013`. Extra checkout files, including ignored build assets and local caches, are retained there in a quarantine directory. Recover source history from Git; use the local manifest and bundle for prior branch names and full working-copy recovery. Generated Python bytecode and local runtime links are excluded from the committed source.

## Qualification limits

The cleanup checks establish frontend type/build compatibility, reader tests, and native compilation as recorded in the PR. They do not qualify every feature or claim the unfinished tickets are done. Original RED, timeout, interrupted, and Jev-escalation evidence remains in the previous experiment directories.
