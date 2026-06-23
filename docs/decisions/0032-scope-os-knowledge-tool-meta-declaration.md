# ADR-0032: pathlint scope — OS knowledge + tool-meta declaration, no tool behavior

- **Status**: Accepted
- **Date**: 2026-06-23
- **Release**: 0.0.39
- **Category**: 5. Architectural style (+7. Persistence / data format, +8. Process / governance)

## Context

pathlint has grown from a 0.0.2 path-prefix matcher into a 0.0.38 release with
33 built-in catalog sources (across `os_baseline_*`, packaging-manager scopes,
and wrapper-installer plugins), 5 `[[relation]]` kinds, two distinct
subcommands operating on the merged catalog (`lint`, `trace`), and a
`doctor` selfcheck. The growth has been entirely additive on the catalog
side — every new source ships as a `plugins/<name>.toml` file (path
patterns, per-OS expansion, uninstall command template) without a Rust
code change. ADR-0014 captured the naming side of that policy
(`<provenance>_<scope>` + `os_baseline_*` split); ADR-0015 captured the
provenance side (one `WrapperInstaller` variant covers mise / asdf /
volta / future entries without re-naming the enum).

What the existing ADR set does **not** capture is the policy *behind*
those decisions: that pathlint commits to knowing how OSes lay out PATH,
and commits to declaring where each tool *says* its binaries land, but
explicitly refuses to model what those tools actually *do* at runtime
(version selection, shim resolution, plugin install order, current-state
queries). The constraint shows up in five places already:

- **ADR-0009** (read-only stance) — pathlint never mutates host state.
  Knowing tool behavior would tempt callers into "now fix it for me",
  which is the line that ADR-0009 holds.
- **ADR-0015** (`Provenance::WrapperInstaller` generalisation) — the
  `installer` *string* carries the tool name; the Rust enum is one
  variant for every wrapper installer past and future. New tools enter
  via `[[relation]]` in TOML, not via a new code path.
- **ADR-0022** (`depends_on` is descriptive-only) — the relation kind
  has no runtime effect. It documents intent for human readers; pathlint
  does not act on it.
- **ADR-0023** (`catalog_version` reserved for the embedded catalog) —
  the user TOML cannot fork the catalog's identity; the catalog is one
  authoritative declaration.
- **ADR-0031** (SARIF + schemastore as integration points, no LSP / no
  RPC) — pathlint's output is the contract; downstream tooling consumes
  it. There is deliberately no "plugin runtime" pathlint hosts.

