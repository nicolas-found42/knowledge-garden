#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
CACHE="$ROOT/.cache"
JAR="$CACHE/tla2tools.jar"
TLA_VERSION="1.8.0"
JAR_URL="https://github.com/tlaplus/tlaplus/releases/download/v${TLA_VERSION}/tla2tools.jar"

mkdir -p "$CACHE"
if [[ ! -f "$JAR" ]]; then
  curl -fsSL -o "$JAR" "$JAR_URL"
fi

MODE="${1:-check}"
cd "$ROOT"

run_tlc() {
  local module="$1"
  local cfg="$2"
  java -XX:+UseParallelGC -cp "$JAR" tlc2.TLC -config "$cfg" "$module"
}

case "$MODE" in
  check)
    run_tlc ControlledExit.tla ControlledExit.cfg
    ;;
  prove-broken)
    if run_tlc ControlledExit_Broken.tla ControlledExit_Broken.cfg; then
      echo "expected ControlledExit_Broken to violate OrderlyRecoveryPreservesBudget" >&2
      exit 1
    fi
    echo "ControlledExit_Broken correctly failed TLC (property violated)."
    ;;
  *)
    echo "usage: $0 [check|prove-broken]" >&2
    exit 2
    ;;
esac
