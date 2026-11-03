# Watch directory

Watch directory ingests new files from a folder, reflects disconnect when the folder moves, and keeps retained originals inspectable until removal reconciles.

## Sub-features

- `select-watch` — **Watch directory** chooses a folder.
- `reconcile` — Background steps pick up adds, renames, and deletes.
- `disconnect-state` — Unavailable path messaging while retaining prior sources.

## How to get to it (user POV)

- Choose **Watch directory** and select a folder containing importable files.

## Driving it with kg-verify

Preconditions:

- Writable watch folder under `$KG_VERIFY_WORKSPACE/incoming`.

- **Reference automation.** `npm run test:ui:integration -- -t "watched directory disconnect"` (application test) exercises real driver watch operations with reader UI.
- **Driver-only skeleton.**

```bash
mkdir -p "$KG_VERIFY_WORKSPACE/incoming"
echo 'Observation V17 at Riverside.' > "$KG_VERIFY_WORKSPACE/incoming/V17.txt"
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver watch "$KG_VERIFY_WORKSPACE/incoming"
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver watch-step
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver list 0
```

## Gotchas

- `watch-step` simulates reconciliation ticks; production uses background scheduling.
- Renaming the watched folder shows unavailability without deleting retained originals until reconciliation says so—follow the application test sequence for proof.
