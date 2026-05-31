# ADR-0015: `Provenance::WrapperInstaller` generalises from mise-only naming

- **Status**: Accepted
- **Date**: 2026-05-05 (decision); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.14
- **Category**: 1. Public API surface (the variant is on the `pathlint::trace::Provenance` type and on `trace --json` wire output)

## Context

0.0.5 introduced the plugin-attribution heuristic for `pathlint
trace`: when a binary's resolved path matches a `[[relation]]
kind = "served_by_via"` shape (typically mise's plugin layout
where `~/.local/share/mise/installs/<plugin>/<version>/bin`
hides binaries from third-party tools), pathlint reports the
upstream installer rather than the catalog source.

The original 0.0.5 variant was named **`Provenance::MiseInstallerPlugin`**.
It worked because mise was the only wrapper installer in the
catalog at the time (asdf, rtx, proto, volta had been
considered but not yet shipped with `served_by_via` relations).

By 0.0.14 the situation had shifted:

- asdf had been added to the catalog (and shipped with its own
  `served_by_via` relation pointing at
  `~/.asdf/installs/<plugin>/<version>/bin`).
- volta's `shims` directory followed a similar wrapper pattern.
- The R4 (provenance) PRD section explicitly listed "wrapper
  installer" as a class, not mise-specific.

The variant name had locked the type to one tool. Every
asdf-installed binary that `trace` resolved through asdf's
plugin path would either trip an
`expected MiseInstallerPlugin, got X` assertion in tests, or
require a parallel variant
(`Provenance::AsdfInstallerPlugin`), or have to be coerced
into the mise-named variant — none acceptable.

The 0.0.14 cut was already shipping several rename-heavy
breaking changes (catalog source names — see ADR-0014; CLI
aliases — see ADR-0019; JSON discriminator unification — see
ADR-0016). Bundling this rename into the same cut keeps the
migration cost concentrated.

## Decision

Rename the variant from `Provenance::MiseInstallerPlugin` to
**`Provenance::WrapperInstaller`**. Field shape and computation
unchanged:

```rust
pub enum Provenance {
    WrapperInstaller {
        installer: String,      // upstream tool name from installer_token
        plugin_segment: String, // raw path segment
    },
}
```

Wire format (`trace --json` `provenance.kind`):
`mise_installer_plugin` → `wrapper_installer` (serde
`rename_all = "snake_case"` derives the wire name from the
variant name).

No aliases on the variant itself (Rust's enum has no alias
mechanism). JSON consumers that pattern-match on
`provenance.kind == "mise_installer_plugin"` must migrate to
`"wrapper_installer"`.

## Alternatives considered

- **A. Keep `MiseInstallerPlugin` and add a parallel
  `AsdfInstallerPlugin` variant.** Rejected because every
  future wrapper installer (rtx, proto, volta-shims, …) would
  demand its own variant; pattern-match arms on `Provenance`
  would proliferate; `trace::locate` would need a lookup
  table mapping installer token to variant constructor. The
  variant should describe the *role* (wrapper installer), not
  the *brand* (mise vs asdf vs rtx).

- **B. Use a generic `Provenance::Other(String)` variant.**
  Rejected because pathlint's R4 contract is *typed*
  provenance, not free-form strings. Downstream code matching
  on the variant (`trace --explain` rendering,
  `Conflict` detector) needs to know "this is a wrapper
  installer specifically" to print the right context. A
  `String` payload would erase the discriminator and force
  every caller back into string matching.

- **C. Stay mise-only and refuse to add asdf / rtx / etc to
  the catalog.** Rejected because pathlint's catalog is
  explicitly multi-installer (cargo, mise, volta, winget,
  choco, scoop, brew, apt, pacman, …); declining asdf would
  reverse pathlint's R4 stance ("a single TOML covers every
  installer").

- **D. Rename to `Provenance::PluginInstaller` instead of
  `WrapperInstaller`.** Rejected because "plugin" is mise's
  own term for its provider abstraction; asdf calls them
  "plugins" too, but volta calls them "shims", winget calls
  its installer scripts something else again. "Wrapper"
  captures the structural commonality (a binary that delegates
  to a versioned, isolated upstream copy) without privileging
  any one tool's terminology.

## Consequences

- **Positive.** New wrapper installers (asdf, rtx, proto,
  volta, scoop hooks) reuse the variant without code changes;
  the `installer` string carries the actual tool name in each
  case. The catalog adds new `[[relation]] kind = "served_by_via"`
  entries; the lib doesn't add new variants.

- **Positive.** JSON consumers that branch on
  `provenance.kind` get a stable discriminator (`wrapper_installer`)
  that does not change every time a new wrapper installer is
  added; the `installer` field carries the variation.

- **Positive.** The variant name aligns with the `[[relation]]
  kind = "served_by_via"` semantics; both describe "this
  source serves binaries on behalf of another via a plugin
  layer". Naming consistency between the catalog wire format
  and the runtime variant.

- **Negative.** Embedders pinned to the 0.0.5-0.0.13
  `MiseInstallerPlugin` variant name must rewrite. The cost
  is mechanical (one rename per match-arm); the variant's
  field shape is unchanged.

- **Negative.** JSON consumers branching on
  `provenance.kind == "mise_installer_plugin"` get
  `"wrapper_installer"` instead. CHANGELOG 0.0.14 announces
  this under `### Breaking`. The same release is shipping
  several other rename-heavy breakings; consumers updating
  one are likely updating all.

- **Follow-up.** None. The variant has stayed
  `WrapperInstaller` through 0.0.14-0.0.31 without further
  reshape; asdf's `served_by_via` relation (added in a
  later release) consumed the rename's benefit without
  needing additional enum work.
