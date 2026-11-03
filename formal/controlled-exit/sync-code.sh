#!/usr/bin/env bash
# Confirms Rust still matches the ControlledExit TLA+ model (manual sync contract).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP="$ROOT/src-tauri/src/application.rs"
PROC="$ROOT/src-tauri/src/processes.rs"

grep -q 'MAX_AUDIO_INTERRUPTION_ATTEMPTS: u32 = 3' "$APP"
grep -q 'knowledge-garden-controlled-exit-v1' "$APP"
grep -q 'fn recover_interrupted_jobs' "$APP"
grep -q 'if orderly' "$APP"
grep -q 'processing_interruptions.saturating_add(1)' "$APP"
grep -q 'A bounded media window was paused on Quit' "$APP"
if rg -U 'if orderly\s*\{[^{}]*audio\.state == "processing"[^{}]*processing_interruptions' "$APP" >/dev/null 2>&1; then
  echo "sync-code: orderly audio recovery must not increment processing_interruptions" >&2
  exit 1
fi
grep -q 'Application is quitting' "$PROC"
grep -q 'registry.stopping = true' "$PROC"

echo "sync-code: application.rs and processes.rs still expose the modeled Quit/recovery hooks."
