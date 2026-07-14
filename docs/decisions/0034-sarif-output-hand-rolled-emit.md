# ADR-0034: `lint --sarif` is hand-rolled emit-only SARIF; rule ids are the kind names

- **Status**: Accepted
- **Date**: 2026-07-14
- **Release**: 0.0.42
- **Category**: 7. Persistence / data format (+6. External dependency, +1. Public API surface)

## Context

ADR-0031 committed pathlint to SARIF 2.1.0 output as one of its two
ecosystem integration points and deferred the implementation, noting
that "the `sarif` Rust crate is the candidate dependency" and that a
separate ADR would record the actual dependency choice, the
detector-kind → SARIF-rule mapping (with stable `rule.id` values),
and the drift-gating strategy. This is that ADR.

Facts that shaped the implementation:

- **GitHub Code Scanning's ingestion minimum** (verified against
  the GitHub SARIF-support reference): every result needs
  `locations[0].physicalLocation.artifactLocation.uri` plus
  `region.startLine`; `tool.driver` needs `name` and `rules[]`;
  every rule needs `id`, `shortDescription.text`,
  `fullDescription.text`, and `help.text`.
- **pathlint findings are not file findings.** A `duplicate` or
  `missing` diagnostic describes a PATH *entry* — an environment
  string, not a repository artifact. SARIF (and GitHub's ingestion
  of it) is file-shaped, so something must be chosen as the anchor.
- **One lint kind has dynamic wire names.** `Kind::Conflict`
  serializes under the relation-declared diagnostic name
  (`mise_activate_both` today; user relations can mint new ones),
  so the rule table cannot be a fixed compile-time list.
- **Dependency landscape at implementation time**: `serde-sarif`
  is at 0.8 (still 0.x, full object model, parse + emit);
  `zizmor-sarif` is a stable 1.x but is another tool's internal
  model subset; pathlint only ever *emits*.

## Decision

1. **No new dependency.** `lint --sarif` serializes ~12 private
   emit-only structs (`SarifLog` → `SarifLogicalLocation`) defined
   next to the other renderers in `format::doctor_sarif`, using the
   `serde` / `serde_json` already in the tree. The subset covers
   exactly GitHub's required properties plus `logicalLocations`.

2. **`ruleId` = the snake_case kind name** the `--json` output has
   used since the detectors landed (`duplicate`, `missing`,
   `malformed`, ...). From 0.0.42 on these ids are a **published
   contract twice over** (JSON `kind` + SARIF `ruleId`): renaming
   one breaks consumer dashboards keyed on ruleId and is a
   Breaking change.

3. **`rules[]` is built dynamically** as the union of kind names
   present in the current output. The static kinds carry fixed
   short/full/help texts; relation-declared conflict ids share one
   generic conflict description. This is the only shape that stays
   correct as user relations mint new conflict names.

4. **Severity → level**: `Error` → `error`, `Warn` → `warning`,
   `Info` → `note`.

5. **Location anchoring**: every result's physical location points
   at the discovered `pathlint.toml` (`startLine` 1), falling back
   to the literal relative `pathlint.toml` when discovery found
   nothing; backslashes are normalized to forward slashes. The
   PATH entry itself travels in `message.text` and in
   `logicalLocations` (`name` = entry, `fullyQualifiedName` =
   `PATH[<index>]`, or the source name for catalog-level
   findings). The config is the one repository artifact that
   declares the user's PATH intent, so it is the honest anchor;
   in the primary CI use case (repo root, Linux runner) the uri
   comes out as the repo-relative `pathlint.toml` GitHub wants.

6. **Drift gating is unit-test golden assertions**, not a sixth
   published schema. `format::tests` pins the envelope, the rule
   ids, the level mapping, the anchor fallback, and the
   uri normalization; the e2e suite re-checks the GitHub-required
   fields through the real binary. No `sarif.schema.json` release
   asset: the official SARIF 2.1.0 schema is the contract, and
   publishing pathlint's private subset as a schema would invite
   consumers to validate against the wrong thing.

