# ADR-0016: JSON wire shape — every union uses a top-level `kind` discriminator, and `required` arrays in `*.schema.json` reflect `skip_serializing_if`

- **Status**: Accepted
- **Date**: 2026-05-05 (decision); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.14 (`trace --json` discriminator), 0.0.15 (`check --json` discriminator), 0.0.17 (`check.schema.json` `required` honesty)
- **Category**: 7. Persistence / data format

## Context

pathlint emits structured JSON from four subcommands:
`check --json`, `doctor --json`, `trace --json`, `sort --json`,
plus the catalog views (`catalog list --json`,
`catalog relations --json`). Each emits a small union type:
- `check --json` distinguishes between OK / NG outcomes and
  between NG sub-kinds (mismatched source, missing source,
  not-executable shape, etc.).
- `doctor --json` distinguishes between detector kinds
  (duplicate, missing, conflict, ...).
- `trace --json` distinguishes between found / not-found.
- `sort --json` distinguishes between note kinds.

Pre-0.0.14 each subcommand had drifted to its own wire shape:

- `trace --json` used `{ "found": true, "path": "...", ... }`
  with a top-level boolean to discriminate. Consumers had to
  branch on `if (json.found) { ... } else { ... }`.
- `check --json` used `{ "status": "ok" }` /
  `{ "status": "ng_not_executable", "reason": "..." }` /
  externally-tagged
  `{ "status": { "ng_not_executable": "..." } }` for the
  payload-carrying variants. Consumers had to handle both
  shapes (string and object) under the same `status` key.
- `doctor --json` and `sort --json` already used a top-level
  `kind` field (Serde internally-tagged enum), serving as
  the precedent.

Three problems:

1. **Inconsistent discrimination**. JSON consumers reading
   both `check` and `trace` had to switch reasoning between
   `kind`-tagged and `found`-tagged union handling. There was
   no policy to point at when adding a new subcommand's JSON
   shape.

2. **Payload-carrying enum variants leaked into wire shape**.
   `Status::NgNotExecutable(String)` rendered as
   `{ "status": { "ng_not_executable": "..." } }` (externally
   tagged) — consumers couldn't branch on the discriminator
   as a flat string without a fallback case for the
   payload-carrying variants.

3. **Schemas drifted from runtime**.
   `check.schema.json`'s `required` array listed `prefer`,
   `avoid`, `reason`, `diagnosis`, `resolved` as required
   fields, but the runtime applied
   `#[serde(skip_serializing_if = "...")]` to all five — the
   schema declared a contract the binary did not honour. JSON
   validators that trusted the schema would reject valid
   pathlint output.