PRD §4 carries this stance in prose ("No package-manager queries
(0.1.x). pathlint does not call `dpkg -S` / `rpm -qf` / `pacman -Qo` /
`brew which-formula`."). PRD §3 R4 (provenance) describes the limit
from the other side ("name the installer it most plausibly came from"
— a path-prefix inference, not a tool-state lookup).

The constraint is load-bearing — every catalog addition and every
detector decision since 0.0.3 has implicitly honoured it — and the
policy keeps coming up as adjacent questions: "should pathlint know
that mise is currently `use`ing python 3.12?", "should pathlint detect
which volta default is active?", "should pathlint warn that pyenv's
shim layer is stale?". The answer to each of those is *no*, for the
same reason, but the reason itself lives in nobody's head except the
maintainer's. A user reviewing the codebase recently described this
state as wanting "a 方針転換" — a direction change — when in fact the
direction has been constant; only the documentation was missing.

This ADR records the policy as a single anchor that future requests
can be evaluated against without reverse-engineering it from five
adjacent ADRs and two PRD paragraphs.

## Decision

pathlint operates on three layers, and the responsibility boundary
between them is the load-bearing constraint this ADR locks in:

1. **OS knowledge is first-class.** pathlint owns the model of how
   each supported OS resolves PATH. Specifically:
   - `os_baseline_*` sources (the directories the OS itself puts on
     PATH: `/usr/bin`, `/usr/sbin`, `C:\Windows\System32`, etc.;
     ADR-0014 split convention) are built-in and authoritative.
   - PATHEXT on Windows, executable-bit on Unix, case-insensitive
     matching on Windows file systems (ADR-0011) — all owned by
     pathlint.
   - The Windows registry overlay (`HKCU\Environment\Path` +
     `HKLM\...\Environment\Path`) for `--target user` / `machine`
     is owned by pathlint (ADR-0003 decode, ADR-0004 overlay
     semantics).
   - OS-level distinctions like the WSL Arch `/usr/sbin` precedence
     fix (the 0.0.14 case that motivated `os_baseline_linux_sbin`)
     live in the catalog, not in tool-specific code.

2. **Tool meta is declarative only.** For every non-OS source —
   package managers (`apt`, `dnf`, `pacman`, `winget`, `choco`,
   `scoop`, `brew_arm`, `brew_intel`, `flatpak`, `snap`, `pkg`,
   `macports`, `mingw`, `msys`), language-ecosystem installers
   (`cargo`, `npm_global`, `pip_user`, `go`, `aqua`, `volta`,
   `strawberry`), wrapper installers (`mise`, `mise_shims`,
   `mise_installs`, `asdf`), and user-scope shorthands
   (`user_bin`, `user_local_bin`, `windows_apps`,
   `termux_user_bin`) — the catalog declares:
   - A human-readable `description` and a per-OS `path` (or
     `windows` / `unix`) expansion;
   - An `uninstall_command` template (a *string* the user runs;
     pathlint never executes it);
   - Optional `[[relation]]` entries (5 kinds: `served_by_via`,
     `alias_of`, `conflicts_when_both_in_path`, `depends_on`,
     `prefer_order_over`) describing how this source relates to
     others.

   That is the entire vocabulary. pathlint **does not** know:
   - Whether mise is currently `use`ing python 3.12 vs 3.11.
   - Which asdf plugin version is the default for the current shell.
   - What `volta install node@20` would install.
   - Whether a pyenv shim points at a healthy CPython binary.
   - What `brew which-formula <bin>` would return.
   - What `dpkg -S <path>` would attribute.

   These are tool-state queries; ADR-0009 prohibits the side effects
   they would introduce, ADR-0031 directs the integration value
   they would carry into the SARIF output layer instead.

3. **"Plugin" means a catalog entry.** From a user perspective, the
   extension point pathlint exposes is *write your own
   `[source.<name>]` (and optionally `[[relation]]`) in your
   `pathlint.toml`*. The built-in catalog is a curated set of those
   same entries shipped in the binary; a user adding a custom
   `[source.my_corp_internal]` is exercising the same mechanism the
   maintainers use to ship `[source.mise]`. There is no dynamic
   plugin loader, no sidecar binary protocol, no Cargo feature flag
   for installer detection, and no LSP-style live-document
   integration (ADR-0031). The TOML schema is the plugin API.

The boundary, restated as a one-sentence rule: **pathlint knows what
the OS puts on PATH and where each tool says its files land; it does
not know what each tool currently has loaded, selected, or active.**

## Alternatives considered

- **A. Absorb tool behavior into pathlint.** Teach pathlint how mise
  resolves version selection, how asdf finds plugin shims, how volta
  picks the default node. Rejected. The cost is one Rust module per
  supported tool, each tracking that tool's release cadence — and
  the cost compounds as tools update. The benefit is "richer
  diagnostics", but those diagnostics already exist in the tools
  themselves (`mise current`, `asdf current`, `volta list`); duplicating
  them inside pathlint introduces a second source of truth that drifts.
  The harder objection is structural: querying tool state is a
  side effect (process invocation, environment introspection beyond
  PATH), which directly erodes ADR-0009's read-only posture. A
  diagnostic that depends on what a sub-process reports is no longer
  a static lint.

- **B. Dynamic plugin loader** (`.so` / `.dll` / `.wasm`). Rejected.
  ABI stabilisation alone is a multi-release commitment for a 0.0.x
  crate. Each plugin format brings its own attack surface (`dlopen`'s
  RPATH escapes, wasm sandbox escape vulnerabilities, untrusted
  shared library load order); the trust boundary documented in
  SECURITY.md and ADR-0027 would have to expand by exactly the size of
  whatever interface we expose. The benefit is "third parties can ship
  tool integrations without a pathlint release" — but the same outcome
  is available today by shipping a TOML snippet users paste into their
  config, with no ABI commitment from pathlint.

- **C. Sidecar binary plugin protocol** (`pathlint-plugin-mise` runs
  as a child process). Rejected for the same family of reasons as B
  with different specifics: sub-process discovery (where does pathlint
  look?), version skew (which protocol version does the sidecar
  speak?), and failure mode multiplication (does pathlint fail open or
  closed when the sidecar segfaults?). Each of those is a question
  that gets answered worse than "the catalog has 33 sources already
  and accepts patches". ADR-0031 also took the position that
  downstream integration should travel through SARIF, not through a
  custom child-process protocol.

- **D. Cargo feature flag plugin system** (`cargo install pathlint
  --features mise-deep`). Rejected. The feature flag mechanism is
  appropriate when the same crate's code path benefits from compile-
  time customisation (link a smaller TLS stack, drop async runtime
  support for embedded use). It is not appropriate as a "plugin"
  system: every feature-flag combination is a binary that ships
  separately and is supported separately, and users would have to
  reinstall pathlint to gain a tool integration. Catalog TOML
  addition reaches the same outcome at zero recompile cost.

- **E. Document the policy only in PRD §3 / §4** (no ADR). Rejected.
  PRD prose has the right reach for users but the wrong reach for
  contributors. PRD updates rarely cite the rejected alternatives;
  ADRs always do. ADR-0000 PA3 ("a load-bearing constraint that
  future-me would reverse without remembering why") and PA5 ("an
  anchor for a rejected request") both apply directly — the request
  for "should pathlint know mise's current selection?" has come up
  enough that the rejection deserves a citable home. A PRD paragraph
  alone leaves the rejection un-grounded.

## Consequences

- **Positive.** Future requests of the form "pathlint should know
  what mise currently has active" / "pathlint should detect stale
  pyenv shims" / "pathlint should warn when volta's default differs
  from package.json" land on ADR-0032 directly and can be evaluated
  against a written policy, not against a reconstruction from
  ADR-0009 + 0015 + 0022 + 0023 + 0031.

- **Positive.** The boundary between "add a catalog entry" (TOML PR,
  no Rust changes, no release coordination) and "add a Rust
  detector" (code review, lib surface implications, ADR if it
  changes the public API) becomes explicit. Contributors choosing
  the first path can move faster; reviewers questioning the second
  have a concrete contrast to point at.

- **Positive.** PRD §4's "No package-manager queries (0.1.x)" line
  gains a citation. The PRD revision in this release adds a single
  reference link to ADR-0032 at the end of §4 (and the parity-
  paired §4 in `PRD.jp.md`); no other PRD content changes.

- **Positive.** The 0.1.0 graduation criterion #6 (EN ↔ JP PRD
  parity audit) becomes easier — `pathlint` scope is now anchored
  in one place that both PRDs link to, instead of being scattered
  across §3 R4, §4, and implicit ADR references.

