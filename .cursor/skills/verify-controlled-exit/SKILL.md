---
name: verify-controlled-exit
description: >-
  Run the TLA+ model checker on explicit Quit vs crash recovery for helper
  shutdown and audio interruption budgets. Use when changing
  src-tauri/src/processes.rs, record_controlled_shutdown,
  recover_interrupted_jobs, ensure_running, or background lifecycle docs/tests.
---

# Verify controlled exit and crash budgets

## Property (plain language)

**Promise.** An explicit Quit with a persisted controlled-exit receipt must resume interrupted audio work without charging the three-attempt crash budget, and helper processes must not survive shutdown or accept new spawns while the app is stopping.

**Harm if broken.** A user who Quit cleanly can lose transcription progress or hit a false "failed" state after reopen, as if the app had crashed repeatedly.

## Code the model tracks

| Model action | Rust location |
|--------------|---------------|
| `SpawnHelper`, `ControlledQuit` stopping | `src-tauri/src/processes.rs` (`spawn`, `shutdown`, `stopping`) |
| `record_controlled_shutdown` / receipt | `Application::record_controlled_shutdown`, `.runtime/controlled-exit` |
| `ReopenCollection` / `orderly` recovery | `Application::open` → `recover_interrupted_jobs(orderly)` |
| `MaxAudioInterruptions = 3` | `MAX_AUDIO_INTERRUPTION_ATTEMPTS` in `application.rs` |

**Sync commit (model last confirmed against code):** `e65f11b24dd0672564b0a6dc63b58102bef71c66`

If any row above changed after that commit, update `formal/controlled-exit/ControlledExit.tla` to match, rerun TLC, then advance the sync commit here only after TLC passes and `sync-code.sh` passes.

## When to run

Run this skill when a diff touches any file in the table, `src-tauri/src/desktop/lifecycle.rs`, `src-tauri/tests/background_lifecycle.rs`, or `docs/background-lifecycle.md`.

Skip when the change is docs-only typos with no behavior claims.

## Commands

From the repository root:

```bash
formal/controlled-exit/sync-code.sh
formal/controlled-exit/run-tlc.sh check
```

Requires Java (Homebrew OpenJDK is fine). The first run downloads `tla2tools` v1.8.0 into `formal/controlled-exit/.cache/`.

Optional regression probe (expects TLC to **fail** on the intentional broken spec):

```bash
formal/controlled-exit/run-tlc.sh prove-broken
```

## Interpreting TLC output

- **Pass (`Model checking completed. No error has been found.`)** — The finite-state model found no counterexample within the bounds below.
- **Fail on `NoOrderlyBudgetViolation`** — Some reachable sequence charges the crash budget after an orderly Quit while audio was processing. Read the printed trace as: start audio window → controlled quit → reopen → budget charged. Map each step to `recover_interrupted_jobs` and the `orderly` branch.
- **Fail on `NoHelpersWhileStopping`** — The model allowed helpers to remain after shutdown; check `processes::shutdown` and spawn guards.
- **Parse / incomplete successor** — Fix the `.tla` spec, not product code, unless the Rust design truly changed.

For invariant violations, read the trace with the pstack **`/statechart`** skill. It turns TLC's shortest counterexample path into plain steps. When TLC passes, `/statechart` can chart legal state movement for review.

## Scope and limits

**Verified.** All interleavings in the model of: helper spawn/stop, controlled vs unmarked quit, reopen, and audio `processing` → `pending` with interruption count updates. `MaxAudioInterruptions` is fixed at 3.

**Not verified.** File-system durability, mutex timing, semantic/URL job recovery, collection reconciliation, macOS login agents, sleep/wake, or real Whisper subprocess behavior. Those stay with `npm run check` and owner qualification in `docs/background-lifecycle.md`.

## Updating after an intentional design change

1. Edit `ControlledExit.tla` so each action still matches one atomic step in Rust.
2. Run `sync-code.sh` and `run-tlc.sh check`.
3. If the old property is wrong, rewrite the **Property** section in this file before weakening the model.
4. Advance **Sync commit** only after the above passes on the new commit.

## Maintain

After code drift without rerunning TLC, use **`/maintain-formal-verification-skill`** to realign the model and this skill.
