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
- `cargo` toolchain that can target `x86_64-unknown-linux-gnu`.
  On non-Linux hosts: install the cross target with
  `rustup target add x86_64-unknown-linux-gnu` and a Linux linker
  (e.g. `mold`, `lld`, or `gcc-multilib`).

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
2. `--help` works on every subcommand
3. `pathlint catalog list` produces non-trivial output
4. `pathlint doctor` exits 0 or 1 and `--json` output starts with `[`
5. `pathlint trace ls` resolves and emits a `kind` field in JSON
6. `pathlint check` (no rules) exits 0 and `--json` is a JSON array
7. `pathlint init` writes `pathlint.toml` to a tempdir

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
- **"cargo build failed"**: the runner needs the
  `x86_64-unknown-linux-gnu` target. Override with
  `PATHLINT_E2E_TARGET=...` if you want a different triple.
- **arch keyring failures during `pacman -Sy`**: stale base image,
  `${runtime} pull archlinux:latest` to refresh and retry.

[Docker Desktop]: https://www.docker.com/products/docker-desktop
[Podman Desktop]: https://podman-desktop.io/
