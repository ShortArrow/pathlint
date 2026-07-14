# ADR-0031: pathlint integrates via SARIF + json-schema-store, not via an LSP server

- **Status**: Accepted. Update (0.0.42): the SARIF output mode
  shipped as `pathlint lint --sarif` — see ADR-0034 for the
  dependency choice (hand-rolled emit-only structs, no new crate),
  the ruleId contract, and the drift-gating strategy. The
  schemastore.org registration remains open.
- **Date**: 2026-06-16
- **Release**: 0.0.38 (decision); implementation deferred to 0.0.40 or later
- **Category**: 7. Persistence / data format (+ 6. External dependency)

## Context

pathlint is a stateless CLI: it reads `PATH` (and optionally
`pathlint.toml`), evaluates expectations and detectors, and
prints JSON or human output. There is no document model, no
incremental edit stream, and no long-lived process state. The
host integration tests, the container e2e (ADR-0030), and the
five `schemars`-generated JSON schemas (ADR-0016) already cover
"is the output machine-readable and stable". What is not
covered is "how do other tools consume it" — the ecosystem
contract.

Adjacent projects (`eslint`, `clippy`, `shellcheck`,
`cargo-audit`, `terraform validate`) have converged on two
standards for that role:

1. **SARIF 2.1.0** (OASIS Static Analysis Results Interchange
   Format) — the wire format GitHub Code Scanning, Azure DevOps
   Advanced Security, SonarCloud / SonarQube, and most enterprise
   static-analysis aggregators consume. `cargo clippy
   --message-format=json` plus `clippy-sarif` is the canonical
   Rust example; ESLint emits SARIF natively via `eslint -f sarif`.
2. **schemastore.org** — the catalog of JSON Schema definitions
   editors consult to validate / autocomplete config files
   without per-extension boilerplate. Taplo (TOML LSP), JetBrains
   IDEs, VS Code's built-in JSON / YAML language servers, and
   Helix's lsp-config all auto-discover schemas from
   `schemastore.org` keyed by filename. Adding `pathlint.toml`
   there means every editor that already supports Taplo / JSON
   Schema validates the file without extra setup on the user's
   side.

Neither of these is an LSP. The question "should pathlint speak
LSP?" comes up because LSP is the lingua franca of language
tools, and contributors familiar with the rust-analyzer /
typescript-language-server pattern reach for it first. The
answer for pathlint is **no**: LSP's value proposition is the
textDocument/didChange stream (incremental analysis of an
edit-in-progress buffer). pathlint analyses a process's `PATH`
environment variable, which is not a document, has no edit
positions, and changes on the order of "once per shell session",
not "once per keystroke". An LSP server for pathlint would
implement an empty document model, a no-op didChange, and a
"diagnostics" endpoint that wraps the same `lint --json` output
SARIF would carry — adding a server lifecycle, a JSON-RPC
transport, and a long-running process for no incremental gain.

The standardization layers pathlint actually needs are:

- **Editor experience on `pathlint.toml`**: autocomplete +
  inline validation when authoring rules. *Solved by Taplo +
  the `#:schema` directive today; widened by schemastore
  registration so it works without the directive.*
- **CI / Code Scanning surface**: the `lint` output should show
  up as PR annotations and feed enterprise SAST aggregators.
  *Solved by emitting SARIF.*
- **Generic log ingestion** (Cloudflare Logpush, Loki, Datadog,
  Splunk, ELK): a stream of one finding per JSONL line. *Already
  solved by piping `pathlint lint --json` through `jq -c '.[]'`
  — no new output mode needed; documenting the recipe in the
  README is enough.*

The decision below records that pathlint commits to SARIF +
schemastore as its integration points, declines to ship an LSP
server or a bespoke RPC, and treats the JSONL-via-`jq` recipe
as the answer for stream consumers.

## Decision

pathlint integrates with surrounding tooling through three
layers, in increasing order of standardization weight:

1. **`pathlint lint --json` / `doctor --json` / `trace --json` /
   `check --json` / `sort --json`** — the existing stable JSON
   wire format, schema-pinned and drift-tested
   (`schemars`-generated `pathlint.schema.json`,
   `check.schema.json`, `doctor.schema.json`, `trace.schema.json`,
   `sort.schema.json`; ADR-0016). This stays the foundational
   layer. Adding a one-line JSONL recipe (`pathlint lint --json |
   jq -c '.[]'`) to the README covers log-ingestion consumers
   (Cloudflare Logpush, Loki, Datadog, Splunk, ELK) without
   adding an output mode.

