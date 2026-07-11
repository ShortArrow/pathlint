# Architecture

A 5-minute repo map for new contributors and future maintainers.
Pairs with [PRD.md](PRD.md) (the why) and [RELEASE.md](RELEASE.md)
(the how-to-cut-a-release).

## TL;DR

`pathlint` is a TOML-driven PATH linter shipped as both a CLI binary
(`pathlint`) and an embeddable Rust library (`pathlint::*`). The
binary reads `pathlint.toml`, evaluates each `[[expect]]` rule
against the running PATH, and reports OK / NG / skip. The library
exposes the same evaluation pipeline as a typed API so external
tools can embed it.

## Repo map

```
src/                    library + binary source
  lib.rs                  public API surface (10 supported modules)
  bin/pathlint/           binary entry point + CLI plumbing
    main.rs               main() — clap parse, dispatch, exit code
    cli.rs                clap derive structs (Cli, GlobalOpts, ...)
    run.rs                orchestration per subcommand
plugins/                  built-in source catalog (one file per group)
  _index.toml             catalog version + plugin order
  cargo.toml, mise.toml…  per-installer source definitions + relations
schemas/                  checked-in JSON schemas (5 — drift-gated)
  pathlint.schema.json    user pathlint.toml shape
  check.schema.json       pathlint check --json shape
  doctor.schema.json      pathlint doctor --json shape
  trace.schema.json       pathlint trace --json shape
  sort.schema.json        pathlint sort --json shape
docs/                     documentation
  PRD.md / PRD.jp.md      product requirements + design rationale
  RELEASE.md / RELEASE.jp.md  release runbook
  README.jp.md            Japanese README
  ARCHITECTURE.md         this file
scripts/
  bench.sh                hyperfine startup-time wrapper (PRD §12)
tests/                    integration tests (one per concern)
.github/workflows/        ci.yml (PR gates) + release.yml (tag/build/publish)
build.rs                  embed plugins/*.toml into the binary at build time
Cargo.toml                crate metadata + [[bin]] declaration
README.md                 user-facing entry point
```

## Library: 10 public modules

`src/lib.rs` declares exactly ten `pub mod` entries — that is the
supported library surface. `tests/public_api.rs` pins them by
import + a callability check; moving or renaming a listed symbol
fails CI.

| Module | Role | Headline symbols |
|---|---|---|
| `config` | `pathlint.toml` schema | `Config`, `Expectation`, `SourceDef`, `Relation`, `Severity`, `Kind` |
| `lint` | core PATH evaluation | `evaluate`, `evaluate_real`, `EvaluateDeps`, `exit_code`, `Outcome`, `Status`, `Diagnosis`, `CheckOutcomeView` |
| `trace` | provenance lookup | `locate`, `locate_real`, `LocateDeps`, `TraceOutcome`, `Found`, `Provenance`, `UninstallHint` |
| `sort` | PATH repair proposals | `sort_path`, `sort_path_real`, `SortDeps`, `SortPlan`, `EntryMove`, `SortNote` |
| `doctor` | PATH hygiene | `analyze`, `analyze_real`, `AnalyzeDeps`, `fs_list_dir_real`, `is_writable_dir_real`, `Diagnostic`, `Filter`, `Kind`, `Severity` |
| `catalog` | built-in source catalog | `builtin`, `builtin_relations`, `merge_with_user`, `merge_with_user_relations`, `check_acyclic`, `version_check`, `embedded_version`, `RelationIndex` |
| `source_match` | path → source matching | `find`, `names_only`, `validate_sources`, `Match`, `SourceWarning` |
| `os_detect` | runtime OS dispatch | `Os`, `os_filter_applies` |
| `expand` | env-var expansion + slash normalisation | `expand_env`, `expand_env_with`, `normalize`, `expand_and_normalize` |
| `path_entry` | PATH entry raw/expanded carrier (0.0.23+, purified in 0.0.28) | `PathEntry`, `PathEntry::from_raw` |

The crate root also exposes:

- `pathlint::CommonDeps` (0.0.27+) — shared env-oracle carrier
  embedded in `AnalyzeDeps` / `EvaluateDeps` / `LocateDeps` /
  `SortDeps`.
- `pathlint::Attribution` (0.0.28+) — cross-source carrier
  wrapping a `PathEntry` together with an optional
  `provenance_raw`. Every entry-list parameter on the lib's
  public surface (`analyze`, `evaluate_real`, etc.) takes
  `&[Attribution]`.

The crate-level rustdoc (`src/lib.rs`) carries the authoritative
list with examples. Embedders should treat docs.rs as the
contract; everything not mentioned there is internal.

## Internal modules

Everything outside the ten listed above is **not** part of the
public contract.

| Module | Visibility | Role |
|---|---|---|
| `format` | `#[doc(hidden)] pub` | human + JSON renderers (consumed by the binary) |
| `report` | `#[doc(hidden)] pub` | check report (multi-line OK/NG with `--explain`) |
| `init` | `#[doc(hidden)] pub` | starter `pathlint.toml` template |
| `path_source` | `#[doc(hidden)] pub` | per-target PATH reader (process / Windows registry) |
| `resolve` | `#[doc(hidden)] pub` | `which`-style command resolver against PATH |
| `catalog_view` | `#[doc(hidden)] pub` | `catalog list` rendering |
| `shell_quote` | `pub(crate)` | POSIX / PowerShell single-quote escapes (used by `trace` uninstall hints) |

