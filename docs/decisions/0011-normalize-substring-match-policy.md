# ADR-0011: `expand::normalize` is case-insensitive + slash-unifying; substring match without canonicalisation

- **Status**: Accepted
- **Date**: 2026-05-31
- **Release**: 0.0.x (policy in force since 0.0.1; recorded retroactively in 0.0.30)
- **Category**: 3. Cross-cutting concern (path comparison policy)

## Context

pathlint compares paths in four places:

- `source_match::find_with` decides which catalog source(s) own a
  given resolved binary path.
- `doctor` detectors that reason about user intent
  (`Shortenable`, `Malformed`, `TrailingSlash`, `ShortName`) read
  the per-entry text.
- `trace::locate` walks a candidate resolved path back to the
  catalog source that contains it.
- `sort::sort_path` indexes PATH entries against catalog source
  paths to compute the proposed order.

All four go through a single comparison policy implemented in
`expand::normalize`:

```rust
pub fn normalize(input: &str) -> String {
    input.replace('\\', "/").to_ascii_lowercase()
}
```

…followed by **substring** match (haystack `.contains(needle)`)
after both sides go through `expand_and_normalize`. There is no
`std::fs::canonicalize`, no realpath, no symlink resolution.

The policy is load-bearing: every cross-platform expectation
("the same `pathlint.toml` works on Windows, macOS, Linux,
Termux") and every Windows behaviour (case-insensitive matching
of `C:\Users\Me\.cargo\bin` against catalog needle `cargo/bin`)
depends on it. Yet it has been implicit since 0.0.1 — there is
no ADR explaining why pathlint rejected the obvious-looking
alternative of "use `Path::canonicalize` and compare canonical
forms".

ADR-0000's Known ADR backlog table lists this as the
`expand::normalize` substring-match policy row, noting that
"path-canonicalize was rejected" deserves a recorded reason.
This ADR is that record.

## Decision

`expand::normalize` lowers ASCII case and converts `\\` to `/`,
then callers `.contains()` the normalised needle against the
normalised haystack. No canonicalisation, no symlink
resolution, no filesystem access during the comparison.

Concretely, the comparison `does PATH entry E match source S?`
is implemented as:

```rust
let h = expand::expand_and_normalize(E);
let n = expand::expand_and_normalize(S.path_for(os));
let matches = h.contains(&n);
```

The two `expand_and_normalize` calls each:
1. Run `expand_env` to substitute `%VAR%`, `$VAR`, `${VAR}`,
   and a leading `~`.
2. Run `normalize` (backslash→slash, ASCII lowercase).

Then `.contains()` decides match. The substring relation is
asymmetric: the catalog needle (`cargo/bin`) must appear inside
the resolved path (`/home/me/.cargo/bin`). Catalog sources are
authored as "the substring distinctive of this installer's
layout"; whether the user's binary lives at exactly that path
or one level deeper does not affect matching.

The policy does *not*:

- Resolve symlinks (`std::fs::read_link`).
- Canonicalise paths (`std::fs::canonicalize`).
- Compare structurally (component-by-component via `Path::components`).
- Honour locale-aware case folding (only ASCII `A-Z` → `a-z`).

## Alternatives considered

- **A. `std::fs::canonicalize` both sides before comparing.**
  Rejected for several reasons. First, `canonicalize` requires
  filesystem access — a PATH entry that does not exist returns
  an error; pathlint must still surface such entries (the
  `Missing` doctor detector depends on this). Second,
  `canonicalize` is unstable across Windows runtime states (it
  may resolve `C:\Users\Me` to `\\?\C:\Users\Me`, gain or lose
  case from disk, follow junctions differently in different
  Windows versions). Third, catalog needles deliberately do
  not have to exist on the host running pathlint — `mise_installs`
  has a needle of `.local/share/mise/installs`, which may or
  may not exist on a given user's machine; canonicalising it
  would either fail (rejecting the needle) or resolve against
  the wrong absolute path.