2. **SARIF 2.1.0 output mode** (`pathlint lint --sarif`) —
   deferred to 0.0.40 or later. Targets GitHub Code Scanning,
   Azure DevOps Advanced Security, SonarCloud, and the
   `clippy-sarif` / `eslint -f sarif` ecosystem of converters and
   aggregators. Implemented as a separate output mode rather than
   a converter so the SARIF rule metadata (id, name, full
   description, help URI) stays under pathlint's control and
   ships at the same version as the detector that emits it. The
   `sarif` Rust crate (currently `sarif = "0.3"` or its
   successor) is the candidate dependency. A separate ADR
   recording the dependency choice will accompany the
   implementation PR.

3. **schemastore.org registration of `pathlint.toml`** —
   deferred to 0.0.40 or later. One PR to
   `SchemaStore/schemastore` adding a catalog entry that maps
   `pathlint.toml` (and any `*.pathlint.toml` convention) to the
   `pathlint.schema.json` hosted on
   `https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/pathlint.schema.json`.
   After merge, every editor that consults schemastore picks up
   validation + completion automatically; the existing
   `#:schema` directive remains supported for users who pin to a
   specific tag.

pathlint **does not** ship:

- An LSP server. The cost is a JSON-RPC transport, a server
  lifecycle, and a long-running process for no incremental gain
  over the CLI; the underlying analysis is not document-shaped.
- A bespoke RPC protocol ("pathlint-rpc", a daemon mode, a
  socket interface). Existing JSON output through stdout covers
  every observed need; introducing a new protocol asks every
  consumer to learn it.
- An ESLint plugin. ESLint analyses source files; pathlint
  analyses an environment variable. The lifecycles do not
  compose.
- A VS Code extension. Taplo + the `#:schema` directive +
  schemastore covers the editor case for `pathlint.toml`. An
  extension would re-implement what Taplo already does.

The schemastore PR and the SARIF output mode are independent
work items; either can ship before the other.

## Alternatives considered