The `#[doc(hidden)] pub` modules are reachable from
`src/bin/pathlint/` because Cargo treats the binary as a separate
crate. They are intentionally **not** re-exported on docs.rs and
not part of the supported lib API surface — see `src/lib.rs`
crate-level docstring.

## Build pipeline

`build.rs` runs once per build:

1. Reads `plugins/_index.toml` to learn the plugin order +
   `catalog_version`.
2. Validates every `plugins/<name>.toml` against
   `PluginFileShape` (a build-time clone of the runtime
   `pathlint::catalog::PluginFileShape`).
3. Concatenates the validated plugin files into one
   `OUT_DIR/embedded_catalog.toml` blob, prefixed with the
   `catalog_version = N` line.
4. The lib reads that blob via `include_str!` at compile time and
   parses it through `EmbeddedCatalogFile`. User `pathlint.toml`
   uses a separate `Config` type that **rejects** `catalog_version`
   structurally — only the embedded blob carries it.

Adding a new built-in source:

1. Create `plugins/<name>.toml` with `[source.<name>]` +
   description.
2. Add `<name>` to `plugins/_index.toml`'s `plugins = [...]`.
3. `cargo build` validates the shape via `build.rs`.
4. `cargo test --test plugin_validation` is a runtime second
   gate against the same shape.
5. Bump `catalog_version` in `_index.toml` only when the change
   modifies an existing source's path or semantics. Adding a
   brand-new source name does not require a bump.

## Schema generators + drift gates

Five binaries under `src/bin/`:

| Binary | Source type | Output schema |
|---|---|---|
| `gen_schema` | `pathlint::config::Config` | `schemas/pathlint.schema.json` |
| `gen_check_schema` | `pathlint::lint::CheckOutcomeView` | `schemas/check.schema.json` |
| `gen_doctor_schema` | `pathlint::doctor::Diagnostic` | `schemas/doctor.schema.json` |
| `gen_trace_schema` | `pathlint::trace::TraceJsonOutput` | `schemas/trace.schema.json` |
| `gen_sort_schema` | `pathlint::sort::SortPlan` | `schemas/sort.schema.json` |

Each generator runs `schemars::schema_for!(Type)` and prints
pretty JSON. The corresponding drift test (`tests/*_schema.rs`)
re-runs the generator and `assert_eq!`s against the checked-in
file — any change to the underlying `#[derive(JsonSchema)]` type
fails until the schema is regenerated and committed.

`release.yml` runs the same generators in the publish-github
job, so the GitHub Release assets always match what the binary
at that tag would emit.

## Test layout

`tests/` has 17 integration tests + lib unit tests. Notable
gates:

| Test | What it pins |
|---|---|
| `tests/public_api.rs` | ten-module surface, with callability checks (not just `use`) |
| `tests/help_contract.rs` | `--version` / `--help` output (subcommand list, alias visibility) |
| `tests/{schema,check_schema,doctor_schema,sort_schema,trace_schema}.rs` | schemas/ files match the generators byte-for-byte |
| `tests/plugin_validation.rs` | every `plugins/*.toml` parses against runtime `PluginFileShape` and is `catalog_version`-free |
| `tests/security.rs` | hostile `pathlint.toml` cases (TOCTOU symlinks, oversized files, root-needle sources, relation cycles, `catalog_version` reject) |
| `tests/cli_global_options.rs` | `--color` and `--no-glyphs` route through every renderer |
| `tests/cli_strings.rs` | canonical `--config` diagnostics (alias `--rules` removed in 0.0.22) |

CI runs `cargo test` on Ubuntu + macOS + Windows. The
`fmt + clippy` job is its own gate (`cargo fmt --check` +
`cargo clippy --all-targets -- -D warnings`).

## Release pipeline

`.github/workflows/release.yml` triggers on pushing a `vX.Y.Z`
tag to `main` (0.0.36+). The version bump rides in a normal PR
before the tag is pushed; CI never writes to `main`:

1. **guard**: asserts that `Cargo.toml` at the tagged commit
   reads `version = "X.Y.Z"` — catches a tag pushed against the
   wrong commit before any expensive job runs.
2. **build × 4 archs**: `x86_64-unknown-linux-gnu`,
   `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`,
   `aarch64-apple-darwin`. Each produces an archive uploaded as
   an artifact.
3. **publish-github**: regenerate the 5 schemas from the tagged
   commit, build SHA256SUMS, create the GitHub Release with the
   archives + 5 `*.schema.json` files attached.
4. **publish-gate → publish-crates** *(opt-out)*: the gate scans
   the tagged commit's message for a standalone `[skip publish]`
   line; unless present, an OIDC-trusted `cargo publish` runs.
   The default is to publish.

Every release is `prerelease = true` while the tag prefix is
`v0.` (set explicitly by `release.yml`). Schema asset URLs are
stable: `https://github.com/ShortArrow/pathlint/releases/download/v<tag>/<name>.schema.json`.

See [RELEASE.md](RELEASE.md) for the human-side checklist
(schema-pin update, EN/JP parity, optional bench run).

## Where to start reading

- New contributor: this file → `src/lib.rs` crate-level docstring
  → the module of interest's rustdoc.
- New user: `README.md` → `docs/PRD.md §6 user stories` →
  `docs/PRD.md §7 functional requirements`.
- Designing a change: `docs/PRD.md` (read why before how) →
  `tests/` (find the gate that pins the contract you'd touch) →
  `src/`.
