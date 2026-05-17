# ADR-0003: Decode `REG_EXPAND_SZ` ourselves

- **Status**: Accepted
- **Date**: 2026-05-10
- **Release**: 0.0.23

## Context

The `winreg` crate offers two ways to read a string registry
value:

- `RegKey::get_value::<String, _>("Path")` — convenience method.
  For `REG_EXPAND_SZ` values it internally calls
  `ExpandEnvironmentStringsW` and returns the expanded literal.
- `RegKey::get_raw_value("Path")` — returns a `RegValue { bytes:
  Vec<u8>, vtype: RegType }`. The caller decides what to do.

pathlint 0.0.22 and earlier used `get_value::<String, _>`. The
expansion was invisible: a registry entry stored as
`%LocalAppData%\Microsoft\WindowsApps` arrived in
`path_source::read_path` as the post-expansion literal. The
`Shortenable` detector then suggested shortening it back, which
was visibly wrong to anyone who had typed the `%VAR%` form in
`regedit`.

The fix needed two things:

1. Read the raw bytes and decode them ourselves so the `%VAR%`
   form is preserved.
2. Run the same `expand_env` we use on Unix afterwards (via
   `PathEntry::from_raw`), so Windows and Unix follow one
   "raw at the source, expanded at the boundary" rule (ADR-0001).

## Decision

Replace `get_value::<String, _>` with `get_raw_value` plus a
hand-rolled UTF-16 LE decoder in `path_source::decode_reg_string`:

```rust
fn decode_reg_string(v: &winreg::RegValue) -> Result<String, &'static str> {
    match v.vtype {
        RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
            if v.bytes.len() % 2 != 0 {
                return Err("UTF-16 byte stream has odd length");
            }
            let units: Vec<u16> = v.bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let trimmed = match units.iter().position(|&u| u == 0) {
                Some(idx) => &units[..idx],
                None => &units[..],
            };
            Ok(String::from_utf16_lossy(trimmed))
        }
        _ => Err("unexpected registry value type"),
    }
}
```

The decoder is **lossy** on invalid UTF-16 surrogate pairs (uses
`from_utf16_lossy`, which replaces bad code units with `U+FFFD`)
and **rejects** every other registry type (`REG_MULTI_SZ`,
`REG_BINARY`, `REG_DWORD`, …) by returning `Err`. The caller
(`path_source::read_registry`) downgrades both error cases to a
`warning` and returns an empty `entries` vector — pathlint never
panics on a hostile registry payload and never silently emits
diagnostics built from garbled bytes.

The decoded raw string then goes through `split_into_entries` →
`PathEntry::from_raw`, which runs the same `expand::expand_env`
pipeline as Unix.

## Alternatives considered

- **Keep `get_value::<String, _>` and undo the expansion.** Would
  require reversing `ExpandEnvironmentStringsW` — impossible in
  general (multiple `%VAR%` references can expand to the same
  literal). Rejected.
- **Decode through `winreg`'s utf16-decoding helper.** `winreg`
  doesn't expose one; even if it did, the lossy / strict
  trade-off and the type-tag handling are pathlint policy
  decisions we want explicit in our code rather than buried in a
  dependency.
- **Reject `REG_EXPAND_SZ` on principle and require users to
  store `Path` as `REG_SZ`.** Rejected because every Windows
  installer that touches PATH writes `REG_EXPAND_SZ`, including
  `setx`. The user has no realistic way to change the type.
- **Decode `REG_MULTI_SZ` too.** Rejected: no Windows shipped
  PATH layout uses `REG_MULTI_SZ` for `Path`, and supporting it
  would commit pathlint to a multi-string handling path that
  duplicates the `split_into_entries` boundary. If a user has
  somehow stored `Path` as `REG_MULTI_SZ`, the explicit warning
  is more honest than guessing.

## Consequences

- **Positive.** The `%VAR%` form survives all the way through
  detector pipelines on Windows. `Shortenable` and friends see
  exactly what the user wrote.
- **Positive.** Hostile or corrupt registry values produce a
  warning, not a panic. The lossy decode is a documented contract
  (PRD §10.1), so the failure mode is predictable.
- **Negative.** pathlint now carries a small UTF-16 LE decoder.
  If `winreg` ships a `raw_value` variant that does what we want,
  we could collapse some of the code — but the decoder is 25 lines
  and 4 tests, so the maintenance cost is low.
- **Follow-up.** The decoder is internal (`pub(crate)`). If lib
  embedders later need to feed pathlint registry bytes they read
  themselves, the function could be lifted to a public surface
  with an ADR documenting that move.