- **A. Build a pathlint LSP server.** Rejected. LSP's primitives
  (textDocument/didChange, hover, completion at position,
  go-to-definition) presuppose a document being edited. pathlint
  has no document — its input is `std::env::var("PATH")` plus an
  optional `pathlint.toml`. An LSP wrapper would forward
  diagnostics that the CLI already prints, behind a JSON-RPC
  transport and a server lifecycle. The cost is real (process
  supervision, RPC contract, version skew between the LSP server
  and the user's `pathlint` binary); the value is zero over a
  CLI that already emits JSON. The decision to skip this is
  reversible if the future brings a use case for hot-reloading
  `pathlint.toml` diagnostics inside an editor without saving
  the file — but at that point Taplo's already-existing schema
  validation covers most of it, and a thin "pathlint check on
  save" file watcher would close the rest with less code than a
  language server.

- **B. Define a bespoke pathlint-RPC protocol.** Rejected. The
  appeal is "we control the wire format end-to-end"; the cost
  is asking every consumer (CI runners, log shippers,
  aggregators) to learn a new schema with no documentation
  beyond pathlint's own README. The same end-to-end control is
  available via JSON schemas published per-release (already
  shipped) plus SARIF for the static-analysis ecosystem and JSONL
  for log shippers.

- **C. Ship an ESLint plugin / pre-commit plugin / etc as the
  primary integration.** Rejected as a *primary*. Plugin
  ecosystems integrate with a specific tool's runtime
  (pre-commit's `pre-commit-hooks.yaml`, eslint-plugin-* for
  ESLint's config); they do not standardize the output for
  *other* tools. A pre-commit recipe in the README is fine and
  costs one PR; making it the standardization layer is not.

- **D. OpenTelemetry / OTLP exporter as the standardization
  layer.** Tempting because OTLP is broadly supported by
  Cloudflare Workers Logs, Datadog, Honeycomb, Tempo, etc.
  Rejected because OTLP is built around *spans* and *metrics* —
  request traces and time-series — neither of which fits a
  pathlint run, which is a single batch of findings at a point
  in time. A SARIF report or a JSONL stream maps more naturally;
  OTLP would require shoehorning findings into events with
  fabricated trace ids. If a pathlint-as-daemon mode ever
  materializes (which Alternative A also rejected) and starts
  producing spans for individual detector evaluations, OTLP
  becomes worth revisiting.

- **E. Wait for someone else to define the standard.** Rejected.
  SARIF, json-schema-store, and the JSONL-stream pattern are
  already established standards with broad tooling support; the
  marginal effort is "produce SARIF / register one PR / document
  one jq recipe", not "design something new". Waiting just
  delays the integration value.

- **F. Do nothing; rely on `--json` only.** Acceptable today but
  loses ground over time. Code Scanning has become the default
  surface for security findings in PR workflows; an analyser
  that does not speak SARIF is invisible in that surface. The
  marginal cost of `--sarif` is low (one output mode, one
  dependency, one ADR recording the dependency choice) and the
  marginal benefit is "pathlint findings appear in the same
  surface as clippy / cargo-audit / dependabot".

## Consequences

- **Positive.** The integration story is recorded and bounded.
  Future contributors asking "shouldn't pathlint be an LSP?"
  find their answer here, not in an ad-hoc Slack thread or in a
  rejected PR. The same applies to "shouldn't pathlint emit
  OTLP?" / "shouldn't pathlint have a daemon mode?".

- **Positive.** Implementation can be paced over multiple
  releases. 0.0.38 ships the ADR + the JSONL-via-jq recipe in
  the README, costing essentially nothing. The SARIF output
  mode and the schemastore PR can land independently in 0.0.40
  or beyond, each in a single self-contained release.

- **Positive.** Selecting SARIF aligns pathlint with `clippy`,
  `cargo-audit`, `cargo-deny`, `eslint`, `shellcheck`, and the
  broader `*-sarif` converter ecosystem. Users running pathlint
  alongside those tools in GitHub Actions get a unified surface
  in Code Scanning (one set of PR annotations, one finding tab).

- **Positive.** The JSONL recipe in the README is essentially
  free (one block of bash + jq, no new code), and addresses the
  Cloudflare Logpush / Loki / Datadog use cases without adding
  output modes pathlint then has to keep stable forever. If a
  consumer's needs outgrow `--json | jq -c '.[]'` later,
  evidence in hand justifies a `--jsonl` mode.

- **Negative.** Committing to SARIF brings a `sarif` crate
  dependency at 0.0.40 (or whichever release ships it). That
  crate's API is currently 0.x and may churn; the
  schemars-1.0-deferred argument from ADR-0012 applies here too,
  modulo "the SARIF crate ecosystem is smaller than the
  schemars ecosystem". A separate ADR will record the
  dependency choice when the work lands, including which crate
  version, which 2.1.0 features are implemented, and how
  pathlint detector kinds map onto SARIF rules.

- **Negative.** schemastore registration concentrates a small
  amount of trust in the upstream `SchemaStore/schemastore` repo
  — they are now part of pathlint's user-onboarding path. The
  worst-case failure is "the schemastore catalog is unreachable,
  editor falls back to no validation"; the editor still works,
  just without inline validation, and `#:schema` users are
  unaffected. Risk is small and reversible (delete the
  schemastore entry, fall back to the directive-only path).

- **Negative.** Choosing not to ship an LSP closes the door on
  one specific feature: live (unsaved-buffer) validation of
  `pathlint.toml` *while the user types*. The user has to save
  the file for Taplo + the schema to validate it. This is
  acceptable: `pathlint.toml` is not a file edited keystroke by
  keystroke; users edit it occasionally and run pathlint. If a
  use case materializes that genuinely needs unsaved-buffer
  validation, the right addition is a thin file-watcher invoking
  the CLI, not a full LSP server.

- **Follow-up.** When the SARIF mode lands, write a separate ADR
  recording: which crate version was picked, how detector kinds
  map onto SARIF rules (including stable `rule.id` values that
  cannot be renumbered without breaking consumer dashboards),
  and how the SARIF mode's output is drift-gated against the
  existing JSON schemas. When the schemastore PR lands, link to
  it from this ADR's Status section so the implementation
  history is traceable.
