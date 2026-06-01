# Security model

pathlint is a read-only PATH linter. It reads the environment,
the filesystem, and (on Windows) the registry, and writes
diagnostics to stdout/stderr. It never mutates the host. This
document records what pathlint treats as trusted, what it
sanitises before rendering, and how to report a security issue.

## Trust boundaries

### Untrusted inputs

These come from outside pathlint's control and may carry hostile
content. Every one of them has a sanitisation step before reaching
human-facing output.

| Input | Source | Why untrusted | Sanitisation |
|---|---|---|---|
| `PATH` entries | `getenv("PATH")` | Any process in the user's history could have prepended an entry with control characters, ANSI escapes, or arbitrary bytes | [`format::strip_control_chars`](../src/format.rs) on human renderers; `serde_json` escapes on JSON output |
| Windows registry `Path` values | `HKCU\Environment\Path`, `HKLM\...\Environment\Path` | Registry can be edited by any installer or user, with arbitrary byte layout and type tags | [`path_source::decode_reg_string`](../src/path_source.rs): UTF-16 LE decode with `from_utf16_lossy` (invalid surrogates → `U+FFFD`); explicit type rejection for everything except `REG_SZ` / `REG_EXPAND_SZ` (returns `Err`, surfaces as a warning, never panics) |
| `Attribution.provenance_raw` overlay | `HKCU` / `HKLM` raw `Path` (Windows `--target process` only, 0.0.24+) | The registry raw form is overlaid onto a process entry whose `expanded` matches; the overlaid string is therefore a registry-derived byte sequence subject to the same threat model as the registry inputs above | Same `decode_reg_string` pipeline at read time. After overlay, the string reaches detectors via [`Attribution::effective_raw_for_user_intent`](../src/lib.rs) and human renderers via `format::strip_control_chars`. The 0.0.28 split kept this sanitisation chain intact: `PathEntry` carries no untrusted bytes by itself; `Attribution` is the carrier where the overlay lives. |
| Resolved binary names | filesystem | The basename of any executable under any PATH directory; can contain unusual characters | `format::strip_control_chars` on the rendered string |
| `pathlint.toml` user config | `--config` flag or `./pathlint.toml` | User-edited file; could be enormous, a symlink chain, or contain hostile shell-substitution-like strings | [`Config::from_path`](../src/config.rs): 16 MiB cap, single-symlink-hop allowance, regular-file check after symlink resolution. The post-parse `Config` value is then trusted internally. |
| Environment-variable values returned by `CommonDeps::env_lookup` (`PATHEXT`, `HOME`, `USERPROFILE`, and any other key referenced during source-path expansion or PATH entry construction) | `std::env::var` in production wiring (`CommonDeps::production`, the `*_real` family, and the two infrastructure boundaries documented in [ADR-0027](decisions/0027-lib-env-read-boundaries.md)); arbitrary closure return in tests / embedders | OS environment variables can be set by any process in the user's session, by installers' post-install hooks, or by a hostile actor with shell access. The closure itself is in-process code (and listed as trusted below), but the **bytes it returns** are external to pathlint and carry the same threat model as `PATH` entries. | Closure return values flow through [`expand::expand_env_with`](../src/expand.rs) (env substitution) and then [`format::strip_control_chars`](../src/format.rs) on human renderers / `serde_json` on JSON output — the same sanitiser chain that the top row's `PATH` entries pass through. |

### Trusted inputs

These are part of pathlint itself or have already passed a
boundary check. The lib treats them as authoritative.

| Input | Why trusted |
|---|---|
| Built-in source catalog | Embedded at compile time via `include_str!`; cannot be modified by a hostile user without rebuilding the binary |
| Post-validation `Config` from a user `pathlint.toml` | DoS guards (size cap, symlink hop check) have passed; the parsed structure is then a trusted carrier inside the lib |
| `Os::current()` and `Os::*` enum values | Determined by `cfg!()` at compile time, no runtime branching on user input |
| `CommonDeps::env_lookup` closure (0.0.27+) and the per-function `*Deps` it embeds (`AnalyzeDeps`, `EvaluateDeps`, `LocateDeps`, `SortDeps`) | The env oracle is **caller-supplied** at the public lib boundary. Production wiring (`CommonDeps::production`, `*::analyze_real` / `evaluate_real` / `locate_real` / `sort_path_real`) substitutes the live process env reader; tests and embedders supply deterministic closures. The closure itself is in-process code, not an attacker-controlled byte stream. Values *returned* by `env_lookup` flow through `expand::expand_env_with` and onwards into the same `PATH`-entry pipeline above, so they receive the same human-renderer / JSON sanitisation. |

