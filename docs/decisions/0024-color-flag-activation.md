# ADR-0024: `--color` flag activation (parsed-but-ignored → effective)

- **Status**: Accepted
- **Date**: 2026-05-05 (0.0.17 promotion); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.17
- **Category**: 8. Process / governance (capability activation policy)

## Context

Pre-0.0.17 `pathlint` declared a global `--color {auto, always,
never}` flag via clap, and pathlint's binary parsed the
argument, but the value was silently ignored — every human
renderer printed ANSI escapes (or didn't) based on terminal
detection alone, and `--color never` had no effect on the
output.

The flag had been added in an earlier release as a placeholder:
the color-handling plumbing existed in `report::Style`, but the
wiring between `cli.global.color` and `Style::color` had not
been written. Two release cycles passed with users seeing
`--color` in `--help` output and reasonably assuming it worked.

The 0.0.17 cut was already shipping major surface motion:
`Status` enum unit-only refactor (see ADR-0018), CLI `cli` /
`run` moved binary-side (see ADR-0017), shell quoting privatised.
Wiring `--color` through to the actual output path took a few
lines but was held back across earlier releases because the
work also revealed a subtle output-capture concern: scripts
that captured pathlint stdout to a file via shell redirection
might suddenly start seeing ANSI bytes in the captured stream
if `--color always` was set. The capture issue is not a bug
(it's `always` doing exactly what it says) but is a change in
observable behaviour for pipelines built against the previous
silent-ignore behaviour.

## Decision

`--color {auto, always, never}` is now **effective**:

- `auto` (default): resolved via
  `std::io::stdout().is_terminal()` — colorise when stdout
  is a TTY, plain output otherwise. Matches user expectation
  ("pipe to `less`, get plain text").
- `always`: colorise unconditionally, even when stdout is a
  pipe. Useful for scripts that pipe pathlint into a colour-
  aware viewer (`less -R`).
- `never`: never colorise. Useful for CI logs where ANSI
  escapes pollute the captured text.

The wiring is two lines: `cli.global.color.resolve(is_tty)`
returns a `bool`, threaded through `report::Style::color`. The
resolution logic lives in `ColorArg::resolve` (see
`src/bin/pathlint/cli.rs` lines 243-249).

CHANGELOG 0.0.17 announces this under `### Breaking` even
though it is technically *adding* behaviour (the silent-ignore
behaviour disappears), because pipelines that captured
pathlint stdout with `--color always` set may newly see ANSI
escapes in their captured stream.

## Alternatives considered

- **A. Keep ignoring the flag (status quo).** Rejected
  because shipping a CLI flag that does nothing is a
  promise-vs-implementation gap: users reading `--help` see
  the flag, try it, and it doesn't work. The drift
  accumulates user trust damage even if no single user
  files a bug.

- **B. Remove the flag entirely (delete `--color` from
  clap).** Rejected because color control is a genuine
  user need (CI vs interactive; `less -R` vs `less`); the
  flag *should* exist, it just needed wiring. Removing the
  flag and re-adding it later would have caused two
  BREAKING releases instead of one.

- **C. Make `--color` effective in a separate release before
  0.0.17 (e.g. 0.0.16).** Rejected because the wiring
  needed `report::Style` to be already in its final shape;
  0.0.16 was carrying the `Resolution` removal (see
  ADR-0018) which itself reshaped the format pipeline.
  Bundling color activation with 0.0.17's larger renderer
  reshape kept the BREAKING surface concentrated.

- **D. Activate as `--color always` only, leaving `auto`
  and `never` undefined.** Rejected because the three-value
  set is the standard convention (`grep --color=auto`,
  `ls --color=always`, etc.); shipping a partial set would
  be confusing and would invite future BREAKING releases
  to fill in the gaps.

- **E. Default to `--color never` (preserve previous
  observable behaviour for pipelines).** Rejected because
  the previous behaviour was "no colour ever" only for
  pipelines that happened to misuse `--color always`; for
  interactive terminal users the default should be `auto`,
  matching every other modern CLI tool's convention.
  `always` users who wanted plain output can switch to
  `never` if they need to.

## Consequences

- **Positive.** The flag now does what `--help` says it
  does. User trust restored.

- **Positive.** Output-capture pipelines that explicitly
  want ANSI escapes (passing to `less -R`, ingesting into
  a colour-aware log aggregator) can finally use
  `--color always` and have it work. Pipelines that
  capture to plain files have always had `--color never`
  available; now it works.

- **Positive.** The auto-detection path
  (`std::io::stdout().is_terminal()`) is the standard Rust
  channel for "am I being piped"; no custom TTY detection
  logic. Matches other Rust CLIs (`cargo`, `rg`, `bat`).

- **Negative.** Pipelines that captured pathlint stdout
  with `--color always` set (intentionally or by accident
  from a shell alias) may newly see ANSI bytes in the
  captured stream. The CHANGELOG 0.0.17 `### Breaking`
  entry calls this out explicitly; users grepping captured
  output for specific strings may need to add
  `--color never` to suppress the escapes.

- **Negative.** The fix relied on a subtle behavioural
  change (parsed-and-ignored → effective) that the
  CHANGELOG had to phrase carefully to make the
  output-capture concern visible. A future activation of
  the same kind (a long-ignored flag suddenly working)
  should reference this ADR for the precedent: the
  activation itself is a BREAKING change even though no
  symbol moved.

- **Follow-up.** None. The flag has stayed effective
  through 0.0.17-0.0.31; no further wiring or rejected-
  alternative work has surfaced.
