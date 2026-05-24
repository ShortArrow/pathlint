# ADR-0004: Process-target registry provenance overlay

- **Status**: Accepted (supersedes part of ADR-0001, later superseded by ADR-0008 for PathEntry concept purity)
- **Date**: 2026-05-10
- **Release**: 0.0.24
- **Category**: 1. Public API surface (and 5. Architectural style — `--target process` Windows semantics gained an overlay)

## Context

ADR-0001 and ADR-0003 fixed `--target user` / `--target machine`
on Windows: pathlint reads the registry raw and the `%VAR%` form
survives all detectors. But the default `--target process` still
mis-suggested shortening.

The reason: `getenv("PATH")` on a Windows child process returns
the *post-expansion* literal. The OS calls
`ExpandEnvironmentStringsW` before handing PATH to the child, so a
registry entry stored as `%LocalAppData%\Microsoft\WindowsApps`
arrives as `C:\Users\me\AppData\Local\Microsoft\WindowsApps`.
`PathEntry::from_raw` happily builds a `PathEntry` with that as
`raw` — there is no `%VAR%` form to preserve because process-PATH
never carried one.

`Shortenable` then fires on it ("you could write this as
`%LocalAppData%\...`"), which is correct in isolation but
infuriating to anyone who already wrote that form in `regedit`.

## Decision

On Windows, `--target process` (the default) reads
`getenv("PATH")` **and also** reads HKCU + HKLM raw. A pure
function `reconcile_process_with_registry` overlays the registry
raw form onto each process entry whose `expanded` matches a
registry entry's `expanded`. The overlay is stored as a new
optional field on `PathEntry`:

```rust
pub struct PathEntry {
    pub raw: String,
    pub expanded: String,
    pub provenance_raw: Option<String>,
}
```

User-intent detectors (`Shortenable`, `Malformed`,
`TrailingSlash`, `ShortName`) read
`entry.effective_raw_for_user_intent()` — provenance when
present, observed `raw` otherwise. Filesystem-side detectors
(`Missing`, `WriteablePathDir`, etc.) keep reading `expanded`.

The overlay's rules:

- **Match condition**: `expand::normalize` equality on the
  `expanded` strings (case-insensitive + slash-unify).
- **Tie-break**: HKCU before HKLM, then first occurrence within a
  source. Deterministic.
- **Skipped when no expanded match is found.** Codex's safety
  rule: false-negative is preferable to false-suppression. A
  runtime-injected PATH (`set PATH=...` in a child shell) has no
  registry counterpart, and silently rewriting it would hide
  whatever rule the user violated.
- **Skipped when registry raw equals process raw verbatim.**
  REG_SZ entries don't need overlays.
- **`provenance_raw` stays `None` everywhere else.** `--target
  user` / `--target machine` already see raw at the source.
  Unix / macOS have no registry to overlay.

## Alternatives considered

Codex review identified four candidate designs:

- **A. Status quo + user education.** Tell users to run `--target
  user` if they want raw form. Rejected: the bug report
  ("doctor lies on my Windows machine") repeats every Windows
  install, and the user has no obvious cue from the doctor
  output that `--target user` exists.
- **B. Change `--target process` default on Windows to read
  HKCU+HKLM instead of `getenv`.** Rejected: this is the most
  visible BREAKING change — `--target` semantics flip on one OS
  and any user who customised PATH via `set PATH=...` in their
  shell would see different results from `--target process` than
  their actual environment. Also breaks the documented contract
  that `process` means "what `getenv` returns".
- **C. Read HKCU+HKLM raw only to suppress `Shortenable`, no
  exposed type.** Rejected as ad-hoc: the overlay information is
  useful to more than one detector (also to the human renderer,
  which should show the form the user typed). Hiding it inside
  one detector duplicates the I/O if `Shortenable` is excluded
  but other user-intent detectors aren't.
- **D. The chosen design** — exposed `provenance_raw` field +
  accessor. The cost is conceptual: `PathEntry` now holds both
  "what this single source said" (`raw`, `expanded`) and "what
  another source said the user wrote" (`provenance_raw`). That
  conflation is real and ADR-0001's follow-up acknowledges a
  future ADR will likely split provenance into its own type.

## Consequences

- **Positive.** Default `pathlint doctor` on Windows stops
  mis-suggesting shortening. The user-visible output also shows
  the registry `%VAR%` form, which matches what the user sees in
  `regedit`.
- **Positive.** Detectors that need user intent go through one
  accessor (`effective_raw_for_user_intent`). Filesystem-side
  detectors don't change. The split stays clean inside `analyze`.
- **Positive.** `--target user` / `--target machine` behaviour is
  byte-for-byte the same as 0.0.23: the overlay only applies in
  `read_process`.
- **Positive.** The reconcile function is pure and unit-tested on
  every OS (`overlay_tests` mod in `src/path_source.rs`). The
  Windows-only registry I/O that feeds it stays in the existing
  `cfg(windows)` test module.
- **Negative.** `PathEntry` now has three fields with two
  different conceptual roles (entry attributes vs cross-source
  reconciliation hint). The conflation makes the type harder to
  describe — see the module doc's "Observed vs. provenance"
  section. ADR-0001's follow-up notes this will likely be split
  before 0.1.0.
- **Negative.** Startup time on Windows process target now
  includes two `RegQueryValueEx` calls plus an `O(n*m)` reconcile
  (n = process entries, m = registry entries; m ≈ 30 in
  practice). Empirical cost is single-digit milliseconds; PRD
  §12's `< 50 ms` budget still holds.
- **Negative.** Race window: if the user mutates `HKCU\Path`
  between `getenv` and `RegQueryValueEx`, expanded equality will
  miss, and the entry stays with `provenance_raw = None`. The
  detector then fires `Shortenable` on a literal that the user
  *just* parameterised. The cost is bearable (false-negative on
  one entry until the next doctor run); the alternative
  (synchronising registry reads) is much more invasive.
- **Follow-up.** Step 4 of the 0.0.25-0.1.0 roadmap revisits this
  by splitting `PathEntry` into `PathEntry { raw, expanded }` and
  a new `Attribution { observed: PathEntry, provenance_raw }`
  carrier, restoring `PathEntry`'s concept purity at the cost of
  one more BREAKING.
