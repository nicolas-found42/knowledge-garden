# Search pages

Search finds durable pages by words, title, tags, and format filters, opens a result, and restores the result list with Back.

## Sub-features

- `open-search` — Toolbar **Search** opens the search panel.
- `filter-and-query` — Searchbox **Words or title**, tag checkboxes, **Format** combobox.
- `open-result` — Open a matching page from results.
- `restore-list` — **Back** returns to the same query and selection focus.

## How to get to it (user POV)

- Choose **Search** in the toolbar.
- Enter query text, optional tags/format, then open a result link or button.

## Driving it with kg-verify

Preconditions:

- Collection contains tagged searchable pages (see `src/reader.test.ts` fixtures via mocked API, or import real fixtures then index).

- **Reference automation.** From repo root: `npm run test:ui -- src/reader.test.ts -t "searches durable page results"`.
- **Native collection search.** Import sources with `driver import`, then use `driver search '{"query":"Observation"}'` (or plain query string) and compare JSON page ids to `driver list 0`.

Dedicated `kg-verify journey search-pages` is not seeded yet; failing to run the reader test while claiming this feature verified is incomplete.

## Gotchas

- Search index may lag immediately after import; wait for list/search consistency before asserting empty results.
- Remembered-meaning mode and keyword mode interact; see reader tests for mode restore.
