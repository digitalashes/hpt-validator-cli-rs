#!/usr/bin/env bash
# CPU-profile the CLI with macOS's built-in `sample` and write plain-text results.
#
# Usage:   scripts/prof.sh <hpt-validator arguments...>
#          e.g. scripts/prof.sh standardcharges.csv v3.0
# Env:     PROF_OUT  output dir                         (default: target/prof)
#          RUNS      unsampled runs timed for time.txt  (default: 3)
#          TOP       lines in hot_self.txt              (default: 40)
#
# Outputs (in $PROF_OUT):
#   time.txt      wall time and peak memory of each unsampled run
#   hot_self.txt  functions by self time, % of samples
#   sample.txt    the full report of `sample`: call tree, then self time per function
set -euo pipefail

OUT="${PROF_OUT:-target/prof}"
RUNS="${RUNS:-3}"
TOP="${TOP:-40}"
BIN=target/profiling/hpt-validator

mkdir -p "$OUT"
cargo build --profile profiling --quiet

: > "$OUT/time.txt"
for _ in $(seq "$RUNS"); do
  /usr/bin/time -l "$BIN" "$@" 2>&1 >/dev/null |
    awk '/ real /{printf "%s s", $1} /maximum resident/{printf "  %.1f MB peak\n", $1 / 1e6}' >> "$OUT/time.txt"
done

"$BIN" "$@" >/dev/null &
pid=$!
# `sample` stops when the process exits.
sample "$pid" 3600 1 -mayDie -file "$OUT/raw.txt" >/dev/null 2>&1 || true
wait "$pid" || true
# `sample` leaves Rust's v0 symbol names mangled; c++filt reads them.
c++filt < "$OUT/raw.txt" > "$OUT/sample.txt" && rm "$OUT/raw.txt"

# The "Sort by top of stack" section: one function per line, sample count last.
awk '/^Sort by top of stack/{on = 1; next} /^Binary Images/{exit} on && NF > 1 {
  n = $NF; $NF = ""; sub(/ +\(in [^)]*\) *$/, ""); sub(/^ +/, ""); sub(/::h[0-9a-f]{16}$/, "")
  self[$0] += n; total += n
} END {
  for (s in self) printf "%6.2f%%  %s\n", 100 * self[s] / total, s
}' "$OUT/sample.txt" | sort -rn | head -n "$TOP" > "$OUT/hot_self.txt"

cat "$OUT/time.txt"
echo "done: $OUT/{time.txt,hot_self.txt,sample.txt}"
