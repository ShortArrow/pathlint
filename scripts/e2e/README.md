# Container-based e2e smoke

Local-only smoke harness that runs `pathlint` inside three Linux
distro containers (Ubuntu, Arch, Fedora) to confirm the binary
still works on PATH layouts the host CI matrix does not exercise.

This is intentionally **not wired into GitHub Actions** — pulling
three images per PR adds minutes of CI time for a check that
existing host-side `cargo test` already covers structurally. Run
it locally before a release that touches:

- the doctor R3 detectors (PATH layout sensitivity)
- the built-in catalog (distro-specific source paths)
- anything that reads `/etc/os-release` or `expand_env`

## Requirements

- `bash` (the orchestrator and inner smoke script)
- Either `podman` (preferred for rootless) or `docker`. The
  orchestrator auto-detects.
- A way to produce the Linux release binary. Two modes:
  - **Builder container (default on Windows / macOS).** The
    orchestrator runs `cargo build --release --bin pathlint`
    inside a `rust:1.85-slim` image and mounts the result into
    each distro container. No Linux toolchain required on the
    host. Set `PATHLINT_E2E_USE_BUILDER=0` to skip.
  - **Host cargo (default on Linux).** Runs
    `cargo build --release --target $PATHLINT_E2E_TARGET` on the
    host. On non-Linux hosts this means installing the cross
    target with `rustup target add x86_64-unknown-linux-gnu` and
    a Linux linker (`mold`, `lld`, `gcc-multilib`); the builder
    container exists to avoid that.

## Run

```bash
# All three distros
scripts/e2e/run.sh

# Just one
scripts/e2e/run.sh ubuntu

# Subset
scripts/e2e/run.sh ubuntu fedora
```

Exit codes:

- `0` — all selected distros passed
- `1` — at least one distro failed
- `2` — environment problem (no runtime, build failure, etc.)

## What is checked

`smoke.sh` runs inside each container and verifies, without pinning
exact human output:

1. `pathlint --version` mentions `pathlint`
2. `--help` works on every subcommand (including `lint`, added in 0.0.34)
3. `pathlint catalog list` produces non-trivial output
4. `pathlint doctor` selfcheck (0.0.34 split): exits 0 or 1 and `--json` is a JSON array
5. `pathlint lint --json` PATH analysis (0.0.34 split): exits 0 or 1 and produces a JSON array
6. `pathlint trace ls` resolves and emits a `kind` field in JSON
7. `pathlint check` (no rules) exits 0 and `--json` is a JSON array
8. `pathlint init` writes `pathlint.toml` to a tempdir
9. `pathlint lint --sarif` (0.0.42+): exits 0 or 1 and carries the SARIF 2.1.0 envelope (`version`, `tool.driver.name`)

The aim is portability assertion (each distro's PATH does not
crash pathlint), not output drift detection. Output drift is
covered by the host-side integration tests.

## Adding a distro

Drop a new `Dockerfile.<name>` in this directory; `run.sh` picks it
up automatically when called as `run.sh <name>`. Keep the image
slim — `bash`, `coreutils`, `ca-certificates`, and whatever the
distro needs to install those.

## Troubleshooting

- **"neither podman nor docker is on PATH"**: install one. On
  Windows, [Docker Desktop] or [Podman Desktop]; on macOS, the
  same plus `brew install podman` works rootless.
- **"cargo build inside builder container failed"**: usually a
  registry / network issue. Re-run after a moment; the cargo
  registry cache is reused via the bind-mounted `target/` and
  the home cargo dir.
- **"cargo build failed"** (host cargo mode): the host needs the
  `x86_64-unknown-linux-gnu` target installed and a Linux linker.
  Either install them (`rustup target add ...`) or just leave
  `PATHLINT_E2E_USE_BUILDER=1` to use the builder container.
- **arch keyring failures during `pacman -Sy`**: stale base image,
  `${runtime} pull archlinux:latest` to refresh and retry.

[Docker Desktop]: https://www.docker.com/products/docker-desktop
[Podman Desktop]: https://podman-desktop.io/
