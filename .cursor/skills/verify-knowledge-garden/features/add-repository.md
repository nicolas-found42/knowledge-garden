# Add repository

Add repository imports a local folder, retains file originals, and exposes navigable pages for supported in-repo Markdown and doc comments.

## Sub-features

- `choose-repo` — **Add repository** and directory picker.
- `import-summary` — Status area reports imported and excluded paths.
- `open-repo-source` — Open a file-scoped source from the list.

## How to get to it (user POV)

- Choose **Add repository** in the toolbar and select a local repository folder.

## Driving it with kg-verify

Preconditions:

- Fixture tree at `src-tauri/tests/fixtures/repositories/garden` (committed in repo).

- **Driver import.** `./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver repository-import src-tauri/tests/fixtures/repositories/garden` — stdout JSON with `import` and `pages`.
- **Reader reference.** `npm run test:ui -- src/reader.test.ts -t "imports a chosen repository"`.

## Gotchas

- Repository import can exclude paths; assert on `exclusions` in driver JSON, not only page count.
- Relative paths in source information must match the supplied repository root.