## Sanitisation pointers

Each sanitiser lives in one place and is reused by every caller.
If a future change wants to introduce a new human renderer, it
should pass through the same point.

- **Control characters in human output**: [`format::strip_control_chars`](../src/format.rs) replaces ASCII control bytes (`0x00–0x08`, `0x0B–0x1F`, `0x7F`) with `?`, preserving `\t` and `\n`. Called from `format::doctor_line`, `format::doctor_conflict`, and every other human-facing renderer.
- **JSON output**: pathlint hands every JSON-bound value to `serde_json`, which escapes control bytes correctly. No custom escaping logic; if `serde_json` has a bug, pathlint inherits it (acceptable given the dependency's audit history).
- **UTF-16 registry decode**: [`path_source::decode_reg_string`](../src/path_source.rs) — lossy on invalid surrogate pairs, strict on unsupported registry types. The Windows-only branch in `path_source::read_registry` downgrades both failure modes to a `warning` and returns an empty `entries` vector, so pathlint never panics on a hostile registry payload.
- **Config DoS guards**: [`Config::from_path`](../src/config.rs) caps file size at 16 MiB before reading any byte, follows at most one symlink hop, and requires the final hop to be a regular file (rejects block devices, sockets, fifo, etc.). Errors here exit with status 2 (config/IO error).
- **Shell-command quoting (trace uninstall hints)**: [`format::quote_for(os, _)`](../src/format.rs) wraps catalog-supplied strings before they appear in `pathlint trace` output. POSIX single-quote on Unix-likes, PowerShell single-quote on Windows. Catalog *template bodies* themselves (the `uninstall_command = "..."` strings) are not re-quoted — they come from the catalog author or user config and pathlint trusts the source.
- **Cross-source provenance overlay (Windows process target)**: [`Attribution::effective_raw_for_user_intent`](../src/lib.rs) is the single accessor every user-intent detector (`Shortenable`, `Malformed`, `TrailingSlash`, `ShortName`) and the human renderer goes through. It returns `provenance_raw` when the `path_source` reconciler attached one, otherwise `observed.raw`. The 0.0.28 type split moved this concern off `PathEntry` so the carrier with untrusted-overlay potential is named for what it is. Filesystem-side detectors (`Missing`, `WriteablePathDir`) read `observed.expanded` and bypass the overlay entirely.
- **Lib-boundary env injection (`*Deps` carriers)**: [`CommonDeps::production`](../src/lib.rs) is the single place that wires `std::env::var` into the lib's env oracle for CLI / production. Every public entry point (`doctor::analyze`, `lint::evaluate`, `trace::locate`, `sort::sort_path`) takes a `*Deps` value whose `common.env_lookup` either was built through `production()` (CLI / `*_real`) or supplied by the embedder. Internal modules (`source_match::*_with`, `expand::expand_env_with`) never call `std::env::var` directly — they thread the closure. The net effect is that an embedder running pathlint inside a sandboxed evaluator can pass a closure that returns `None` for every key and pathlint will not reach for the process env, removing one ambient-input source from the trust surface. The closure's *return values* are themselves untrusted bytes (see the new trust-boundary row above); [ADR-0027](decisions/0027-lib-env-read-boundaries.md) documents the two intentional env-read boundary systems (source catalog resolution and PATH entry construction) and explains why the wrapper / `_with` split is the injection seam, not unfinished cleanup.

## Non-goals (security stance)

pathlint deliberately does **not** do the following, and reviewers
should reject patches that re-introduce these capabilities:

- **No PATH/registry mutation.** pathlint reads `PATH`, `HKCU\Path`,
  `HKLM\Path`. It never writes them. `pathlint sort` prints a
  proposed order; it never applies one. See [PRD.md §4](PRD.md#4-non-goals).
- **No privilege escalation.** pathlint runs with the privileges
  of the invoking user. Reading `HKLM\Path` succeeds for normal
  users (it's a global-readable key); pathlint does not request
  elevation, and would refuse a feature that did.
- **No child process spawning for resolution.** pathlint locates
  executables by path-prefix matching against its catalog. It does
  not call `dpkg -S`, `pacman -Qo`, `brew which-formula`, or any
  other tool to verify ownership. Trade-off documented in [PRD.md
  §4](PRD.md#4-non-goals) and revisited in §16. `pathlint --explain`
  *prints* candidate uninstall commands; the user runs them by
  hand.
- **No network access.** pathlint does not contact crates.io,
  GitHub, mise/cargo/winget registries, or any other network
  endpoint at runtime. The built-in catalog is embedded; updates
  ship via crate releases.
- **No deep environment parsing.** pathlint sees only what
  `getenv("PATH")` returns plus, on Windows, the two registry
  locations. `/etc/environment`, PAM, launchd plists, systemd
  unit `Environment=`, `eval "$(brew shellenv)"` are out of
  scope. Documented in [PRD.md §4](PRD.md#4-non-goals) and §16's
  R3 launchd discussion.

## Threat model

The realistic threats for a read-only linter:

1. **Hostile path entry triggers diagnostic injection.** A PATH
   entry like `/tmp/\x1b[31m_red` or `"abc\nINJECTED LINE"` could
   inject ANSI escapes or extra log lines into the doctor output.
   Mitigation: `strip_control_chars` runs on every human renderer.
2. **Hostile registry payload triggers a panic.** A `Path` value
   stored as `REG_DWORD` with arbitrary bytes, or a malformed
   UTF-16 LE string, could panic a naive decoder. Mitigation:
   `decode_reg_string` returns `Err` on type mismatch and
   `from_utf16_lossy` on bad code units. The caller downgrades
   both to a warning and returns empty `entries`.
3. **Hostile `pathlint.toml` causes resource exhaustion.** A
   16 GB TOML file or a symlink chain to `/dev/zero` could
   exhaust memory or disk. Mitigation: 16 MiB cap before reading
   any byte, single-symlink-hop allowance, regular-file check.
4. **Hostile catalog source path causes mismatch.** A user
   `[source.X] path = "/foo"` could match more than intended.
   Mitigation: the source catalog is trusted (user-controlled or
   embedded). pathlint does not parse paths to extract sub-paths;
   matching is substring + case-insensitive after
   `expand::normalize`.

Threats that are explicitly **out of scope** for the security
model (and therefore not mitigated):

- A user with shell access drops a malicious binary in a
  PATH-listed directory. `WriteablePathDir` (R3) surfaces such
  directories as a hygiene issue, but cannot prevent the attack;
  the user must read the diagnostic and act.
- A user's `pathlint.toml` declares fraudulent source paths
  intending to mislabel binaries. The config is user-provided
  and trusted; pathlint does not cross-check against a remote
  authority.

## Reporting a vulnerability

For coordinated disclosure, open a GitHub Security Advisory at
[github.com/ShortArrow/pathlint/security/advisories](https://github.com/ShortArrow/pathlint/security/advisories).

Public bug reports for security issues (filing a normal issue,
mentioning the bug in social media before a fix lands) are
allowed but not preferred. The project is small enough that the
fix cycle is short, but please use the Security Advisory channel
when the issue is unambiguously security-relevant.

There is currently no GPG key for encrypted email; the GitHub
Security Advisory channel is the supported path.

## What's pinned by tests

These are the security-relevant assertions that a CI failure
indicates a regression:

- `format::tests::doctor_conflict_strips_hostile_diagnostic` —
  ANSI-escape-bearing diagnostic gets sanitised
- `format::tests::doctor_line_strips_control_chars_from_entry` —
  per-entry control bytes get sanitised
- `path_source::tests::decode_reg_string_rejects_unsupported_reg_type`
  — registry types other than `REG_SZ` / `REG_EXPAND_SZ` are
  rejected
- `path_source::tests::decode_reg_string_rejects_odd_byte_length`
  — malformed UTF-16 input does not panic
- `config::tests::config_from_path_rejects_oversized_file` (and
  related symlink-hop tests in `config.rs#tests`) — DoS guards
  trip before file content is buffered

Future security work should add test coverage in the same form so
the contract stays visible.
