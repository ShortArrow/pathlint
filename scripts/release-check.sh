#!/usr/bin/env bash
# release-check.sh — local gate for the manual release fallback.
#
# Usage:
#   ./scripts/release-check.sh X.Y.Z
#
# The standard release path is the `release.yml` GitHub Actions
# workflow (see docs/RELEASE.md). This script reproduces the
# `prepare` job's quality gate locally for the rare case where
# the workflow is broken and someone has to ship by hand.
#
# Runs:
#   - cargo fmt --check
#   - cargo clippy -D warnings (all targets)
#   - cargo test
#   - cargo package --allow-dirty
#   - argument is a valid X.Y.Z string
#
# Does NOT bump Cargo.toml, write commits, or push anything. After
# this passes, the manual fallback in docs/RELEASE.md takes over.

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 X.Y.Z" >&2
    exit 2
fi

target_version="$1"
case "$target_version" in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *)
        echo "error: '$target_version' is not in X.Y.Z form" >&2
        exit 2
        ;;
esac

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

step() { printf '\n== %s ==\n' "$*"; }

step "cargo fmt --check"
cargo fmt --all -- --check

step "cargo clippy -D warnings"
cargo clippy --all-targets -- -D warnings

step "cargo test"
cargo test

step "cargo package --allow-dirty"
cargo package --allow-dirty >/dev/null

step "summary"
echo "gate green for $target_version. Run the manual fallback in docs/RELEASE.md if release.yml cannot be used."
