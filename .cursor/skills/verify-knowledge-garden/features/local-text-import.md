# Local text import

Add source imports a user-chosen UTF-8 text file into the collection, shows its source page in the reader, copies a retained original, and publishes Markdown under `sources/`.

## Sub-features

- `add-source-picker` — Toolbar **Add source** with native file selection (journeys stub `chooseFile`; native picker is manual).
- `reader-page` — Title heading and verbatim source text in the article.
- `open-original` — **Open original** resolves to byte-identical retained file.
- `restart-open` — Driver `open` after removing `.derived` still reads published Markdown.

## How to get to it (user POV)

- Choose **Add source** in the toolbar and pick a `.txt`, `.md`, or extensionless UTF-8 file.
- Drop a supported file on the window (same import path with `drop` method; not automated in default journeys).

## Driving it with kg-verify

Preconditions:

- `eval "$(./.cursor/skills/verify-knowledge-garden/scripts/kg-verify init-run)"` and `doctor` ready.
- `build-driver` completed.

- **Run journey.** `./.cursor/skills/verify-knowledge-garden/scripts/kg-verify journey local-text-import`. Exit code `0`; log at `$KG_VERIFY_ARTIFACTS/journey-local-text-import.log`.
- **Reader proof.** Artifacts include `reader-article.txt` containing `Observation V17` and `source-index.md` containing `Source text · lines`.
- **Collection proof.** `driver-list.json` lists exactly one source; retained original bytes match the fixture input file.
- **Restart proof.** Journey removes `.derived` and calls `driver open` on the imported source id; body still contains `Observation V17`.

Direct driver check (no reader UI):

```bash
echo 'Hello verify.' > "$KG_VERIFY_WORKSPACE/sample.txt"
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver import "$KG_VERIFY_WORKSPACE/sample.txt" picker
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver list 0
```

## Gotchas

- Acquisition method must be `picker` or `drop` for `driver import`.
- Duplicate byte-identical reimport reopens the same source; journeys use a fresh workspace per `init-run`.
- Native **Add source** without `launch-native` does not exercise Tauri IPC; use optional native shell for that boundary.
- Preview limit is 2 MiB; originals are still copied whole—large files need a dedicated check.