7. **Message wording is single-sourced**: the SARIF message reuses
   the same per-kind detail sentences the human renderer prints
   (extracted into a shared helper), so `lint` and `lint --sarif`
   never describe the same finding differently.

## Alternatives considered

- **A. Adopt `serde-sarif` 0.8.** Full, correct object model — and
  a 0.x dependency whose API churn pathlint would track forever,
  for a tool that never parses SARIF. The same reasoning that
  deferred schemars 1.0 applies with less counterweight: here the
  hand-rolled subset is ~150 lines against a whole-model crate.
  Revisit if pathlint ever needs to *read* SARIF.

- **B. Adopt `zizmor-sarif` 1.x.** Stable semver, minimal — but it
  is another analyzer's internal model shaped by that tool's
  needs, and coupling pathlint's wire output to a third party's
  refactoring cadence is strictly worse than owning 150 lines.

- **C. External converter binary (`pathlint lint --json |
  pathlint-sarif`), the `clippy-sarif` pattern.** Rejected by
  ADR-0031 already: a separate binary means version skew between
  converter and producer, and the rule metadata (descriptions,
  help text) would live outside the crate that owns the detectors.
  clippy needs the pattern because cargo owns clippy's output
  format; pathlint owns its own.

- **D. Publish a sixth `sarif.schema.json` release asset and
  drift-gate against it** (the pattern the five existing schemas
  use). Rejected: the five existing schemas *define* pathlint's
  own wire formats; for SARIF the definition already exists
  upstream, and shipping a subset schema with the official name
  on it invites consumers to validate third-party SARIF against
  pathlint's private profile. Golden unit tests give the same
  drift protection without the misleading artifact.

- **E. Add `--sarif` to `check` (and `doctor`) in the same
  release.** Rejected for scope: `lint` produces findings-shaped
  output (one diagnostic per problem), which is what SARIF models.
  `check` produces expectation-outcome rows (including `ok` rows)
  — mapping those onto SARIF results requires deciding what an
  "ok" is in SARIF terms, a design question with no driving use
  case yet. `doctor` selfcheck findings are about the host, not
  the repository, and have even less claim to a code-scanning
  surface. Either can be added additively later.

## Consequences

- **Positive.** pathlint findings land in GitHub Code Scanning
  next to clippy / cargo-audit with one `upload-sarif` step and no
  converter. The full pipeline is
  `pathlint lint --sarif > pathlint.sarif` + one action step.
- **Positive.** Zero new dependencies; the lib gains one additive
  `pub fn` (`format::doctor_sarif`) and nothing else — the
  public-API freeze streak continues.
- **Negative.** The SARIF wire is now a second stability surface.
  The kind names were already frozen by the JSON schema; SARIF
  adds dashboards as a consumer class, which makes renames
  strictly more expensive. This is deliberate — the ids were
  never going to be renameable cheaply once `--json` shipped.
- **Negative.** The hand-rolled subset means new SARIF features
  (fingerprints for alert de-duplication, `partialFingerprints`,
  `suppressions`) require touching pathlint code rather than
  bumping a dependency. Accepted: each lands additively when a
  consumer asks.
- **Neutral.** Anchoring alerts at `pathlint.toml` means every
  finding annotates line 1 of the config rather than the "real"
  location — because there is no real file location for an env
  var. If GitHub ever grows a location-less alert surface, the
  anchor can be dropped additively.
- **Follow-up.** The schemastore.org registration promised by
  ADR-0031 remains open and is unaffected by this ADR.

## Related ADRs

- **ADR-0031** (SARIF + schemastore as the integration points) —
  the commitment this ADR implements; its Status carries the
  implementation note.
- **ADR-0016** (JSON wire shape, `kind` discriminator) — froze the
  kind names this ADR now re-exports as SARIF rule ids.
- **ADR-0012** (schemars 1.0 deferred) — the dependency-adoption
  posture applied here to `serde-sarif`.
