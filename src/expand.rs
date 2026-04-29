//! Environment-variable expansion and slash normalization.
//!
//! Both `%VAR%` (Windows-style) and `$VAR` / `${VAR}` (POSIX-style) are
//! accepted on every OS so the same `pathlint.toml` works under
//! Windows pwsh, macOS bash, and Termux fish. Unresolved variables are
//! left verbatim — they simply fail to substring-match later.
//!
//! Slash normalization converts `\` to `/` so `mise\\shims` (TOML
//! literal) and `mise/shims` are equivalent for substring comparison.

use std::env;

/// Expand `%VAR%`, `$VAR`, `${VAR}`, and a leading `~` against the
/// process environment. Unresolved variables are kept verbatim.
pub fn expand_env(input: &str) -> String {
    let tilde = expand_tilde(input);
    let dollar = expand_dollar(&tilde);
    expand_percent(&dollar)
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

fn expand_tilde(s: &str) -> String {
    if let Some(rest) = s.strip_prefix('~') {
        if let Some(home) = env::var_os("HOME") {
            return format!("{}{}", home.to_string_lossy(), rest);
        }
        if let Some(profile) = env::var_os("USERPROFILE") {
            return format!("{}{}", profile.to_string_lossy(), rest);
        }
    }
    s.to_string()
}

fn expand_dollar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'{' {
                if let Some(end) = s[i + 2..].find('}') {
                    let name = &s[i + 2..i + 2 + end];
                    match env::var(name) {
                        Ok(val) => out.push_str(&val),
                        Err(_) => out.push_str(&s[i..i + 2 + end + 1]),
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
                match env::var(name) {
                    Ok(val) => out.push_str(&val),
                    Err(_) => out.push_str(&s[i..j]),
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

fn expand_percent(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(rel_end) = s[i + 1..].find('%') {
                let name = &s[i + 1..i + 1 + rel_end];
                if !name.is_empty() && name.chars().all(is_ident_char) {
                    match env::var(name) {
                        Ok(val) => out.push_str(&val),
                        Err(_) => out.push_str(&s[i..i + 1 + rel_end + 1]),
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
}
