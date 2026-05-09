#!/usr/bin/env bash
#
# Container-based smoke test for pathlint across Linux distros.
#
# Builds a Linux release binary, then runs `scripts/e2e/smoke.sh`
# inside ubuntu / archlinux / fedora containers. Validates that
# pathlint starts, the four major subcommands exit cleanly, and
# the diagnostic output is non-empty where expected.
#
# Auto-detects podman first (rootless preferred), falls back to
# docker. Either runtime works without changes; container images
# are pulled from the runtime's default registry.
#
# Usage:
#   scripts/e2e/run.sh                # run all three distros
#   scripts/e2e/run.sh ubuntu         # one distro only
#   scripts/e2e/run.sh ubuntu archlinux
#
# Exit codes:
#   0 — all selected distros pass
#   1 — at least one distro failed
#   2 — environment problem (no runtime, build failure, etc.)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TARGET_TRIPLE="${PATHLINT_E2E_TARGET:-x86_64-unknown-linux-gnu}"

# ----------------------------------------------------------------
# Container runtime detection
# ----------------------------------------------------------------
runtime=""
if command -v podman >/dev/null 2>&1; then
    runtime="podman"
elif command -v docker >/dev/null 2>&1; then
    runtime="docker"
else
    echo "scripts/e2e/run.sh: neither podman nor docker is on PATH" >&2
    exit 2
fi
echo "==> runtime: ${runtime}"

# ----------------------------------------------------------------
# Build the Linux release binary the containers will execute
# ----------------------------------------------------------------
echo "==> building pathlint for ${TARGET_TRIPLE}"
cd "${REPO_ROOT}"
if ! cargo build --release --target "${TARGET_TRIPLE}" --bin pathlint; then
    echo "scripts/e2e/run.sh: cargo build failed" >&2
    exit 2
fi
BINARY_PATH="${REPO_ROOT}/target/${TARGET_TRIPLE}/release/pathlint"
if [[ ! -x "${BINARY_PATH}" ]]; then
    echo "scripts/e2e/run.sh: built binary not found at ${BINARY_PATH}" >&2
    exit 2
fi

# ----------------------------------------------------------------
# Distro selection
# ----------------------------------------------------------------
all_distros=(ubuntu archlinux fedora)
if [[ $# -gt 0 ]]; then
    selected=("$@")
else
    selected=("${all_distros[@]}")
fi

# ----------------------------------------------------------------
# Build + run each container
# ----------------------------------------------------------------
failures=()
for distro in "${selected[@]}"; do
    dockerfile="${SCRIPT_DIR}/Dockerfile.${distro}"
    if [[ ! -f "${dockerfile}" ]]; then
        echo "==> ${distro}: SKIP (no ${dockerfile})"
        failures+=("${distro}: no dockerfile")
        continue
    fi

    image_tag="pathlint-e2e-${distro}:latest"

    echo
    echo "==> ${distro}: build image ${image_tag}"
    if ! "${runtime}" build \
        --file "${dockerfile}" \
        --tag "${image_tag}" \
        "${SCRIPT_DIR}"; then
        echo "==> ${distro}: FAIL (build)"
        failures+=("${distro}: build failed")
        continue
    fi

    echo "==> ${distro}: run smoke.sh"
    # Mount the freshly-built binary read-only into /usr/local/bin
    # inside the container. Mount smoke.sh likewise so editing the
    # script does not require an image rebuild.
    if ! "${runtime}" run --rm \
        --volume "${BINARY_PATH}:/usr/local/bin/pathlint:ro" \
        --volume "${SCRIPT_DIR}/smoke.sh:/usr/local/bin/smoke.sh:ro" \
        "${image_tag}" \
        bash /usr/local/bin/smoke.sh; then
        echo "==> ${distro}: FAIL (smoke)"
        failures+=("${distro}: smoke failed")
        continue
    fi

    echo "==> ${distro}: PASS"
done

# ----------------------------------------------------------------
# Report
# ----------------------------------------------------------------
echo
if [[ ${#failures[@]} -eq 0 ]]; then
    echo "==> all distros PASSED"
    exit 0
else
    echo "==> failures:"
    for f in "${failures[@]}"; do
        echo "  - ${f}"
    done
    exit 1
fi
