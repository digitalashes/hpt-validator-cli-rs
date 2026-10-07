#!/bin/sh
# Compares this CLI with upstream's, pinned in UPSTREAM.lock, on every case of the core
# repository checked out next to this one. Needs Node 20.19.4
# (fnm exec --using 20.19.4 oracle/run.sh).
set -eu
cd "$(dirname "$0")/.."
[ "$(node --version)" = v20.19.4 ] || { echo "need node v20.19.4, have $(node --version)" >&2; exit 1; }
sha=$(awk '$1 == "CMSgov/hpt-validator-cli" { print $2 }' UPSTREAM.lock)
up=target/upstream-cli
[ -d $up ] || git clone -q https://github.com/CMSgov/hpt-validator-cli.git $up
if [ "$(cat $up/.built 2>/dev/null)" != "$sha" ]; then
  git -C $up checkout -q "$sha"
  (cd $up && npm ci --silent && npm run build --silent)
  echo "$sha" > $up/.built
fi
cargo build -q --release
# Plain output on a terminal too, as this CLI's is when piped.
FORCE_COLOR=0 node oracle/compare.mjs $up target/release/hpt-validator ../hpt-validator-rs/tests/cases
