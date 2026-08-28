#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="$ROOT/target/release/headroom"
OUT=${1:-"$ROOT/dist/performance-metrics.txt"}
LOG=$(mktemp "${TMPDIR:-/tmp}/headroom-metrics.XXXXXX")
mkdir -p "$(dirname "$OUT")"

start_ns=$(python3 -c 'import time; print(time.time_ns())')
"$BIN" --version >/dev/null
end_ns=$(python3 -c 'import time; print(time.time_ns())')
cold_cli_ms=$(((end_ns - start_ns) / 1000000))

"$BIN" >"$LOG" 2>&1 &
pid=$!
cleanup() {
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  rm -f "$LOG"
}
trap cleanup EXIT

for _ in {1..120}; do
  grep -q 'refresh_duration_ms=' "$LOG" && break
  sleep 1
done
sleep 2

if kill -0 "$pid" 2>/dev/null; then
  read -r cpu rss <<<"$(ps -o %cpu=,rss= -p "$pid" | awk '{print $1, $2}')"
  children=$({ pgrep -P "$pid" || true; } | wc -l | tr -d ' ')
else
  cpu="unavailable"
  rss="unavailable"
  children="unavailable"
fi
refresh_ms=$(sed -n 's/.*refresh_duration_ms=\([0-9]*\).*/\1/p' "$LOG" | tail -1)

{
  echo "version=$("$BIN" --version | awk '{print $2}')"
  echo "architecture=$(uname -m)"
  echo "binary_bytes=$(stat -f %z "$BIN")"
  echo "cold_cli_start_ms=$cold_cli_ms"
  echo "idle_cpu_percent=$cpu"
  echo "resident_memory_kib=$rss"
  echo "refresh_duration_ms=${refresh_ms:-unavailable}"
  echo "child_processes=$children"
} > "$OUT"
cat "$OUT"
