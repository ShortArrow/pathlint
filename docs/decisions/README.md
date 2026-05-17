# Architecture Decision Records

This directory records the **why** behind pathlint's load-bearing
design choices, in the format popularised by Michael Nygard. Each
ADR captures one decision: the forces that made the obvious answer
wrong, what was rejected, and what shipping it actually costs.

A decision belongs here when it meets one of these tests:

- It introduces or changes a publicly visible type / function on the
  10-module surface listed in [ARCHITECTURE.md](../ARCHITECTURE.md).
- It commits the project to a stance that future contributors will
  want to revisit (env injection policy, `--target` semantics, etc.).
- It bakes in a trade-off whose cost is paid by every release that
  follows (BREAKING in 0.0.x, schema-store registration, etc.).

A decision **does not** belong here when:

- It's a bug fix whose entire story fits in the commit message.
- It's a documentation-only edit (typos, drift fixes).
- It's a CI change with no downstream visibility.

## Index

| ADR | Title | Status | Shipped in |
|---|---|---|---|
| [0001](0001-pathentry-as-tenth-public-module.md) | PathEntry as the 10th public module | Accepted (partially superseded) | 0.0.23 |
| [0002](0002-from-raw-closure-injection.md) | `PathEntry::from_raw` takes a closure | Accepted | 0.0.23 |
| [0003](0003-reg-expand-sz-raw-decode.md) | Decode `REG_EXPAND_SZ` ourselves | Accepted | 0.0.23 |
| [0004](0004-process-target-registry-provenance-overlay.md) | Process-target registry provenance overlay | Accepted (supersedes part of 0001) | 0.0.24 |
| [0005](0005-pre-1-0-breaking-policy.md) | 0.0.x line allows MAJOR-equivalent BREAKING | Accepted | 0.0.x |

## When to write a new ADR

1. **Before opening a PR** that introduces a new public type or
   changes a public signature — the ADR is the explanation other
   readers will look for when the PR description has long scrolled
   past.
2. **When picking between two designs** and one of them rejects an
   obvious-looking alternative — the rejected alternative goes in
   the ADR's *Alternatives* section so future readers don't replay
   the same debate.
3. **When a release CHANGELOG entry has a `### Breaking` line** —
   the ADR linked from that line carries the full Context /
   Decision / Alternatives / Consequences treatment.

## How to write an ADR

Use the template at the bottom of this file. Keep each section
short: a reader should be able to skim Context + Decision in under
two minutes and decide whether the rest of the ADR is worth
reading.

- **Status**: `Proposed` → `Accepted` → optionally `Superseded by
  ADR-NNNN` or `Deprecated`. Never delete an old ADR; rewriting
  history hides the chain of decisions.
- **Date**: `YYYY-MM-DD` of the decision (not the file edit). Add
  a second line for the supersession date if relevant.
- **Release**: which crate version shipped the decision, or
  `planned` if the ADR predates implementation.

## Supersession

When a later ADR overrides an earlier one:

1. Mark the old ADR's status as `Superseded by ADR-NNNN` and add
   a one-line pointer at the top.
2. Do **not** edit the body of the superseded ADR — the body
   captures what was true at the time.
3. The new ADR references the old one in its Context and explains
   what changed.

This way `git blame` on the docs reflects edits to the ADR system
itself, not retroactive rewrites of history.

## Template

```markdown
# ADR-NNNN: <short title>

- **Status**: Proposed | Accepted | Superseded by ADR-MMMM | Deprecated
- **Date**: YYYY-MM-DD
- **Release**: 0.0.X (or "planned")

## Context

Why this decision needed to be made. The forces in play, the
constraint that made the obvious answer wrong, the prior art.

## Decision

What we decided. One sentence at the top, then details.

## Alternatives considered

Each rejected alternative with one-paragraph reason for rejection.
Future readers asking "why didn't they just do X" find their
answer here, not in commit archaeology.

## Consequences

- **Positive**: what got better.
- **Negative**: what got worse / what tech debt we accepted.
- **Follow-up**: what this decision implies for future work.
```
