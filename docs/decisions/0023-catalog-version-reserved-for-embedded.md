# ADR-0023: `catalog_version` is reserved for the embedded catalog; user `pathlint.toml` rejects it

- **Status**: Accepted
- **Date**: 2026-05-05 (post-parse error in 0.0.14, structural in 0.0.15); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.14 (post-parse rejection) → 0.0.15 (promoted to structural / `deny_unknown_fields` parse error)
- **Category**: 7. Persistence / data format (reserved-field policy)

## Context

pathlint ships a built-in source catalog embedded at compile
time via `include_str!` (see `build.rs` and ADR-0021). The
embedded blob carries one piece of metadata users can pin
against: `catalog_version = N`. The version bumps whenever an
existing built-in source changes path or semantics; user
`pathlint.toml` files can declare `require_catalog = N` to
fail loudly when the binary's embedded catalog is older than
the user expects (e.g. a dotfiles repo sharing
`pathlint.toml` across machines where each machine might have
a different pathlint version installed).

The 0.0.14 PR that introduced `catalog_version` left a hole:
user `pathlint.toml` could *also* set
`catalog_version = N`, and pathlint silently accepted it.
That created two ways to express what should be one
concept:

- The embedded `catalog_version` (authoritative for what the
  binary actually has).
- A user-declared `catalog_version` (no semantics; pathlint
  did nothing with it).

Worse, the absence of behaviour on the user-declared value
invited drift: a user who declared `catalog_version = 7`
because they thought it would request catalog v7 would get
no error and no warning; their pin would silently fail to
do anything.

The 0.0.14 cut shipped a post-parse rejection (the value was
read during `Config` parsing and then surfaced as an exit-2
config error). The 0.0.15 cut promoted this to a structural
rejection via serde's `deny_unknown_fields`: a user
`pathlint.toml` containing `catalog_version = 7` fails at
the TOML parse step before any `Config`-level logic runs.

## Decision

`catalog_version` is **reserved** for the embedded catalog
file (`OUT_DIR/embedded_catalog.toml`, produced by
`build.rs`). User `pathlint.toml` files **must not** declare
it.

Implementation:

- The `Config` struct (the user-TOML deserialisation target)
  has `#[serde(deny_unknown_fields)]` (0.0.15+); the
  embedded-catalog deserialisation uses a separate type
  (`EmbeddedCatalogFile`) that does accept `catalog_version`.
- A user `pathlint.toml` containing `catalog_version = N`
  fails parsing with serde's standard "unknown field"
  message; pathlint exits 2 (config error).
- The `require_catalog = N` field on the user side is the
  channel for pinning against a minimum embedded version;
  it has structural meaning (range check at config-load
  time) where `catalog_version` on the user side has none.

User migration: rename `catalog_version` to `require_catalog`
in user files (the spelling difference is small but the
semantic difference matters; the structural rejection forces
the user to re-read the docs and pick the right one).

## Alternatives considered

- **A. Accept `catalog_version` in user TOML as an alias for
  `require_catalog`.** Rejected because aliasing one
  reserved name to another changes a user-declared value's
  semantics, which violates the
  "reserved means reserved" promise. A user typing
  `catalog_version = 7` expecting one thing should not
  silently get another thing.

- **B. Keep the post-parse rejection (0.0.14's original form)
  rather than promoting to structural in 0.0.15.** Rejected
  because post-parse rejection means the error message
  shows up after partial config interpretation; the user
  doesn't immediately see "this field can't be here" at the
  field's line. Structural rejection via
  `deny_unknown_fields` pinpoints the offending key with
  serde's standard error format.

- **C. Silently ignore user-declared `catalog_version`
  (no-op).** Rejected because silence is the worst failure
  mode for pin-style fields: the user thinks they have a
  pin, they don't, and a configuration drift across
  machines goes undetected until it bites in production.

- **D. Move `catalog_version` from the embedded catalog into
  user TOML entirely.** Rejected because the catalog
  version describes the *embedded* state — it's a property
  of the binary, not of the user's intent. Putting it in
  user TOML would force users to declare the binary's
  version, which they don't necessarily know.

- **E. Don't have `catalog_version` at all (assume catalog
  is monotonically additive).** Rejected because some
  catalog changes (path renames, source removals) are
  inherently breaking for users who pinned to a previous
  set of source names; the `catalog_version` + user-side
  `require_catalog` pair gives those users a fail-fast
  channel.

## Consequences

- **Positive.** User TOML and embedded catalog have
  cleanly separated schemas: `Config` (user) declares
  `[source.X]`, `[[expect]]`, `[[relation]]`, and
  `require_catalog = N`; `EmbeddedCatalogFile` (embedded)
  declares `catalog_version = N` and the rest of the
  catalog. No field overlap, no shared parsing path.

- **Positive.** A user who tries to pin against the
  catalog with the wrong field name gets a structural
  error at TOML parse time. Migration cost is one
  rename (`catalog_version` → `require_catalog`).

- **Positive.** The reserved-field policy generalises: if
  future fields need to live only in the embedded catalog
  (e.g. a `built_at = "<timestamp>"` for audit purposes),
  they can be added to `EmbeddedCatalogFile` without
  worrying about user TOML accepting them.

- **Negative.** The promotion from post-parse error
  (0.0.14) to structural error (0.0.15) is itself a
  BREAKING change for any user who had wired around the
  0.0.14 post-parse error message format (e.g. greping
  stderr for the specific 0.0.14 error string). The
  one-release gap was deliberately small to limit the
  number of users affected; the 0.0.14 release notes
  explicitly mention `catalog_version` rejection and the
  0.0.15 release notes describe the promotion.

- **Negative.** The `Config` type's
  `deny_unknown_fields` is now a load-bearing attribute:
  removing it would silently re-accept `catalog_version`
  and any future reserved fields. A regression test
  (`config::tests::parse_toml_rejects_catalog_version_via_deny_unknown_fields`)
  pins the behaviour; removing the attribute fails the
  test.

- **Follow-up.** None. The reserved-field policy has held
  through 0.0.15-0.0.31 without further reshape; the
  `require_catalog` / `catalog_version` split is now the
  established way to pin against catalog state.
