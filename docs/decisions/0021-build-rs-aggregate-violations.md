# ADR-0021: `build.rs` aggregates plugin referential-integrity violations into one failure

- **Status**: Accepted
- **Date**: 2026-05-05 (decision); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.14
- **Category**: 8. Process / governance (build-time validation reporting)

## Context

The catalog is built into the binary at compile time:
`build.rs` reads `plugins/_index.toml` to learn the plugin
order, parses each `plugins/<name>.toml` against
`PluginFile` (a build-script-local mirror of the runtime
`PluginFileShape`), and concatenates them into
`OUT_DIR/embedded_catalog.toml` for `src/catalog.rs` to
`include_str!`.

A plugin file can declare `[[relation]]` entries that point at
other catalog sources by name (e.g. `mise_shims` declares
`served_by_via` toward `cargo`). Referential integrity says
that every name in any relation must resolve to a
`[source.<name>]` defined by some plugin file. If a plugin
typos a target (`served_by_via_cargo`) or references a source
that was removed (`served_by_via_winget` after winget rename),
the catalog will load at runtime with a dangling relation that
no detector can act on.

Two failure-reporting strategies:

- **Fail-fast**: stop at the first violation; tell the user
  which one plugin to fix.
- **Aggregate**: walk every plugin, collect every violation,
  report them all in one panic.

Pre-0.0.14 used fail-fast. The PR introducing the catalog had
two plugins with mistyped relation names; fixing the first
exposed the second only after another rebuild, then a third
only after another rebuild, etc. Each rebuild took ~30s
because `build.rs` runs every time `plugins/` changes. The
catalog at the time had ~10 plugins; expanding to the current
24 plugins (plus user-defined ones via
`merge_with_user_relations`) would have multiplied the iteration
cost.

## Decision

`build.rs` collects **every** referential-integrity violation
across all plugins in a single pass, then emits one panic
listing all violations. The relevant code path (lines 76-83
of `build.rs` as of 0.0.32):

```rust
if let Err(violations) = check_referential_integrity(&plugins) {
    panic!(
        "plugin catalog failed referential integrity ({} violations):\n  - {}\n\n\
         every relation must point at a source defined by some plugin file",
        violations.len(),
        violations.join("\n  - ")
    );
}
```

`check_referential_integrity` walks every plugin, every
relation, every source-name reference; it returns
`Err(Vec<String>)` of human-readable violation messages rather
than `Err(String)` of the first one encountered.

The same policy applies to shape-check failures (a plugin
that fails `toml::from_str::<PluginFile>`) — those still
fail-fast per-plugin because the shape error message already
tells the user which line to look at, and a malformed plugin
can't contribute to referential integrity until it parses.

## Alternatives considered

- **A. Keep fail-fast (one violation per build cycle).**
  Rejected because the iteration cost is O(violations × build
  time). With 24 plugins and N relations per plugin, the
  user might have ~5–10 violations after a refactor (e.g.
  renaming a source touches every plugin that references
  it); each requires a full rebuild to see the next.
  Aggregation costs nothing extra at the build step (the
  walk is already O(N)) and reduces user iteration to one
  cycle.

- **B. Aggregate within a plugin, fail-fast across plugins.**
  Rejected as inconsistent. A user fixing relation typos
  doesn't care which file boundary the typos straddle; the
  goal is "show me everything that's broken so I can fix it
  all in one editor pass". Splitting reporting at plugin
  boundaries forces the user to think about file boundaries
  when reasoning about the catalog.

- **C. Emit warnings (not errors) for missing references.**
  Rejected because a dangling `served_by_via` relation has
  no runtime semantics — `trace` would silently fail to
  attribute the binary, `Conflict` would silently skip the
  check, with no log line because relations are walked
  lazily. Warnings would mask catalog bugs in production
  binaries; errors at build time catch them at the only
  point where the catalog can be edited.

- **D. Make this opt-in via a `--check-integrity` flag on a
  separate `cargo run --bin catalog-check` binary.**
  Rejected because the catalog is *embedded* at build time;
  any check that doesn't run during `cargo build` doesn't
  prevent shipping a broken binary. Build-time validation is
  the only point where the embedding can be vetoed.

## Consequences

- **Positive.** Refactors that rename catalog sources (most
  recently 0.0.14's `system_*` → `os_baseline_*` per
  ADR-0014) get caught in one build cycle: every relation
  pointing at the old name shows up in the same panic
  message. The user fixes all references at once.

- **Positive.** New plugin authors get a complete dependency
  list of what their plugin references but is missing,
  rather than discovering missing references one at a time.

- **Positive.** CI build failures carry useful diagnostic
  text (the violation list) rather than just "build failed
  on plugin X" — the developer reading the CI log can scope
  the fix immediately.

- **Negative.** The panic message length scales with
  violation count; a catalog refactor producing 50
  violations would produce a 50-line panic. Tolerable: panic
  output is a terminal scrollback concern, not a runtime
  one.

- **Negative.** Aggregation forces the walk to continue past
  the first violation, doing extra work on a broken
  catalog. The work is bounded (the catalog has ~24 plugins
  with ~50 total relations; the walk is O(relations × sources)
  with sources being a `BTreeSet` lookup); the overhead is
  unmeasurable compared to the panic message formatting.

- **Follow-up.** None. The aggregation policy has held
  through 0.0.14-0.0.31 without need for revisiting; new
  plugins integrate via the same shape-check + integrity
  check pipeline.
