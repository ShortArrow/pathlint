# ADR-0028: `doctor` is pathlint's selfcheck; PATH analysis moves to a new `lint` subcommand

- **Status**: Proposed
- **Date**: 2026-06-07
- **Release**: 0.0.34
- **Category**: 1. Public lib/CLI surface (+8. CLI subcommand topology)

## Context

Round 1 dogfooding (ShortArrow/dotfiles PR #3, merged 2026-06-03)
captured 202 diagnostics from `pathlint doctor --json` on a real
Windows host (102 PATH entries). The first user observation upon
reading the snapshot was:

> doctor が 見るべき は pathlint が 動作可能 か という 観点。
> pathlint 自身 の 責務 と pathlint doctor の 責務 が
> かぶって は 意味 が ない。

Reading the current implementation against that statement reveals
a responsibility tangle:

- `pathlint check` runs `[[expect]]` rules from `pathlint.toml`
  against the live PATH. Its responsibility is **"PATH matches
  the user's declared intent"**.
- `pathlint doctor` runs 12 detector kinds (`duplicate_but_shadowed`,
  `shortenable`, `writeable_path_dir`, `missing`, `trailing_slash`,
  `conflict`, `relative_path_entry`, `mise_activate_both`,
  `case_variant`, `short_name`, `malformed`,
  `per_source_missing_required`) and surfaces them as `severity:
  warn` JSON. Its responsibility is **"PATH has anomalies the
  user might want to know about"**.
- Both responsibilities answer the same shape of question:
  *"does this PATH look right?"* They differ only in *where the
  expectation comes from* — `[[expect]]` rules (user-declared)
  vs detector heuristics (catalog-derived).

The name `doctor` does not match its current behaviour. In UNIX
convention (`brew doctor`, `flutter doctor`, `rustup doctor`)
the verb means **"check the tool itself is healthy in this
environment"** — binary on PATH, config readable, dependencies
present. pathlint's `doctor` does none of those; it inspects the
*user's* PATH, not pathlint's own working environment.

The Round 1 snapshot also exposed a UX symptom of this confusion:
101 of the 202 diagnostics were `duplicate_but_shadowed`, dominated
by Windows OS-stub shadowing (WindowsApps→WinGet, system32→Git
usr/bin) that is *correct by design*. The user's reaction was to
ask whether design-intent-aware noise filtering belongs in doctor.
But filtering "PATH anomalies the user wants to know about" is
the lint problem. doctor (in the proper sense) should not surface
them at all.

A separate user request — *"正常 ケース も 表示 (cargo dev OK)"* —
was raised in the same session. On reflection it is a **lint** UX
concern (showing "expectation met" for each `[[expect]]` rule or
catalog-derived precedence), not a doctor concern. That request is
recorded as Round 3 candidate; it is not part of this ADR.

## Decision

Split the responsibilities into three subcommands, each answering
a single question:

| Subcommand | Question answered |
|---|---|
| `pathlint doctor` | **Is pathlint itself functional in this environment?** (selfcheck) |
| `pathlint check` | **Does the live PATH satisfy the user's `[[expect]]` rules?** (unchanged) |
| `pathlint lint` | **Does the live PATH have catalog-derived anomalies?** (new — inherits 0.0.33 doctor's PATH-anomaly detector kinds; `pathlint.toml` semantic validation is Round 3 follow-up) |

### What `pathlint doctor` does (0.0.34 onward)

Three checks only:

1. **Binary self-locate.** Resolve the running pathlint binary
   against PATH. Warn if it is absent from PATH (running by
   absolute path), or if more than one `pathlint` resolves on PATH
   (the running one is *not* the first match).
2. **`pathlint.toml` discovery and parse.** Walk from cwd upward
   to find a `pathlint.toml`. Report not-found as info (legitimate
   case: user runs `pathlint` without a config), and parse failure
   as error. **Semantic validity (does `[source.x] path` exist?
   does `[[expect]] command` match a catalog entry?) is not
   checked here** — that is `pathlint lint`'s responsibility.
3. **`env_lookup` operational.** Verify `PATH` is readable; on
   Windows also `PATHEXT`; verify `HOME` (Unix) or `USERPROFILE`
   (Windows) is readable. Report each missing variable as error;
   they are pathlint's hard dependencies (ADR-0027).

Output: JSON top-level **array** of selfcheck diagnostics, each
with `severity` in {`error`, `warn`, `info`} and `kind` in a small
enum (proposed: `binary_not_in_path`, `binary_shadowed`,
`config_not_found`, `config_parse_error`, `env_lookup_failed`).
The array shape matches `pathlint lint --json` for consumer
uniformity.

### What `pathlint lint` does (new in 0.0.34)

PATH anomaly detection — the 12 detector kinds currently living
in `src/doctor.rs::analyze`. The kind enum is preserved verbatim;
the move is a CLI relocation, not a redesign. The underlying
`Diagnostic` Rust type and `analyze()` API are unchanged.

`pathlint.toml` semantic validation (verifying that each
`[source.x]` override resolves on this host, that each
`[[expect]] command` is known to the merged catalog, etc.) is
**scoped out of 0.0.34** and recorded as Round 3 follow-up.
0.0.34 ships the responsibility split; semantic validation is
additive and does not block the split.

The `lint` output JSON shape mirrors the 0.0.33 `doctor` output:
top-level **array** of diagnostics, each element with `severity`,
`kind`, plus kind-specific fields. doctor and lint share the
same `Diagnostic` Rust type and emit through the same
`schemas/doctor.schema.json` — the 0.0.34 schema additively
grows by the 4 selfcheck kind variants (`binary_not_in_path`,
`config_parse_error`, `config_not_found`, `env_lookup_failed`)
on top of the 12 lint variants. Consumers migrating from
`pathlint doctor --json` (12-kind output) to `pathlint lint --json`
(same 12 kinds) only change the subcommand name — the parser
is the same. A future ADR may split into `lint.schema.json`
if the kind sets diverge; 0.0.34 keeps the schema single to
minimise migration surface.

### What `pathlint check` does (unchanged)

`pathlint check` continues to evaluate `[[expect]]` rules against
the live PATH. Its output schema (`schemas/check.schema.json`,
the 8-status enum `ok` / `ng_wrong_source` / etc) is preserved
verbatim. **No change to check's behaviour or schema in 0.0.34.**

### Migration: no alias runway

0.0.34 is a direct BREAKING release. The previous `pathlint
doctor --json` 12-kind output is replaced by selfcheck output
(4 kinds) in the same release. The 12 lint kinds remain emittable
by the new `pathlint lint --json` subcommand (same `Diagnostic`
schema additively grown by the 4 selfcheck kinds). There is no
`--legacy` flag, no transition window for the subcommand split.

Reasoning (recorded so a future maintainer can re-evaluate):

- **No observed consumers.** The 0.0.33 release is the first to
  ship JSON schemas as Release assets. We have no evidence of
  downstream CI integrations consuming `doctor --json`. (Search
  of crates.io reverse-deps: zero. Search of GitHub for
  `pathlint doctor --json`: no hits beyond the pathlint repo
  itself and the dotfiles snapshot.)
- **0.0.x license.** SemVer ADR-0005 explicitly treats each
  0.0.x→0.0.(x+1) as MAJOR-equivalent. BREAKING is permitted by
  the version contract.
- **Simpler code.** A `--legacy` flag would require keeping the
  old `analyze()` callable from `execute_doctor()` with a divergent
  output path. The lint module would then have two callers (lint
  and legacy-doctor) and a kind enum that varies by entry point.
  The migration window cost outweighs the migration window
  benefit, given no observed consumers.
- **Streak reset is accepted.** The additive-only streak from
  0.0.18 through 0.0.33 (16 releases, per ADR-0025) resets at
  0.0.34. The user explicitly accepted this trade in plan
  approval; the responsibility split is the load-bearing change.

If a future BREAKING release wants an alias runway (e.g. removing
a Category 1 lib type), ADR-0019's where→trace pattern remains
the template. This ADR does not invalidate ADR-0019; it records
*why this particular* BREAKING did not need the runway.

### Backwards compatibility surface

- **CLI:** `pathlint doctor` continues to be a valid subcommand;
  its behaviour changes. Scripts calling `pathlint doctor` without
  parsing the JSON output (e.g. exit-status-only) continue to
  work — selfcheck exits 0 on healthy, non-zero on error, same
  contract as current doctor.
- **JSON:** scripts parsing `pathlint doctor --json` for specific
  `kind` values will break. CHANGELOG 0.0.34 entry will document
  the migration: `pathlint doctor` → `pathlint lint`.
- **Catalog:** unchanged.
- **`pathlint.toml`:** unchanged. Semantic validation is *additive*
  diagnostics; existing valid configs still parse and run.
- **Other subcommands** (`check`, `sort`, `trace`, `catalog`,
  `init`): unchanged.

### Module shape

- `src/doctor.rs` shrinks to selfcheck-only (~150 LOC).
- The 12 detector kinds plus pathlint.toml semantic validation
  move to a new module (`src/lint_detector.rs` or extend
  `src/lint.rs`; chosen during implementation).
- `src/lint.rs::evaluate` (the `[[expect]]` evaluator backing
  `pathlint check`) is unchanged.
- `src/config.rs` gains a `validate_semantic` function (or
  similar) that takes a `Config` and a `Catalog` and returns
  validation diagnostics; called only by lint, not by parse.

## Alternatives considered

- **A. Keep `doctor` as-is; add `lint` as a new command;
  document the overlap.** Rejected. The user's framing was that
  responsibility overlap is the *defect*. Adding a new command
  without resolving doctor's identity preserves the confusion.
  The diagnostic load (202 entries on a healthy host) was the
  symptom; the diagnosis is "doctor is doing lint's job".

- **B. Rename `doctor` to `lint`; do not introduce a separate
  selfcheck command.** Rejected. UNIX `doctor` convention is
  established; pathlint should *have* a selfcheck command, and
  `doctor` is the natural name. Renaming `doctor` to `lint` and
  leaving selfcheck unimplemented optimises for a smaller
  diff at the cost of leaving the actual problem (pathlint has
  no way to verify itself in a broken env) unaddressed.

- **C. Alias runway: `pathlint doctor --legacy` for one release.**
  Rejected for the reasons listed under *Migration: no alias
  runway* above (no observed consumers, 0.0.x BREAKING license,
  code simplicity). This alternative would otherwise be the
  ADR-0019 template; the reasoning is specifically why *this*
  BREAKING does not warrant it.

- **D. Fold all three responsibilities into `pathlint check`;
  delete `doctor`.** Rejected. The three responsibilities answer
  different questions (selfcheck vs declared-intent vs
  catalog-anomaly). Folding them obscures which question is
  being asked when the command runs. The check/lint split also
  matches `cargo check` vs `cargo clippy` precedent in the
  Rust ecosystem: check enforces the declared contract; lint
  surfaces heuristic concerns.

- **E. Replace `doctor` JSON output with text-only "ok / not ok"
  health status; drop `--json` for doctor entirely.** Rejected.
  Selfcheck output is small but consumers (CI integrations
  verifying pathlint is functional in a deploy environment) want
  structured output. The right answer is a *small* JSON envelope
  with a focused enum, not abandoning JSON.

- **F. Move PATH anomaly detection to `pathlint check` (so
  check does both `[[expect]]` and detector kinds); leave
  `doctor` for selfcheck.** Rejected. `[[expect]]` evaluation
  is the *user's declared intent*; detector kinds are *catalog-
  derived heuristics*. Mixing them in check makes
  `pathlint check` ambiguous: did this rule fail because the
  user wrote a wrong `[[expect]]` or because a detector heuristic
  fired? Keeping them in separate subcommands keeps each
  command answering one question.

## Consequences

- **Positive.** Each subcommand answers one question. A user
  reading `pathlint --help` can immediately tell which
  command to run: "is pathlint working?" → doctor; "does my
  PATH match what I declared?" → check; "is my PATH sensible
  and is my config valid against the catalog?" → lint.

- **Positive.** Selfcheck capability now exists. CI integrations
  deploying pathlint can run `pathlint doctor && pathlint
  check` as a pre-flight sequence; previously this required
  manually inspecting `pathlint --version` and the parse
  status of `pathlint.toml`.

- **Positive.** The 101-`duplicate_but_shadowed`-on-Windows
  noise problem becomes a *lint UX* problem with a clear
  surface (aggregation, severity tuning, `--exclude` support)
  rather than entangled with doctor's identity. Round 3
  candidates (severity=ok, `[[precedence]]` relation, per-pair
  aggregation) all become coherent lint enhancements.

- **Positive.** ADR-0019's alias runway pattern is preserved
  for future BREAKING that warrants it. This ADR explicitly
  documents *why this particular* BREAKING does not.

- **Negative.** The additive-only streak from 0.0.18 (16
  releases, per ADR-0025) resets at 0.0.34. The user
  accepted this trade; the streak restarts at 0.0.35+.

- **Negative.** `pathlint doctor --json` output schema changes
  in a way no migration shim covers. CI integrations parsing
  specific `kind` values (none observed, but possible) must
  switch to `pathlint lint`. CHANGELOG 0.0.34 will document
  the migration with explicit before/after JSON examples.

- **Negative.** Round 1's `windows-pwsh-2026-06-03.json`
  snapshot (taken with 0.0.33 doctor) is not directly
  comparable to Round 2's `windows-pwsh-lint-YYYY-MM-DD.json`
  (taken with 0.0.34 lint). The dotfiles readme will
  document the relationship; both snapshots remain in git
  history.

- **Negative.** The PRD §3 R3 description must be updated to
  reflect the doctor/lint split. Failure to update PRD breaks
  graduation criterion "PRD matches implementation". The
  CHANGELOG 0.0.34 entry will list the PRD edit explicitly so
  it cannot be skipped.

- **Follow-up.** Round 3 will revisit (with 0.0.34 lint
  output in hand) whether `[[precedence]]` relations,
  `severity=ok` for satisfied checks, or per-pair shadow
  aggregation should be added. None of those are committed
  by this ADR.

- **Follow-up.** If Round 3+ observes that lint output is
  high-volume enough to need filtering, an `--exclude` or
  `--include` flag would extend lint without revisiting this
  ADR. The selfcheck/lint boundary established here is
  load-bearing; the lint output shape is not.