- **Negative.** Users who arrive expecting pathlint to know what
  their installer-of-choice is currently doing will be disappointed.
  The mitigation is the SARIF integration path (ADR-0031): pathlint
  emits findings about *paths*, and downstream tooling that *does*
  know installer state can join the SARIF reports against its own
  data. Users wanting "pathlint + mise current" can pipe both into
  the same SARIF aggregator.

- **Negative.** Declaring a hard boundary forecloses one design
  space: a future "pathlint daemon" that watches PATH change events
  and re-runs detectors. That was already foreclosed by ADR-0031
  (which rejected an LSP server and a bespoke daemon-mode RPC);
  ADR-0032 just makes the reason behind that rejection
  generalisable. If the daemon idea returns with a concrete user
  story, ADR-0031 + ADR-0032 are the pair to argue against, and the
  load-bearing question will be "does the daemon need tool-state
  queries to be useful?".

- **Neutral.** Implementation does not change in this release. The
  catalog is already declarative-only; `src/source_match.rs`,
  `src/lint.rs`, `src/doctor.rs`, and `src/trace.rs` already operate
  on path prefixes and relation entries with no tool-state callouts.
  This ADR records the policy that the code already reflects.

- **Neutral.** The `[[relation]]` kind list (`served_by_via`,
  `alias_of`, `conflicts_when_both_in_path`, `depends_on`,
  `prefer_order_over`) is what the policy delivers today. If a
  future tool meta cannot be expressed in those 5 kinds, the
  response is to consider extending the catalog's vocabulary in a
  separate ADR (and a versioned `catalog_version` bump per
  ADR-0023), not to start modelling tool behavior in Rust.

## Related ADRs

- **ADR-0009** (read-only stance) — the side-effect prohibition that
  tool-state queries would erode.
- **ADR-0014** (source naming) — the `<provenance>_<scope>` +
  `os_baseline_*` split that makes the OS / tool-meta distinction
  show up in source names.
- **ADR-0015** (`Provenance::WrapperInstaller` generalisation) — the
  pattern of "new tool entries via TOML, no Rust variant per tool".
- **ADR-0022** (`depends_on` descriptive-only) — relations declare
  intent for humans; they have no runtime effect.
- **ADR-0023** (`catalog_version` reserved for embedded) — user TOML
  cannot fork the catalog's identity; the catalog is one
  authoritative declaration that this ADR's "plugin = catalog
  entry" claim depends on.
- **ADR-0031** (SARIF + schemastore integration) — the integration
  layer where tool-state-aware downstream tooling joins pathlint's
  output, replacing the "pathlint hosts a plugin runtime" path.

## Follow-up

This ADR records policy only; no code or schema changes ship in
0.0.39. The Cargo bump is for the documentation release itself, and
PRD §4 gains one reference link to this ADR (both EN and JP).

The next release (0.0.40 candidate) is expected to extend
`locate_rules()` (`src/bin/pathlint/run.rs:401-441`) to support
**monorepo config discovery**: when the cwd lacks `pathlint.toml`,
walk up the directory tree to the enclosing `.git` boundary before
falling through to the XDG / `$HOME` user-global location. The walk
terminates at `.git` (worktree marker included) and does not climb
to filesystem root, to avoid accidentally picking up a stray
`pathlint.toml` in the user's home.

A `--scope=auto|local|global|system` global option is also under
consideration for the same release. Default `auto` would preserve
today's precedence (`--config` > cwd / walked repo > XDG); explicit
`--scope=local` or `--scope=global` would let users pin to one
layer without mutating cwd or XDG_CONFIG_HOME. With `auto` as
default, the option is purely additive — no existing invocation
changes behaviour — so no BREAKING flag would be required. The
design and trade-offs will land in a separate ADR (tentative
ADR-0033) at the moment the implementation does, not now;
recording the intent here only so that "monorepo config discovery"
and "scope flag" come up in the right ADR thread next cycle.
