//! Environment-variable expansion and slash normalization.
//!
//! Both `%VAR%` (Windows-style) and `$VAR` / `${VAR}` (POSIX-style) are
//! accepted on every OS so the same `pathlint.toml` works under
//! Windows pwsh, macOS bash, and Termux fish. Unresolved variables are
//! left verbatim — they simply fail to substring-match later.
//!
//! Slash normalization converts `\` to `/` so `mise\\shims` (TOML
//! literal) and `mise/shims` are equivalent for substring comparison.
//!
//! # Examples
//!
//! ```
//! use pathlint::expand;
//!
//! assert_eq!(expand::normalize("C:\\Users\\Me"), "c:/users/me");
//!
//! // Unresolved variables stay verbatim.
//! let raw = expand::expand_env("$THIS_VAR_DOES_NOT_EXIST_PROBABLY_XYZ/bin");
//! assert!(raw.contains("$THIS_VAR_DOES_NOT_EXIST_PROBABLY_XYZ") || raw.ends_with("/bin"));
//! ```

use std::env;

/// Expand `%VAR%`, `$VAR`, `${VAR}`, and a leading `~` against the
/// caller-supplied env lookup. Pure — every env reference is
/// resolved through `env_lookup`, never through the process
/// environment directly. `~` resolves through the closure too:
/// pathlint asks for `HOME` first, then `USERPROFILE`. Unresolved
/// variables are kept verbatim.
///
/// Production callers pass `|v| std::env::var(v).ok()` (which is
/// what [`expand_env`] does internally). Tests and lib embedders
/// pass a deterministic closure so the result is independent of
/// the host environment.
///
/// 0.0.23+: introduced as the injection-aware form. [`expand_env`]
/// is now a thin wrapper.
pub fn expand_env_with<V>(input: &str, env_lookup: V) -> String
where
    V: Fn(&str) -> Option<String>,
{
    let tilde = expand_tilde(input, &env_lookup);
    let dollar = expand_dollar(&tilde, &env_lookup);
    expand_percent(&dollar, &env_lookup)
}

/// Expand `%VAR%`, `$VAR`, `${VAR}`, and a leading `~` against the
/// process environment. Unresolved variables are kept verbatim.
///
/// 0.0.23+: thin wrapper over [`expand_env_with`] that reads the
/// live process env. The actual expansion logic lives in
/// `expand_env_with`; this entry point exists for callers (the lib
/// itself, doctests, and embedders happy to read the host env)
/// who don't need to inject a custom lookup.
pub fn expand_env(input: &str) -> String {
    expand_env_with(input, |v| env::var(v).ok())
}

/// Lowercase + slash-normalize. Use on both haystack and needle before
/// substring comparison.
pub fn normalize(input: &str) -> String {
    input.replace('\\', "/").to_ascii_lowercase()
}

/// Convenience: expand env vars then normalize.
pub fn expand_and_normalize(input: &str) -> String {
    normalize(&expand_env(input))
}

