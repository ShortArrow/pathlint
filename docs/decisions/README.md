# Architecture Decision Records

This directory records the **why** behind pathlint's load-bearing
design choices, in the format popularised by Michael Nygard. Each
ADR captures one decision: the forces that made the obvious
answer wrong, what was rejected, and what shipping it actually
costs.

**Start here**: [ADR-0000](0000-adr-categories.md) defines the
eight decision categories pathlint recognises, the positive
criteria for writing an ADR (PA1-PA8), and the negative criteria
for *not* writing one (NA1-NA4). Every other ADR in this
directory belongs to one or more of those categories.

## Index by number (timeline view)

| ADR | Title | Category | Status | Shipped in |
|---|---|---|---|---|
| [0000](0000-adr-categories.md) | ADR categories and when to write one | 8 (self) | Accepted | 0.0.25 |
| [0001](0001-pathentry-as-tenth-public-module.md) | PathEntry as the 10th public module | 1 (+4) | Accepted (partially superseded by 0004) | 0.0.23 |
| [0002](0002-from-raw-closure-injection.md) | `PathEntry::from_raw` takes a closure | 3 (+1) | Accepted | 0.0.23 |
| [0003](0003-reg-expand-sz-raw-decode.md) | Decode `REG_EXPAND_SZ` ourselves | 4 | Accepted | 0.0.23 |
| [0004](0004-process-target-registry-provenance-overlay.md) | Process-target registry provenance overlay | 1 (+5) | Accepted (supersedes part of 0001) | 0.0.24 |
| [0005](0005-pre-1-0-breaking-policy.md) | 0.0.x line allows MAJOR-equivalent BREAKING | 8 | Accepted | 0.0.x |
| [0006](0006-source-match-env-closure-injection.md) | `_with` env-lookup closures on `expand_and_normalize` and `source_match` | 3 (+1) | Accepted | 0.0.26 |
| [0007](0007-deps-bag-layered.md) | layered `*Deps` carriers + per-function production wrappers | 2 (+1) | Accepted | 0.0.27 |

## Index by category (topical view)

The category numbers refer to the list in
[ADR-0000](0000-adr-categories.md). Importance ordering reflects
how often the category bites in pathlint specifically; the order
is **not** a universal ADR hierarchy.

### 1. Public API surface

- [ADR-0001](0001-pathentry-as-tenth-public-module.md) — PathEntry as the 10th public module
- [ADR-0004](0004-process-target-registry-provenance-overlay.md) — provenance overlay on `--target process`
- [ADR-0006](0006-source-match-env-closure-injection.md) — secondary (Added `_with` variants on `expand` / `source_match`)
- [ADR-0007](0007-deps-bag-layered.md) — secondary (4 BREAKING signature changes on `analyze` / `evaluate` / `locate` / `sort_path`)

### 2. Module boundary / dependency direction

- [ADR-0007](0007-deps-bag-layered.md) — layered `*Deps` carriers across `doctor::analyze`, `lint::evaluate`, `trace::locate`, `sort::sort_path`

### 3. Cross-cutting concern

- [ADR-0002](0002-from-raw-closure-injection.md) — env injection via closure on `PathEntry::from_raw`
- [ADR-0006](0006-source-match-env-closure-injection.md) — env injection extended to `expand::expand_and_normalize_with` and `source_match::*_with`

### 4. Trust / security boundary

- [ADR-0001](0001-pathentry-as-tenth-public-module.md) — secondary (registry decode boundary)
- [ADR-0003](0003-reg-expand-sz-raw-decode.md) — `decode_reg_string` lossy UTF-16 + type reject

### 5. Architectural style

- [ADR-0004](0004-process-target-registry-provenance-overlay.md) — secondary (Windows `--target process` semantics)

### 6. External dependency

*(none yet — `winreg` adoption predated the ADR system; schemars 1.0 evaluation is planned for Step 5 T.B.D.)*

### 7. Persistence / data format

*(none yet — JSON schema discriminator rename in 0.0.15 predated the ADR system; covered by its CHANGELOG entry)*

### 8. Process / governance

- [ADR-0000](0000-adr-categories.md) — this index, ADR categories and application criteria
- [ADR-0005](0005-pre-1-0-breaking-policy.md) — pre-1.0 BREAKING licence

## When to write an ADR

Authoritative answer: see [ADR-0000 §Decision](0000-adr-categories.md#decision).

Short version: if your PR matches any of PA1-PA8 in ADR-0000, write an
ADR before merging. If your PR matches all of NA1-NA4, do not.
When in doubt, lean toward writing one.

## How to write an ADR

Use the template at the bottom of this file. Keep each section
short: a reader should be able to skim Context + Decision in
under two minutes and decide whether the rest is worth reading.

- **Status**: `Proposed` → `Accepted` → optionally `Superseded
  by ADR-NNNN` or `Deprecated`. Never delete an old ADR;
  rewriting history hides the chain of decisions.
- **Date**: `YYYY-MM-DD` of the decision (not the file edit). Add
  a second line for the supersession date if relevant.
- **Release**: which crate version shipped the decision, or
  `planned` if the ADR predates implementation.
- **Category**: one or more of the eight categories listed in
  [ADR-0000](0000-adr-categories.md). Pick the primary category
  for the index sort; note secondary categories in parentheses.

## Supersession

When a later ADR overrides an earlier one:

1. Mark the old ADR's status as `Superseded by ADR-NNNN` and add
   a one-line pointer at the top.
2. Do **not** edit the body of the superseded ADR — the body
   captures what was true at the time. (Exception: additive
   `Category:` metadata when this ADR system was introduced;
   that backfill is one-time work for ADRs 0001-0005, recorded
   here for transparency.)
3. The new ADR references the old one in its Context and
   explains what changed.

This way `git blame` on the docs reflects edits to the ADR
system itself, not retroactive rewrites of history.

## Template

```markdown
# ADR-NNNN: <short title>

- **Status**: Proposed | Accepted | Superseded by ADR-MMMM | Deprecated
- **Date**: YYYY-MM-DD
- **Release**: 0.0.X (or "planned")
- **Category**: N. <name> (and M. <name> if applicable)

## Context

Why this decision needed to be made. The forces in play, the
constraint that made the obvious answer wrong, the prior art.

## Decision

What we decided. One sentence at the top, then details.

## Alternatives considered

Each rejected alternative with one-paragraph reason for
rejection. Future readers asking "why didn't they just do X"
find their answer here, not in commit archaeology.

## Consequences

- **Positive**: what got better.
- **Negative**: what got worse / what tech debt we accepted.
- **Follow-up**: what this decision implies for future work.
```