The three problems were addressed across 0.0.14 (`trace`
discriminator), 0.0.15 (`check` discriminator + 0.0.17 (schema
`required` honesty), but the underlying policy is one
decision: **every union has a top-level `kind` and schemas
reflect what the runtime actually emits**.

## Decision

**Policy**: every JSON union pathlint emits uses Serde
internally-tagged enums with `tag = "kind"`, rendered as a
top-level `kind` field plus the variant's payload flattened
into the same object. `*.schema.json` files declare `required`
exactly for the fields the runtime always serialises;
`skip_serializing_if` fields are listed in `properties` but
not in `required`.

Concrete realisations:

- **0.0.14** — `trace --json` switches from `{ "found": bool, ... }`
  to `{ "kind": "found" | "not_found", ... }`. JSON
  consumers migrate from `if (json.found)` to
  `if (json.kind === "found")`.

- **0.0.15** — `check --json` outcome array elements
  switch from `{ "status": "ok" }` /
  `{ "status": { "ng_not_executable": "..." } }` to
  `{ "kind": "ok" }` / `{ "kind": "ng_not_executable", "reason": "..." }`.
  The `Status` enum stays internally tagged via Serde.

- **0.0.17** (separate but related) — `Status` enum becomes
  unit-variant only; payload moves to `Outcome::reason:
  Option<String>` (see ADR-0018 for the type design). The
  wire form becomes
  `{ "kind": "ng_not_executable", "reason": "..." }` flat
  rather than `{ "kind": { "ng_not_executable": "..." } }`
  externally-tagged. ADR-0018 is the *type* decision;
  this ADR records the *wire* decision they jointly
  satisfy.

- **0.0.17** — `check.schema.json`'s `required` array
  drops `prefer`, `avoid`, `reason`, `diagnosis`,
  `resolved`. The schema now matches what
  `serde_json::to_string_pretty` actually emits for an
  arbitrary `CheckOutcomeView`.

The five generator binaries (`gen_schema`,
`gen_check_schema`, `gen_doctor_schema`,
`gen_trace_schema`, `gen_sort_schema`) re-run on every CI
build and `assert_eq!` against the checked-in
`schemas/*.schema.json` files — any future derive change
that breaks the policy fails the drift gate.

## Alternatives considered

- **A. Use Serde externally-tagged enums uniformly
  (the default).** Rejected because externally-tagged
  enums produce `{ "kind": { "variant": "payload" } }`
  for payload-carrying variants — a nested object where
  the consumer must reach into `json.kind` to get the
  discriminator string. Internally-tagged
  (`#[serde(tag = "kind")]`) produces a flat
  `{ "kind": "variant", "payload_field": "..." }` which
  is simpler to consume.

- **B. Keep per-subcommand discriminator choice
  (boolean for binary unions, string for n-ary).**
  Rejected because the line between "binary" and
  "n-ary" is fluid (`trace`'s `found` / `not_found`
  could plausibly grow a third variant for
  "found but ambiguous"), and consumers benefit from
  one consistent pattern across all subcommands.

- **C. Use a separate `type` or `tag` field instead of
  `kind`.** Rejected because `kind` was already the
  precedent in `doctor --json` and `sort --json`; the
  policy made the consistent choice.

- **D. Mark every potentially-emitted field as
  `required` in `*.schema.json` and emit empty
  strings / nulls for the suppressed ones.**
  Rejected because pathlint deliberately uses
  `skip_serializing_if` to keep wire output compact;
  reversing that to fit the schema would bloat output
  by ~30% (the suppressed fields are frequent — most
  `Outcome` values have no `reason`, no `diagnosis`,
  etc.) and would create distinct `null` / empty-string
  / missing-field cases for consumers to handle.

- **E. Drop schemas entirely; let consumers infer from
  examples.** Rejected because pathlint ships schemas
  as GitHub Release assets at stable URLs; downstream
  tools (an editor plugin, a CI gate) consume them
  programmatically. Removing schemas would break those
  consumers.

## Consequences

- **Positive.** One policy for JSON consumers: read
  `kind` as a string, branch on it. No fallback for
  payload-carrying variants. The handler shape
  collapses to a single `switch` / `match` per
  subcommand.

- **Positive.** Schemas are honest. JSON validators
  that consume pathlint's `*.schema.json` files don't
  reject valid pathlint output. New consumer tooling
  can trust the schema without empirical verification.

- **Positive.** The drift gate (`tests/*_schema.rs`)
  catches future derive changes that would break the
  policy — if someone added an `#[serde(tag = "type")]`
  on a new enum, the generated schema would diverge from
  the checked-in file and CI would fail.

- **Negative.** Three releases of BREAKING in three
  months for consumers (0.0.14 `trace`, 0.0.15
  `check`, 0.0.17 schema `required`). Each release's
  CHANGELOG carried a migration note; consumers
  tracking pathlint at all closely had a sequence of
  small per-release migrations rather than a single big
  one.

- **Negative.** The policy is implicit in the code
  (every `#[derive(Serialize, JsonSchema)]` enum uses
  `#[serde(tag = "kind", rename_all = "snake_case")]`).
  A new contributor adding a new enum could miss the
  pattern; no `clippy` lint enforces it. The drift gate
  catches *schema* drift but not *policy* drift (a new
  union with externally-tagged shape would be allowed
  if its schema was checked in correctly). Risk
  mitigated by code review and by this ADR being
  citable.

- **Follow-up.** None. The policy has held through
  0.0.17-0.0.31 without new wire shapes diverging;
  every new enum added (e.g. `SortNote` kinds in
  0.0.x baseline, `Diagnostic` kinds in 0.0.x baseline,
  `UninstallHint` kinds in 0.0.18) inherits the policy
  by following the existing derive pattern.