/// Read PATHEXT and split into the literal extension tokens
/// (`.EXE`, `.BAT`, …). Defaults to the standard Windows list when
/// PATHEXT is unset. Caller passes its own env lookup so tests
/// can stub it; pass `|v| std::env::var(v).ok()` for production.
/// Use `pathext_lower` instead when case-insensitive comparison
/// is needed (the doctor `duplicate_but_shadowed` detector).
pub(crate) fn pathext_raw(env_lookup: impl Fn(&str) -> Option<String>) -> Vec<String> {
    env_lookup("PATHEXT")
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC".to_string())
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// PATHEXT entries lowercase. Convenience for case-insensitive
/// suffix matching on Windows.
pub(crate) fn pathext_lower(env_lookup: impl Fn(&str) -> Option<String>) -> Vec<String> {
    pathext_raw(env_lookup)
        .into_iter()
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn expand_tilde<V>(s: &str, env_lookup: &V) -> String
where
    V: Fn(&str) -> Option<String>,
{
    if let Some(rest) = s.strip_prefix('~') {
        if let Some(home) = env_lookup("HOME") {
            return format!("{home}{rest}");
        }
        if let Some(profile) = env_lookup("USERPROFILE") {
            return format!("{profile}{rest}");
        }
    }
    s.to_string()
}

fn expand_dollar<V>(s: &str, env_lookup: &V) -> String
where
    V: Fn(&str) -> Option<String>,
{
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'{' {
                if let Some(end) = s[i + 2..].find('}') {
                    let name = &s[i + 2..i + 2 + end];
                    match env_lookup(name) {
                        Some(val) => out.push_str(&val),
                        None => out.push_str(&s[i..i + 2 + end + 1]),
                    }
                    i += 2 + end + 1;
                    continue;
                }
            } else if is_ident_start(bytes[i + 1]) {
                let mut j = i + 1;
                while j < bytes.len() && is_ident_cont(bytes[j]) {
                    j += 1;
                }
                let name = &s[i + 1..j];
                match env_lookup(name) {
                    Some(val) => out.push_str(&val),
                    None => out.push_str(&s[i..j]),
                }
                i = j;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn expand_percent<V>(s: &str, env_lookup: &V) -> String
where
    V: Fn(&str) -> Option<String>,
{
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(rel_end) = s[i + 1..].find('%') {
                let name = &s[i + 1..i + 1 + rel_end];
                if !name.is_empty() && name.chars().all(is_ident_char) {
                    match env_lookup(name) {
                        Some(val) => out.push_str(&val),
                        None => out.push_str(&s[i..i + 1 + rel_end + 1]),
                    }
                    i += 1 + rel_end + 1;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_cont(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_var<F: FnOnce()>(key: &str, value: &str, body: F) {
        // Tests that touch process env vars must not run in parallel.
        // cargo test by default parallelizes; callers should use a unique
        // var name per test to avoid cross-test interference.
        // SAFETY: single-threaded scope per unique variable name.
        unsafe { env::set_var(key, value) };
        body();
        unsafe { env::remove_var(key) };
    }

    #[test]
    fn dollar_brace_expansion() {
        with_var("PATHLINT_TEST_BRACE", "ok", || {
            assert_eq!(expand_env("a/${PATHLINT_TEST_BRACE}/b"), "a/ok/b");
        });
    }

    #[test]
    fn dollar_bare_expansion() {
        with_var("PATHLINT_TEST_BARE", "ok", || {
            assert_eq!(expand_env("a/$PATHLINT_TEST_BARE/b"), "a/ok/b");
        });
    }

    #[test]
    fn percent_expansion() {
        with_var("PATHLINT_TEST_PCT", "ok", || {
            assert_eq!(expand_env("a/%PATHLINT_TEST_PCT%/b"), "a/ok/b");
        });
    }

    #[test]
    fn missing_var_is_kept_verbatim() {
        // Use a name that is exceedingly unlikely to be set.
        let s = "x/$PATHLINT_NOT_DEFINED_XYZ/y";
        assert_eq!(expand_env(s), s);
        let s2 = "x/%PATHLINT_NOT_DEFINED_XYZ%/y";
        assert_eq!(expand_env(s2), s2);
    }

    #[test]
    fn normalize_lowers_and_unifies_slashes() {
        assert_eq!(normalize("Foo\\Bar/Baz"), "foo/bar/baz");
    }

    #[test]
    fn lone_dollar_is_literal() {
        assert_eq!(expand_env("a$/b"), "a$/b");
    }

    #[test]
    fn mixed_percent_and_dollar_in_one_string() {
        with_var("PATHLINT_TEST_MIX_A", "AA", || {
            with_var("PATHLINT_TEST_MIX_B", "BB", || {
                let s = "%PATHLINT_TEST_MIX_A%/$PATHLINT_TEST_MIX_B/x";
                assert_eq!(expand_env(s), "AA/BB/x");
            });
        });
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(expand_env(""), "");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn unclosed_brace_is_kept_verbatim() {
        // `${FOO` (no closing brace) must not crash and must be left
        // alone.
        let s = "abc/${FOO/def";
        assert_eq!(expand_env(s), s);
    }

    #[test]
    fn percent_with_non_ident_inside_is_left_alone() {
        // `%not an ident%` is not a valid env-var name, must stay literal.
        let s = "50% off";
        assert_eq!(expand_env(s), s);
    }

    #[test]
    fn expand_and_normalize_combines_both_steps() {
        with_var("PATHLINT_TEST_COMBO", "C:/Users/U", || {
            assert_eq!(
                expand_and_normalize("$PATHLINT_TEST_COMBO\\.cargo\\bin"),
                "c:/users/u/.cargo/bin",
            );
        });
    }

    #[test]
    fn pathext_raw_uses_user_value_when_set() {
        let raw = pathext_raw(|v| {
            if v == "PATHEXT" {
                Some(".EXE;.BAT".to_string())
            } else {
                None
            }
        });
        assert_eq!(raw, vec![".EXE".to_string(), ".BAT".to_string()]);
    }

    #[test]
    fn pathext_raw_falls_back_to_default_when_unset() {
        let raw = pathext_raw(|_| None);
        assert!(raw.contains(&".EXE".to_string()));
        assert!(raw.contains(&".CMD".to_string()));
    }

    #[test]
    fn pathext_lower_normalizes_case() {
        let lower = pathext_lower(|v| {
            if v == "PATHEXT" {
                Some(".EXE;.Bat".to_string())
            } else {
                None
            }
        });
        assert_eq!(lower, vec![".exe".to_string(), ".bat".to_string()]);
    }

    // ---- 0.0.23: env_lookup injection through expand_env_with --------

    #[test]
    fn expand_env_with_calls_lookup_for_dollar_var() {
        // The injected closure is the only env source — process env
        // is not consulted. Pin both the call shape and that
        // unresolved tails stay verbatim.
        let out = expand_env_with("/x/$FOO/bin", |k| {
            (k == "FOO").then(|| "/replaced".to_string())
        });
        assert_eq!(out, "/x//replaced/bin");
    }

    #[test]
    fn expand_env_with_keeps_unresolved_var_verbatim() {
        let out = expand_env_with(
            "/x/$DEFINITELY_NOT_DEFINED_XYZ/bin",
            |_| -> Option<String> { None },
        );
        assert_eq!(out, "/x/$DEFINITELY_NOT_DEFINED_XYZ/bin");
    }

    #[test]
    fn expand_env_with_does_not_touch_process_env() {
        // Set a process env var that the closure does NOT report,
        // and confirm the result keeps the variable verbatim — the
        // closure is the only oracle expand_env_with consults.
        with_var("PATHLINT_TEST_INJECTION_LEAK", "leaked", || {
            let out = expand_env_with("x/$PATHLINT_TEST_INJECTION_LEAK/y", |_| -> Option<String> {
                None
            });
            assert_eq!(out, "x/$PATHLINT_TEST_INJECTION_LEAK/y");
        });
    }

    #[test]
    fn expand_env_with_handles_percent_and_brace_through_lookup() {
        let lookup = |k: &str| match k {
            "A" => Some("AA".to_string()),
            "B" => Some("BB".to_string()),
            _ => None,
        };
        assert_eq!(expand_env_with("%A%/${B}/c", lookup), "AA/BB/c");
    }

    // ---- 0.0.26: expand_and_normalize_with closure injection -------

    /// 0.0.26: pin that `expand_and_normalize_with` runs the input
    /// through `expand_env_with` (so `$VAR` resolves through the
    /// closure) and then through `normalize` (so backslashes flip
    /// to forward slashes and ASCII letters lowercase).
    #[test]
    fn expand_and_normalize_with_uses_closure_only() {
        let lookup = |k: &str| (k == "ROOT").then(|| "/Var".to_string());
        let out = expand_and_normalize_with(r"$ROOT\BIN", lookup);
        assert_eq!(out, "/var/bin");
    }

    /// 0.0.26: pin that `expand_and_normalize_with` does *not*
    /// fall back to the live process env when the closure says
    /// `None`. Without this guard the wrapper-vs-non-wrapper
    /// distinction collapses.
    #[test]
    fn expand_and_normalize_with_does_not_leak_process_env() {
        with_var("PATHLINT_TEST_NORMALIZE_LEAK", "leaked", || {
            let out = expand_and_normalize_with(
                "$PATHLINT_TEST_NORMALIZE_LEAK/x",
                |_| -> Option<String> { None },
            );
            // Closure returned None, so the variable stays verbatim
            // through expand_env_with and only normalize touches it.
            assert_eq!(out, "$pathlint_test_normalize_leak/x");
        });
    }
}
