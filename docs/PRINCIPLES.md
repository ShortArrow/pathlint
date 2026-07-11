# pathlint — Product Principles

🌐 **English** | [日本語](PRINCIPLES.jp.md)

The seven design principles below shape every CLI flag, every
detector kind, every catalog entry pathlint ships. They apply across
all four roles (R1 resolve order, R2 existence and shape, R3 PATH
hygiene + selfcheck, R4 provenance — see
[PRD §1](PRD.md#1-overview) for the role definitions). When a
proposed feature conflicts with one of these, the principle wins;
the feature gets reshaped, deferred, or rejected.

These are the *invariant* product principles. They have been in
force since 0.0.x began, and they survive the 0.0.x → 0.1.0
graduation unchanged. The scope boundary behind them is equally
fixed: pathlint commits to knowing how OSes lay out PATH and
where each tool declares its files land, and it deliberately does
not model what any tool does at runtime.

## The principles

1. **Declarative.** Whatever pathlint cares about is expressible
   in a `pathlint.toml` that lives in a dotfiles repo. Nothing is
   hidden in invocation flags only.

2. **Source labels, not paths.** Users speak in installer names
   (`cargo`, `mise_shims`, `winget`, `brew_arm`, `apt`) — the path
   patterns come from a catalog so the same TOML works on every
   machine.

3. **Built-in catalog with override.** pathlint ships defaults for
   the popular installers; users redefine `[source.X]` only when
   they want to override or add a new one.

4. **One file, all OSes.** Each `[[expect]]` may carry an
   `os = [...]` filter, and each `[source.X]` may declare per-OS
   paths (`windows = ...`, `unix = ...`, etc.). The same
   `pathlint.toml` drives Windows, macOS, Linux, and Termux.

5. **Substring + case-insensitive match.** Source paths are matched
   against the resolved binary path as substrings, after env-var
   expansion and slash normalization.

6. **Honest exit codes.** `0` = clean, `1` = at least one
   expectation failed, `2` = config / I/O error. R3 (`doctor`) and
   R4 (`where`) follow the same scale.

7. **Read-only.** pathlint never mutates PATH, registry, dotfiles,
   or installed packages. It tells you what's there; you act.

## How the principles cross-cut the rest of the docs

- **PRD §3** is the canonical statement of the principles inside the
  PRD; this file is a verbatim extraction so contributors can cite
  one short document instead of a PRD section heading.
- **PRD §4 (Non-goals)** records what falls *outside* the principles
  — package-manager queries, install simulation, PATH rewriting, and
  the "no document model, so no LSP server" line. Read §4 alongside
  this file when deciding whether a proposed feature fits.

When you reach for one of these — to cite it in a PR review, an
issue triage decision, or an ADR — link to the principle number in
this file. They stay stable across releases; PRD section numbering
may shift.
