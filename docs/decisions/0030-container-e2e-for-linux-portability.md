# ADR-0030: container e2e for Linux portability, local-only, no Vagrant / no post-publish smoke

- **Status**: Accepted
- **Date**: 2026-06-15
- **Release**: 0.0.14 / 0.0.21 (scripts/e2e shipped); recorded retroactively in 0.0.38 after ADR-0029 prompted a release-engineering decision sweep
- **Category**: 8. Process / governance (release engineering) + 6. External dependency (container toolchain)

## Context

pathlint reads the host's `PATH` and resolves entries against a
built-in catalog whose source paths vary by Linux distro (`/usr/bin`
vs `/usr/sbin` ordering on Arch — ADR-0014 — is the canonical
example). The host-side `cargo test` suite covers the logic in
isolation: closure-injected env lookups (ADR-0002, ADR-0006), the
`*Deps` carriers (ADR-0007), and fake catalogs make every detector
testable on whichever machine the developer happens to be sitting
at. What `cargo test` cannot cover is **whether the binary starts
at all on a distro the developer has not booted recently**.

The 0.0.14 Arch `/usr/sbin`-before-`/usr/bin` incident (now
documented in ADR-0014) was the trigger: the host integration
tests passed, the binary shipped, and a user on Arch hit a
distro-specific PATH layout the test harness had never modelled.
The fix was to add `os_baseline_linux_sbin` to the built-in
catalog, and to add a portability gate so a similar incident on
the next distro would surface before the release tag was cut.

`scripts/e2e/` is that gate. It builds the Linux release binary
(via a `rust:1.85-slim` builder container on non-Linux hosts so
the developer does not need a cross toolchain, or via host cargo
on Linux), then runs `scripts/e2e/smoke.sh` inside Ubuntu, Arch,
and Fedora containers. The smoke script exercises every
subcommand against the container's actual `PATH`, asserting
exit codes and structural shape (`--json` produces a JSON array,
`--json` carries the `kind` discriminator from ADR-0016) without
pinning exact human output. The harness has been in place since
0.0.14 / 0.0.21 — the ADR records the choice retroactively
because ADR-0029 (release trigger to tag-push) prompted a sweep
of the release-engineering decisions that were still living only
in code and `README.md`.

Two adjacent options keep coming up in conversations and are
explicitly rejected here:

1. **Vagrant / multi-VM e2e**. Plausible because each distro is
   a real boot, not a container layer. Rejected because the
   incidents container e2e catches are PATH layout / package
   manager defaults, not kernel / init-system differences;
   running them in a VM adds maintenance burden without
   widening coverage. See Alternative A.
2. **Post-publish smoke matrix in `release.yml`**. Plausible
   because it would prove "the published binary actually
   runs" on every OS that ships a prebuilt archive. Rejected
   because the build matrix already executes the binary's
   tests on each target, and a post-publish job that only runs
   `--help` would be ceremony, not coverage. See Alternative D.

## Decision

Linux portability is gated by `scripts/e2e/`, run **locally**
before any release that touches:

- the `doctor` selfcheck detectors (binary self-locate / config
  parse / env_lookup)
- the `lint` subcommand's detector set (the 0.0.34 split moved
  PATH analysis out of `doctor`; see ADR-0028)
- the built-in catalog (distro-specific source paths)
- anything that reads `/etc/os-release` or uses `expand_env`

The harness is **not wired into CI**. Pulling three base images
on every PR adds minutes of CI time for a check that the
host-side `cargo test` already covers structurally — the e2e
catches "does it start", not "does it compute the right answer".
The release procedure (`docs/RELEASE.md`) gains one line under
the pre-release checklist directing the releaser to run
`scripts/e2e/run.sh` when the release touches any of the four
categories above.

`scripts/e2e/smoke.sh` exercises seven surfaces (version, help
on every subcommand, catalog list, doctor with both human and
`--json` output, trace with both, check with no rules with
both, init writing `pathlint.toml`). It pins exit codes and
structural JSON shape only — exact human strings are covered by
host-side integration tests against fake catalogs, where output
drift surfaces as a test failure rather than smoke false-alarm.

macOS and Windows are not covered by this harness. They are
covered by the build matrix in `release.yml` (which runs `cargo
test` on the target before the binary ships) and by the
maintainer running pathlint on their own Windows machine
day-to-day. Adding a Mac VM or a Windows runner to a local
smoke harness was rejected as cost without coverage — see
Alternative B.

## Alternatives considered

