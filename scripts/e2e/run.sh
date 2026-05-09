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
# docker. Either runtime works without changes.
#
# Build mode:
#   PATHLINT_E2E_USE_BUILDER=1 (default on non-Linux hosts) — build
#     the binary inside a rust:1.85-slim container so the host does
#     not need a Linux cross-compile toolchain.
#   PATHLINT_E2E_USE_BUILDER=0 — assume `cargo build --release
#     --target $PATHLINT_E2E_TARGET` works on the host. Useful on
#     Linux hosts where no extra container layer is needed.
#
# Usage:
#   scripts/e2e/run.sh                # all three distros
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
# Pick build mode
# ----------------------------------------------------------------
# Default: use the builder container on non-Linux hosts (Windows,
# macOS) so callers do not need to install a Linux cross
# toolchain. Override with PATHLINT_E2E_USE_BUILDER=0 / 1.
default_use_builder=1
if [[ "$(uname -s 2>/dev/null || echo unknown)" == "Linux" ]]; then
    default_use_builder=0
fi
USE_BUILDER="${PATHLINT_E2E_USE_BUILDER:-${default_use_builder}}"

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
echo "==> build mode: $([[ ${USE_BUILDER} == 1 ]] && echo 'builder container' || echo 'host cargo')"

# ----------------------------------------------------------------
# Build the Linux release binary the containers will execute
# ----------------------------------------------------------------
if [[ "${USE_BUILDER}" == "1" ]]; then
    builder_image="pathlint-e2e-builder:latest"
    echo "==> building builder image ${builder_image}"
    if ! "${runtime}" build \
        --file "${SCRIPT_DIR}/Dockerfile.builder" \
        --tag "${builder_image}" \
        "${SCRIPT_DIR}"; then
        echo "scripts/e2e/run.sh: builder image build failed" >&2
        exit 2
    fi

    echo "==> running builder (cargo build --release --bin pathlint)"
    # Bind-mount the source and the target dir so cargo's
    # incremental cache survives between runs of run.sh.
    if ! "${runtime}" run --rm \
        --volume "${REPO_ROOT}:/work" \
        "${builder_image}"; then
        echo "scripts/e2e/run.sh: cargo build inside builder container failed" >&2
        exit 2
    fi
    BINARY_PATH="${REPO_ROOT}/target/release/pathlint"
else
    echo "==> building pathlint for ${TARGET_TRIPLE} on the host"
    cd "${REPO_ROOT}"
    if ! cargo build --release --target "${TARGET_TRIPLE}" --bin pathlint; then
        echo "scripts/e2e/run.sh: cargo build failed" >&2
        exit 2
    fi
    BINARY_PATH="${REPO_ROOT}/target/${TARGET_TRIPLE}/release/pathlint"
fi

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
