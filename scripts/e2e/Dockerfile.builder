# Builder image for the e2e harness.
#
# Compiles the Linux release binary inside a container so the host
# does not need a Linux cross-compile toolchain. Used by run.sh
# when PATHLINT_E2E_USE_BUILDER=1 (default on Windows / macOS) or
# when --use-builder is passed explicitly.
#
# The repo source is bind-mounted at /work, target/ is bind-mounted
# at /work/target so cargo's incremental cache survives between
# runs. The container produces /work/target/release/pathlint and
# exits.

FROM rust:1.85-slim

# Cargo registry / git fetch needs ca-certificates; pkg-config and
# libssl-dev would be needed for some dependencies but pathlint
# only uses pure-Rust crates so we keep the image lean.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work

# Default command: cargo build the pathlint binary in release mode
# and exit. Caller bind-mounts the source at /work and target/ at
# /work/target.
CMD ["cargo", "build", "--release", "--bin", "pathlint"]
