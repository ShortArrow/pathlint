# ADR-0002: `PathEntry::from_raw` takes a closure

- **Status**: Accepted
- **Date**: 2026-05-10
- **Release**: 0.0.23
- **Category**: 3. Cross-cutting concern (env injection policy) — also touches Category 1 because the constructor signature changed

## Context

ADR-0001 introduced `PathEntry::from_raw(raw)`, which internally
called `expand::expand_env(&raw)`. `expand_env` read
`std::env::var` directly. That meant:

1. Tests building a `PathEntry` directly inherited whatever the
   test runner's `PATH`/`HOME`/`%LocalAppData%` happened to be.
   A `$HOME/bin` entry expanded against the developer's real
   `HOME`, not a deterministic stub. Tests that hit the `expand`
   path were silently flaky across machines.
2. Lib embedders had no way to substitute a different env oracle.
   The 9 modules listed in ARCHITECTURE.md already took closures
   for filesystem and env lookups (e.g. `doctor::analyze` takes
   `fs_exists` and `env_lookup` closures); `PathEntry::from_raw`
   was the only construction path that bypassed that pattern.

The 0.0.23 PR that introduced `PathEntry::from_raw(raw)` shipped
just before this discrepancy was caught in codex review. The fix
landed in the same release rather than waiting for 0.0.24.

## Decision

Change `PathEntry::from_raw` to take an env-lookup closure:

```rust
pub fn from_raw<V>(raw: impl Into<String>, env_lookup: V) -> Self
where
    V: Fn(&str) -> Option<String>,
{
    let raw = raw.into();
    let expanded = expand::expand_env_with(&raw, &env_lookup);
    Self { raw, expanded, provenance_raw: None }
}
```

The closure flows through a new public function
`expand::expand_env_with(input, env_lookup)`. The existing
`expand::expand_env(input)` becomes a thin wrapper:

```rust
pub fn expand_env(input: &str) -> String {
    expand_env_with(input, |v| std::env::var(v).ok())
}
```

Production callers (`path_source::read_path`,
`resolve::split_path`) inject `|v| std::env::var(v).ok()`
explicitly so the env-reading boundary is visible in the call
site. Tests inject deterministic closures.

## Alternatives considered

- **Keep `from_raw(raw)` as-is and add `from_raw_with_env(raw,
  env_lookup)` as a second constructor.** Rejected because lib
  embedders would never know which one to call. The whole point
  of forcing the choice into the constructor signature is to make
  the env oracle a parameter of every `PathEntry` construction,
  not a default that varies by call site.
- **A global trait object (`set_env_oracle(Box<dyn Fn(...)>)`).**
  Rejected: globals make tests order-dependent and fight against
  the rest of pathlint's "pass deps explicitly" pattern.
- **A `PathEntry` builder pattern.** Rejected: `PathEntry` has at
  most three fields and there is no construction-time validation
  to spread across multiple steps.

## Consequences

- **Positive.** Every `PathEntry::from_raw` call site states its
  env oracle explicitly. `path_source::read_path` and
  `resolve::split_path` are the only two places in the lib that
  inject the live process env — every other call site uses a
  deterministic closure.
- **Positive.** `expand::expand_env_with` is independently useful
  for lib embedders that want env-aware string expansion without
  constructing a full `PathEntry`. It's pinned on the public API
  surface in `tests/public_api.rs`.
- **Negative.** This is the second BREAKING release in the 0.0.23
  cycle (the first introduced `from_raw(raw)`; this one changed
  it to `from_raw(raw, env_lookup)`). Within the working branch
  the change is visible as two commits; the squash merge to main
  collapses them, so the on-crates-io history shows one change.
- **Follow-up.** `resolve::split_path` and `source_match::find`
  still read `std::env::var` directly (the latter via
  `expand_and_normalize` → `expand_env`). Step 2 of the
  0.0.25-0.1.0 roadmap finishes the closure-injection rollout by
  giving those entry points the same `_with` variants.
