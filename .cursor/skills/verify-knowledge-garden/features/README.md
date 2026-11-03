# Knowledge Garden verification map

This directory is the maintained source for verifying user-facing behavior. Read this index before driving the app, then open the feature file for the journey you need.

## Baseline preconditions

- From the repo root: `npm ci` (once), then `eval "$(./.cursor/skills/verify-knowledge-garden/scripts/kg-verify init-run)"`.
- `./.cursor/skills/verify-knowledge-garden/scripts/kg-verify build-driver`.
- `./.cursor/skills/verify-knowledge-garden/scripts/kg-verify doctor` must report ready.
- `$KG_VERIFY_COLLECTION` is empty or disposable; never point verification at the default Application Support collection while the shipping app may be open.
- Do not start `launch-native` if another process already owns port 1420 unless that process is this run's recorded pid.

## Driving conventions

- Start from baseline unless a feature's preconditions say otherwise.
- Prefer ARIA roles and accessible names matching `src/reader.test.ts` and `src/application.test.ts`.
- Collection mutations go through `kg-verify driver …` or reader journeys that call the same operations.
- Restore disposable workspace with `kg-verify cleanup`; keep proof files under `$KG_VERIFY_ARTIFACTS`.

## Proof and skip reporting

- Capture the user action and resulting state, not only the final screen.
- Reader journey proof: `$KG_VERIFY_ARTIFACTS/journey-<id>.log` plus feature-specific files listed in the feature doc.
- Driver proof: stdout JSON and, when relevant, paths under `$KG_VERIFY_COLLECTION/sources/`.
- Record feature ID and entry point with every artifact.
- If a precondition is missing (fixture file, opt-in live semantic env), report failure with the unmet condition; do not weaken the journey.

## Feature entry contract

Each feature file uses H1 plus four H2 sections: `Sub-features`, `How to get to it (user POV)`, `Driving it with kg-verify`, and `Gotchas`.

## Features

- [Local text import](./local-text-import.md) — Add source, reader page, Markdown, retained original, restart via driver.
- [Sources list](./sources-list.md) — Open Sources and navigate to a page.
- [Search pages](./search-pages.md) — Search toolbar, filters, open result, Back restores list.
- [Add repository](./add-repository.md) — Import a local repository folder and open a file-scoped page.
- [Watch directory](./watch-directory.md) — Watch folder, disconnect, retained original, reconnect semantics.
