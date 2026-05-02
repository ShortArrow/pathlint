#!/usr/bin/env bash
# release-check.sh — gate the cargo & docs state before cutting a release.
#
# Usage:
#   ./scripts/release-check.sh X.Y.Z
#
# Runs the same checks `docs/RELEASE.md` step 1 lists, plus a few
# integrity checks that are easy to forget by hand:
#   - cargo fmt / clippy / test green
#   - cargo package --allow-dirty succeeds
#   - either Cargo.toml already says X.Y.Z and CHANGELOG has [X.Y.Z]
#     (bump-on-develop), or current version is < X.Y.Z and CHANGELOG
#     has an [Unreleased] section ready to be renamed (bump-on-main).
#
# Exits 0 when ready to cut, non-zero with a one-line reason
# otherwise. Safe to run repeatedly.

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

step "Cargo.toml version sanity"
current_version="$(awk -F'"' '/^version[[:space:]]*=/{print $2; exit}' Cargo.toml)"
if [[ -z "$current_version" ]]; then
    echo "error: could not read version from Cargo.toml" >&2
    exit 1
fi
echo "  Cargo.toml version: $current_version"
echo "  target version:     $target_version"

bump_mode=""
if [[ "$current_version" == "$target_version" ]]; then
    bump_mode="develop"
    echo "  mode: bump-on-develop (Cargo.toml already at $target_version)"
elif [[ "$(printf '%s\n%s\n' "$current_version" "$target_version" | sort -V | head -1)" == "$current_version" ]] && [[ "$current_version" != "$target_version" ]]; then
    bump_mode="main"
    echo "  mode: bump-on-main (will bump $current_version -> $target_version on main)"
else
    echo "error: Cargo.toml version $current_version is greater than target $target_version" >&2
    exit 1
fi

step "CHANGELOG.md sanity"
if [[ "$bump_mode" == "develop" ]]; then
    if ! grep -qE "^## \[$target_version\]" CHANGELOG.md; then
        echo "error: bump-on-develop mode but CHANGELOG.md has no '## [$target_version]' section" >&2
        exit 1
    fi
    echo "  found '## [$target_version]' in CHANGELOG.md"
else
    if ! grep -qE "^## \[Unreleased\]" CHANGELOG.md; then
        echo "error: bump-on-main mode but CHANGELOG.md has no '## [Unreleased]' section to rename" >&2
        exit 1
    fi
    echo "  found '## [Unreleased]' in CHANGELOG.md (will be renamed to [$target_version])"
fi

step "summary"
echo "ready to cut $target_version (mode: bump-on-$bump_mode)"
