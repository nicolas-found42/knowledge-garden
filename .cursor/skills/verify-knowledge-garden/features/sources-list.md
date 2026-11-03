# Sources list

Sources opens the collection's paginated source list so the user can return to any imported page.

## Sub-features

- `open-sources` — Toolbar **Sources** shows the list panel.
- `open-source-row` — Activate a source button to load its reader page.
- `pagination` — **More sources** when the list exceeds one page (see reader tests).

## How to get to it (user POV)

- Choose **Sources** in the toolbar from the main reader canvas.
- Select a source by its displayed title button.

## Driving it with kg-verify

Preconditions:

- At least one imported source in `$KG_VERIFY_COLLECTION` (run `local-text-import` journey or `driver import` first).

- **Seed.** `./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver import "$KG_VERIFY_WORKSPACE/Riverside notes.txt" picker` after creating that file in the workspace (or reuse post-journey collection).
- **List.** `./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver list 0` — JSON `sources` array length ≥ 1.
- **Reader navigation (automated reference).** Run `npm run test:ui -- src/reader.test.ts -t "keeps explicit source pagination"` from repo root when changing pagination; map entry points must stay aligned with **Sources** and **More sources**.

Mapped journey file: not yet extracted—use driver list + integration test above until a dedicated `journeys/sources-list.test.ts` lands.

## Gotchas

- Stale list ordering can race background refresh; reader tests cover "older refresh finishes later".
- Source buttons use the imported display title, not the filesystem path.
