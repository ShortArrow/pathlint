# ADR-0005: 0.0.x line allows MAJOR-equivalent BREAKING

- **Status**: Accepted
- **Date**: 2026-05-10
- **Release**: 0.0.x (policy in force from 0.0.1 onward, recorded retroactively in the ADR system)
- **Category**: 8. Process / governance (versioning policy)

## Context

`pathlint` is on Cargo's pre-1.0 versioning ramp. SemVer's own
text [§4](https://semver.org/spec/v2.0.0.html#spec-item-4) says:

> Major version zero (0.y.z) is for initial development. Anything
> MAY change at any time. The public API SHOULD NOT be considered
> stable.

Cargo interprets this stringently: under SemVer rules a `0.x.y →
0.x.(y+1)` bump *can* be BREAKING, but Cargo's resolver treats
`^0.x.y` as "exactly compatible to `0.x.*`", so dependents see
every patch bump as potentially incompatible. The crates.io
ecosystem has therefore converged on a custom convention for
the `0.0.x` sub-range: every `0.0.x → 0.0.(x+1)` bump is
MAJOR-equivalent, and dependents pin to an exact `0.0.x` line.

CHANGELOG.md states this at the top:

> The 0.0.x line treats each `0.0.x → 0.0.(x+1)` bump as
> MAJOR-equivalent (Cargo's pre-1.0 convention). Breaking changes
> are allowed within 0.0.x and announced under `### Breaking`.
> Whether and when 0.0.x graduates to 0.1.0 is undecided.

This policy has been in force since the project started; this
ADR is the retroactive record so the rule has a single citation
target for future BREAKING releases.

## Decision

Until the project ships 0.1.0, every release is permitted to
introduce BREAKING changes to public surfaces. The release notes
(CHANGELOG.md per-version section) MUST call out every BREAKING
change under a `### Breaking` heading. Each BREAKING change that
names a publicly visible type or function SHOULD link to an ADR
explaining the why (this becomes a hard requirement in the
graduation checklist; see ADR-0005's Consequences).

Concretely:

- Bumping `0.0.x → 0.0.(x+1)` may rename, remove, or change the
  signature of any public symbol.
- Embedders pin pathlint as `pathlint = "=0.0.x"` (exact) or
  `pathlint = ">=0.0.x, <0.0.(x+1)"` (range, but practically the
  same).
- Cargo's resolver lockfile records the exact 0.0.x in use; no
  "patch-level" upgrades happen silently.

The 0.0.x → 0.1.0 bump is gated separately (see the graduation
criteria in PRD.md). Once shipped, 0.1.0 ends the
"BREAKING freely" stance and pathlint follows standard SemVer
from there.

## Alternatives considered

- **Treat every `0.0.x → 0.0.(x+1)` as a strict patch (no
  BREAKING).** Rejected: pathlint is still finding the right
  shape for several core types (`PathEntry`, `analyze`
  signature, `--target` semantics). Forcing all BREAKING into a
  `0.1.0` jump would delay every release and pile risk into one
  cut.
- **Skip 0.0.x entirely and ship 0.1.0 immediately.** Rejected:
  pathlint deliberately wants the "BREAKING freely" runway to
  iterate on the design before committing to a stable API. The
  graduation checklist (PRD.md) makes that commitment explicit
  rather than implicit.
- **Use semver-exempt unstable-feature flags inside a stable
  0.1.x.** Rejected: the library is small (10 public modules)
  and the audience is still small enough that a clean BREAKING
  per release is more honest than an `#[cfg(feature =
  "unstable")]` zone of constantly-shifting types.

## Consequences

- **Positive.** Each release can land the right design rather
  than the design that fits existing callers. ADR-0001 (PathEntry)
  and ADR-0004 (provenance overlay) both depended on this licence.
- **Positive.** The CHANGELOG `### Breaking` section is a clear
  contract: embedders only need to read those sections to
  understand migration cost.
- **Negative.** Embedders cannot do `pathlint = "0.0"` to ride
  patch updates; they have to bump explicitly. crates.io's exact
  version pinning makes this no worse than any other pre-1.0
  crate, but it does mean every release demands attention.
- **Negative.** Each BREAKING release accumulates migration work
  for downstream users. The graduation checklist (PRD.md
  Graduation section) requires that every BREAKING in the 0.0.x
  line have an ADR before 0.1.0 ships — this ADR is the policy
  citation that enforces that link.
- **Follow-up.** ADR-0009 (planned) will be the graduation
  verification: at the moment of cutting 0.1.0, walk the
  CHANGELOG and confirm every `### Breaking` entry naming a
  public symbol links to an ADR. The criterion is mechanical, so
  the check is countable.
