---
name: verify-knowledge-garden
description: >-
  Drive Knowledge Garden's Mac reader and Rust collection through the public
  Application driver and mapped reader journeys. Use when verifying import,
  navigation, search, or collection persistence after UI or src-tauri changes.
---

# Verify Knowledge Garden

Knowledge Garden is a **Tauri macOS desktop app** (reader web UI + native collection engine). Automated verification uses the same boundary as `src/application.test.ts`: the **`application_driver`** example talks to the real `Application` type, and reader journeys mount `mountReader` with a driver-backed `GardenApi`. That exercises import, Markdown publication, retained originals, and reader navigation without mocking collection logic.

**Not covered here:** native macOS file-picker delivery, Finder drag-and-drop into the webview, live semantic providers, and live Whisper transcription. Those stay manual or behind opt-in driver operations (`live-import`, `audio-live-import`, …) with explicit env flags.

**Isolation:** One collection writer per process. Set `KNOWLEDGE_GARDEN_COLLECTION` (native shell) and `KG_VERIFY_COLLECTION` (driver/journeys) to a disposable directory. Do not run two native instances against the same collection. Do not drive port **1420** unless this run started `launch-native`.

## Launch

From the repository root, after `npm ci` once:

```bash
chmod +x .cursor/skills/verify-knowledge-garden/scripts/kg-verify
eval "$(./.cursor/skills/verify-knowledge-garden/scripts/kg-verify init-run)"
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify build-driver
```

**Reader + collection (default verification):** `build-driver` is the launch step. Each `driver` or `journey` invocation opens the collection in a fresh process (same as integration tests).

**Optional native shell** (real Tauri IPC and webview):

```bash
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify launch-native
```

Ready when `curl -sf http://127.0.0.1:1420/` succeeds. The window title is **Knowledge Garden**; collection path is `KNOWLEDGE_GARDEN_COLLECTION=$KG_VERIFY_COLLECTION`.

First-time native builds may require `npm run provision:assets` (Whisper submodule and pinned model). Text-only journeys do not need that for the driver path.

**Teardown:**

```bash
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify stop-native   # if launch-native was used
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify cleanup
```

## Doctor

```bash
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify doctor
```

Confirms Apple Silicon/macOS expectations (warn-only off-platform), Node 24, `cargo`, built `application_driver`, disposable collection directory, and that port 1420 is not owned by another dev server unless this `RUN_ID` started it.

## Drive

Harness: **`kg-verify`** plus the feature map under [`features/`](features/README.md).

```bash
# Collection operations (JSON on stdout) — see src-tauri/examples/application_driver.rs
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver import /path/to/file.txt picker
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver list 0
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify driver open <source-id>

# Reader journey (jsdom + user-event + real driver), named in the feature map
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify journey local-text-import
```

Reader journeys use **ARIA roles and accessible names** from `src/reader.ts` (for example `Add source`, `Sources`, `Open original`, `Search`). Prefer those over CSS selectors.

For native-only flows (picker, drop), use the packaged app with `KNOWLEDGE_GARDEN_COLLECTION` as in the README; do not attach a second writer to the user's default Application Support collection.

## Evidence

Proof directory: **`$KG_VERIFY_ARTIFACTS`** (default `.cursor/skills/verify-knowledge-garden/artifacts/<RUN_ID>/`).

Standards:

- Exercise the **public user path** (`Add source` → reader → `Open original`), not private Rust test hooks except via documented `application_driver` operations.
- Capture **action and resulting state**: journey log (`journey-<name>.log`), `source-index.md`, `reader-article.txt`, `driver-list.json` for local text import.
- Verify **side effects**: retained original bytes on disk, `sources/.../index.md`, and `list` returning one source.
- Mocks: reader journeys stub file picker (`chooseFile`) but call real `import` on the driver—same as `src/application.test.ts`.

## Cleanup

```bash
./.cursor/skills/verify-knowledge-garden/scripts/kg-verify cleanup
```

Removes `$KG_VERIFY_WORKSPACE` and stops the native pid recorded for this `RUN_ID`. **Does not** delete `$KG_VERIFY_ARTIFACTS`. Never `pkill` by process name; only stop the pid from `launch-native`.

## Helpers

All paths relative to the repository root.

| Helper | Purpose |
|--------|---------|
| `./.cursor/skills/verify-knowledge-garden/scripts/kg-verify` | `init-run`, `doctor`, `build-driver`, `driver`, `journey`, `launch-native`, `stop-native`, `cleanup` |
| `.cursor/skills/verify-knowledge-garden/vitest.config.ts` | Vitest entry for `journeys/*.test.ts` |
| `src-tauri/target/debug/examples/application_driver` | Public Application CLI (built by `build-driver`) |

## Maintain

When routes, toolbar labels, or driver operations change, update the feature map and journeys, then rerun `doctor` and at least one affected journey. Use **`/maintain-verification-skill`** to keep the map aligned with the app.
