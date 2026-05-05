#!/bin/sh
# 0.0.18+: hyperfine-based startup-time baseline.
#
# Use: scripts/bench.sh [pathlint binary path]
#
# Default binary path is ./target/release/pathlint. Builds in
# release mode automatically when the binary is missing. Output is
# the hyperfine table on stdout, suitable for pasting into release
# notes alongside the host description.
#
# Why this script exists: PRD §12 claims "<50 ms startup time on a
# modern host". Until 0.0.18 there was no measurement script
# attached to the repo; the claim was a future-tense promise.
# Running this on a release build verifies the claim and provides
# a quotable median for release notes.
#
# This is *not* a CI gate. The Windows runner in particular is
# variance-prone enough that a hard threshold would flake. Treat
# bench.sh as a developer/reviewer tool.

set -eu

BIN="${1:-./target/release/pathlint}"

if [ ! -x "$BIN" ]; then
  echo "scripts/bench.sh: building release binary at $BIN" >&2
  cargo build --release --bin pathlint
  BIN="./target/release/pathlint"
fi

if ! command -v hyperfine >/dev/null 2>&1; then
  cat >&2 <<'EOF'
scripts/bench.sh: hyperfine is not on PATH.
  - Linux/macOS: install via your package manager (e.g. apt install hyperfine,
    brew install hyperfine).
  - Windows: scoop install hyperfine, or download from
    https://github.com/sharkdp/hyperfine/releases.
EOF
  exit 1
fi

echo "scripts/bench.sh: warming up and benchmarking startup of $BIN"
hyperfine \
  --warmup 3 \
  --shell=none \
  "$BIN --version" \
  "$BIN --help" \
  "$BIN catalog list --names-only"
