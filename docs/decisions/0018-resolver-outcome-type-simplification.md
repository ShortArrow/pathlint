# ADR-0018: resolver and outcome type simplifications — `Option<PathBuf>` and unit-variant `Status` with `Outcome::reason`

- **Status**: Accepted
- **Date**: 2026-05-05 (decisions in 0.0.16 and 0.0.17); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.16 (`Resolution` removed) → 0.0.17 (`Status` unit-only + `Outcome::reason`)
- **Category**: 1. Public API surface (resolver closure shape and `pathlint::lint` outcome shape)

## Context

Two near-adjacent type simplifications shipped in 0.0.16 and
0.0.17. They are recorded together because they share the
same underlying theme: **one-field newtypes carry their
"discriminator-ness" in the type name, not in invariants
the type enforces, and conflate the JSON wire form's needs
with the Rust API's needs**.

### Resolver: `Resolution { full_path }` (0.0.16)

Pre-0.0.16, the resolver closure that `pathlint::lint::evaluate`
and `pathlint::trace::locate` took had the shape:

```rust
fn(&str) -> Option<Resolution>

pub struct Resolution {
    pub full_path: PathBuf,
}
```

The single-field newtype existed only to label the value as
"this is a resolution result, not just any PathBuf". It
carried no invariants the bare `PathBuf` did not, no
methods, and no field expansion in plan. Embedders writing
their own resolver closures had to construct
`Some(Resolution { full_path: pb })` for every match; the
extra type added noise without leverage.

### Status payload (0.0.17)

Pre-0.0.17, `pathlint::lint::Status` had two payload-carrying
variants:

```rust
pub enum Status {
    Ok,
    Ng { ... },
    NgNotExecutable(String),  // <-- carried its own reason
    ConfigError(String),      // <-- carried its own reason
}
```

The two `String` payloads were the human-readable explanation
of why the outcome was that variant. The JSON serialisation
(see ADR-0016 for the wire-shape policy) rendered these as
externally-tagged objects:

```json
{ "kind": { "ng_not_executable": "..." } }
```

JSON consumers had to handle a discriminator that was
sometimes a string and sometimes an object — a fallback case
in every `kind`-branching switch.

A separate but parallel field on `Outcome` (the wrapper that
combines `Status` with other per-outcome metadata) could
absorb the reason without changing the `Status` discriminator:

```rust
pub struct Outcome {
    pub status: Status,
    pub reason: Option<String>,
    ...
}
```

The wire form becomes:

```json
{ "kind": "ng_not_executable", "reason": "..." }
```

flat and consistently discriminated.

## Decision

Adopt both simplifications, bundled here because they share
the "one-field-newtype-is-noise" theme.

### `Resolution` removed (0.0.16)

The resolver closure type becomes:

```rust
fn(&str) -> Option<PathBuf>
```

`pathlint::resolve::resolve` likewise returns
`Option<PathBuf>`. Embedders that built their own resolver
closures drop the wrapper:
`Some(Resolution { full_path: pb })` → `Some(pb)`. The
`Resolution` type is removed from the public surface.

### `Status` unit-only + `Outcome::reason` (0.0.17)

`Status` becomes unit-variant only:

```rust
pub enum Status {
    Ok,
    Ng,
    NgNotExecutable,
    ConfigError,
}
```

`Outcome` gains `reason: Option<String>`; the wire form
emits `kind` + `reason` flat. JSON consumers branch on
`outcome.kind` as a string without any object-case fallback.

## Alternatives considered

### For `Resolution`

- **A. Keep `Resolution` as a newtype to preserve type-level
  signalling.** Rejected because the newtype carried no
  invariants; a `PathBuf` returned from the resolver is just
  a `PathBuf`. Signalling in the type name only adds
  cognitive load when the embedder writes the closure.

- **B. Add real fields to `Resolution`
  (`is_executable: bool`, `resolved_via_alias: bool`).**
  Rejected because those concerns belong to the detector
  layer (`pathlint::doctor`) and the matcher
  (`pathlint::source_match`), not to the resolver. The
  resolver's job is to map a command name to a path on
  disk; nothing more.

### For `Status` / `Outcome::reason`

- **C. Keep payload-carrying `Status` variants and add
  JSON consumer fallback documentation.** Rejected
  because consumers consistently asked for a flat
  discriminator; the externally-tagged shape was a
  source of confusion. Documenting around it would have
  preserved the cognitive cost.

- **D. Move `reason` into each NG variant's payload as a
  named field
  (`NgNotExecutable { reason: String }`).** Rejected
  because the externally-tagged JSON shape would still
  be
  `{ "kind": { "ng_not_executable": { "reason": "..." } } }`
  — even more nested than before, with `kind` carrying
  an object that wraps another object.

- **E. Use internally-tagged + flatten on the existing
  payload-carrying variants.** Rejected because Serde's
  `#[serde(tag = "kind")]` on an enum with newtype
  variants doesn't compose cleanly (the payload's
  scalar `String` has nowhere to go in the flattened
  form). The unit-variant rewrite is the cleaner
  approach: the discriminator is always the variant
  name, the reason is always on `Outcome`.

- **F. Drop `reason` entirely and let consumers infer
  from the discriminator.** Rejected because the
  reason carries context the discriminator alone cannot
  (which specific file path failed the executable check,
  which TOML field caused the config error). The
  context is the whole reason the variant exists.

## Consequences

- **Positive.** Resolver closures collapse from
  `Some(Resolution { full_path: pb })` to `Some(pb)` —
  one fewer type to import, one fewer struct literal
  to construct. The resolver closure becomes the same
  shape an embedder would naturally write.

- **Positive.** JSON consumers branch on
  `outcome.kind` as a string in every case. No
  fallback. The schema (`check.schema.json`) declares
  `kind` as a string enum and `reason` as
  `Option<String>` with `skip_serializing_if`, matching
  what the runtime emits (see ADR-0016 for the schema
  honesty side).

- **Positive.** The change set realigns Rust types and
  JSON wire forms: every union has a flat discriminator
  on both sides. The mental model is "Status enum →
  variant name → JSON `kind` string"; no encoding
  trick in between.

- **Negative.** Embedders that built their own resolver
  closures and pre-allocated `Resolution { full_path: ... }`
  literals (a small set, since the type was new in
  0.0.x baseline) must do the mechanical rewrite. The
  CHANGELOG 0.0.16 entry calls this out.

- **Negative.** Embedders pattern-matching on
  `Status::NgNotExecutable(reason)` lose the `reason`
  destructuring at the match site; they must read
  `outcome.reason` separately. The CHANGELOG 0.0.17
  entry provides migration guidance: the
  `outcome.reason` is populated whenever the
  corresponding payload variant used to carry it, so
  the value is preserved, only the access pattern
  changes.

- **Negative.** JSON consumers that branched on
  `status` as a discriminator (some pre-0.0.15 / 0.0.17
  consumers may have had complex fallback logic) now
  branch on `kind` per ADR-0016 *and* find the reason
  in a flat `reason` field. The two BREAKING releases
  in close succession (0.0.16 resolver, 0.0.17 Status)
  asked consumers to migrate twice in two months;
  acceptable for pre-0.0.x users but worth noting.

- **Follow-up.** The pattern ("one-field newtype is
  noise; payload-carrying enum variants are JSON-shape
  hostile") generalises. Future PRs introducing a new
  type or enum should default to multi-field structs
  with named fields and unit-only enums with adjacent
  metadata fields; this ADR is the citation for
  rejecting the alternatives.