- **A. Vagrant + multi-VM e2e (Ubuntu / Arch / Debian / FreeBSD
  as full VMs).** Rejected. The failure modes container e2e
  catches are PATH layout, `os-release` content, and package
  manager defaults — all of which are present in a container as
  truthfully as in a full VM. Full VMs add kernel / systemd /
  network differences that pathlint does not touch (it is a
  read-only CLI, not a daemon — ADR-0009), so the extra surface
  is wasted. Vagrant + VirtualBox also fails on a clean Windows
  laptop without Hyper-V toggle, while podman / Docker Desktop
  works rootless. Maintainability matters: one maintainer
  cannot keep four VM box images green. Container `:latest` tags
  drift but the failures are loud (`pacman -Sy keyring`
  failures) and the README documents the refresh command.

- **B. macOS / Windows post-publish smoke in `release.yml`.**
  Run `cargo install pathlint --version $TAG` on
  `macos-latest` and `windows-latest` after the GitHub Release
  step, then `pathlint --help` and `pathlint doctor`. Rejected.
  The build job in `release.yml` already compiles for
  `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, and
  `aarch64-apple-darwin` on those exact runner images, and
  `cargo test` runs against each in `ci.yml` before merge. A
  post-publish job that only runs `--help` proves nothing the
  build matrix did not already prove. The job would surface
  exactly one new failure mode — "the crates.io publish
  succeeded but the published artefact cannot install" — which
  has never happened in 35 releases and would surface in user
  bug reports within hours when it does.

- **C. Wire `scripts/e2e/` into CI on every PR.** Rejected. Three
  container builds per PR adds ~3–5 minutes of CI time for a
  smoke that does not catch logic regressions (those are
  covered by `cargo test`). A pre-release-only gate matches the
  failure mode this harness was built for (a release ships and
  Arch users find a new PATH-layout regression). Running it
  per-PR would generate noise from base-image `:latest` drift —
  the Arch keyring breaks every few months and that should not
  block a docs PR.

- **D. Replace container e2e with a release-engineering rehearsal
  on a fork repo.** Push every release candidate to
  `ShortArrow/pathlint-release-rehearsal` first, with its own
  Trusted Publishing setup and a throwaway crate name. Rejected:
  maintaining two Trusted Publisher registrations doubles the
  ceiling on "who can push what to crates.io" and the rehearsal
  would not have caught any of the failures the real workflow
  saw (the 0.0.34 partial-release pattern was a workflow logic
  bug, the 0.0.36 in-prose-token skip was a workflow logic bug,
  and both surfaced from the tag-push event no rehearsal could
  faithfully replay).

- **E. Add a Termux / aarch64-linux-android container to the
  matrix.** Tempting because PRD §3 lists termux as a supported
  target. Rejected for now: `aarch64-linux-android` is built
  from source via `pkg install rust && cargo install pathlint`
  (per `release.yml`'s build matrix comment), so a container
  smoke gives no signal beyond what `cargo install` from
  crates.io already does. A real device smoke from the
  maintainer's Android handset is the better escalation
  path if it ever bites.

## Consequences

- **Positive.** Linux portability bugs surface before the tag is
  pushed. The 0.0.14 incident retrospectively gates against the
  next-distro variant of the same failure (the next time a
  distro ships `/foo/bar/sbin` ahead of `/foo/bar/bin`, the
  smoke fails before a release goes out). The harness is run
  locally so a developer iterating on PATH detection logic can
  reproduce a distro-specific bug without pushing to CI.

- **Positive.** The harness is testable without crates.io,
  without GitHub Actions, and without network access (after
  base images are pulled once). It works on Windows / macOS
  hosts via the builder container.

- **Negative.** It is **not enforceable**. There is no CI gate
  that fails the release if `scripts/e2e/run.sh` was not run.
  The pre-release checklist in `docs/RELEASE.md` is honour-based.
  This is intentional — see Alternative C — but a release that
  legitimately should have run the smoke and didn't can ship a
  regression.

- **Negative.** `scripts/e2e/smoke.sh` and `scripts/e2e/README.md`
  must be kept in sync with the actual subcommand surface. The
  0.0.34 BREAKING (`doctor` selfcheck split, `lint` introduced —
  ADR-0028) was a case where the smoke script's "doctor produces
  PATH analysis JSON" assertion became wrong; the harness was
  updated in 0.0.38 alongside this ADR. Future BREAKING changes
  to the CLI surface require a smoke.sh update in the same PR.

- **Negative.** macOS / Windows are not covered by the e2e
  harness. They rely on `cargo test` in `ci.yml` plus
  day-to-day maintainer usage. A regression that only manifests
  on macOS / Windows shipping behaviour (not detected by
  `cargo test`) will surface in user reports rather than
  pre-release.

- **Follow-up.** If a Linux portability bug ships despite the
  e2e gate, revisit Alternative C (CI integration) with a
  concrete failure case in hand. If a macOS / Windows
  ship-behaviour bug surfaces twice in a row, revisit
  Alternative B (post-publish smoke). Decisions about which
  layer to add stay grounded in observed failure modes, not
  speculation.