- **B. Component-by-component structural compare via
  `std::path::Path::components`.** Rejected because catalog
  needles are deliberately *substrings*, not whole paths. A
  needle of `cargo/bin` is meant to match
  `/home/me/.cargo/bin/rg` even though the path has additional
  components on either side. Structural compare would force
  catalog authors to anchor needles, which defeats the design
  ("substring distinctive of the installer's layout").

- **C. Locale-aware case folding (e.g. Unicode case mapping).**
  Rejected because pathlint's hot path runs on every PATH entry
  on every invocation, and locale-aware case mapping pulls in
  ICU-class dependencies for vanishingly small benefit. Real
  PATH entries on Windows are ASCII in 99.9% of cases; the
  remaining 0.1% (a Japanese username with kanji in `%USERPROFILE%`)
  is handled by exact-equality matching once `expand_env`
  substitutes the literal expanded form. The ASCII case-fold
  is the deliberate trade — performance and predictability over
  locale completeness.

- **D. Slash normalisation only, no case folding (Unix-style
  comparison).** Rejected because Windows filesystems are
  case-insensitive in practice — a catalog needle authored as
  `cargo/bin` must match `C:\Users\Me\.Cargo\Bin\` (note the
  cased `.Cargo`). Unix-only comparison would force catalog
  authors to ship case-variant needles or would lose Windows
  matches.

- **E. Tokenise the path and use prefix-trie matching.**
  Rejected as over-engineering. Substring `.contains()` is
  O(haystack × needle) but PATH entries are short (< 200
  chars typically) and needles even shorter (< 50 chars),
  so the runtime is unnoticeable. A trie would speed up
  matching across many needles but pathlint's catalog has
  ≈ 20 sources; the speedup is not worth the API complexity.

## Consequences

- **Positive.** No filesystem access on the comparison path
  means pathlint can run inside sandboxed evaluators (CI
  containers, embedded use cases) without granting
  arbitrary read access. The closure-injection work in
  ADR-0002 / ADR-0006 / ADR-0007 extends this: even the env
  oracle is caller-supplied.

- **Positive.** Catalog needles are stable across host
  states. A user editing `pathlint.toml` to add a source can
  test the needle on any machine and predict matching behaviour
  on every machine; canonicalisation would make this
  host-dependent.

- **Positive.** The single `expand_and_normalize` call site
  (one for haystack, one for needle, both with closure-injected
  env oracle) is the only place where this policy lives. A
  future change to the policy (e.g. honour `Path::components`
  for a subset of detectors) requires touching only that
  function plus its tests.

- **Negative.** Symlinks are not resolved. A PATH entry of
  `/usr/local/bin/python` that symlinks to
  `/usr/local/Cellar/python@3.12/3.12.4/bin/python` will not
  match a brew catalog needle that targets the Cellar form.
  Mitigation: catalog needles target the *user-visible* path
  (`/usr/local/bin`, `/opt/homebrew/bin`), which is what
  PATH-entry comparison sees anyway. The implication for trace
  R4 is that `pathlint trace python` reports the user-visible
  source (`brew_arm`) rather than the Cellar path, which is
  the desired output.

- **Negative.** Locale-aware case folding is not honoured.
  Users with non-ASCII chars in `$HOME` need to make sure the
  catalog needle uses the same case as the host's actual
  directory; the ASCII fold does not help with Unicode case
  variants. Practical impact: near zero for the populations
  pathlint targets today; the day someone files an issue is
  the day this trade-off gets revisited (and an ADR
  superseding this one will be written).

- **Negative.** Substring match has the usual false-positive
  surface: a needle of `bin` would match every PATH entry
  containing the substring. Mitigation: catalog needles in
  practice use installer-specific roots (`.cargo`, `mise`,
  `volta`, `homebrew`, `Microsoft\WindowsApps`) so the
  collision space is small. User-defined sources that pick
  too-generic needles get flagged by
  `source_match::validate_sources_with` (the per_source_missing_required
  and overlap warnings).

- **Follow-up.** None planned. If pathlint ever needs
  symlink-aware matching (e.g. for an R4 enhancement that
  reports both user-visible and physical paths), the new
  comparison policy goes into a sibling function
  (`expand::normalize_canonical`) and detectors opt in
  rather than this policy changing under everyone's feet.
