//! Shell-string quoting used by uninstall-hint rendering.
//!
//! 0.0.17 carved this out of `format` to break the
//! `trace ↔ format` dependency cycle that codex review flagged
//! as a BLOCKER. `trace::derive_uninstall` needs to splice
//! quoted bin names into command templates and `format` needs
//! to render `trace::TraceOutcome` as JSON / human strings —
//! both directions used to flow through the same module, which
//! made `trace` (a public domain module) reverse-depend on
//! `format` (an internal presentation module) in violation of
//! the layering documented in `src/lib.rs`.
//!
//! With this module in place the dependency direction is:
//!
//!     trace → shell_quote          (uninstall command templating)
//!     format → trace                (rendering domain values)
//!     format → shell_quote          (still allowed if needed)
//!
//! No cycle.
//!
//! `pub(crate)` because the quoting rules are an implementation
//! detail; embedders should not poke at them. They get the same
//! quoted output via `pathlint trace --json` `uninstall.command`.
//!
//! 0.0.10 introduced these quoters in `format` to neutralise
//! shell metacharacters in attacker-controlled binary names; the
//! security argument is unchanged, just relocated.

use crate::os_detect::Os;

/// POSIX shell single-quote escape. Wraps the input in single
/// quotes and replaces every embedded `'` with `'\''` (close,
/// escaped quote, reopen). Always quotes — even simple inputs —
/// so the caller never has to decide whether quoting is needed.
/// Quoting also neutralises every other shell metacharacter
/// (`$`, `` ` ``, `;`, `&`, `|`, …), which is what makes this
/// safe to splice into `cargo uninstall {bin}` style templates
/// when the user copy-pastes it into bash / zsh / sh.
pub(crate) fn posix_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// PowerShell single-quote escape. Wraps the input in single
/// quotes and doubles every embedded `'` (PowerShell's literal
/// single-quote-inside-single-quotes convention). Used for
/// uninstall hints rendered on Windows hosts.
pub(crate) fn powershell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push('\'');
            out.push('\'');
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Quote `s` for a shell command displayed on `os`. Windows hosts
/// get PowerShell single-quote rules; everything else gets POSIX
/// single-quote rules. Both styles always quote, so a user can
/// safely substitute the result into `cargo uninstall {bin}` style
/// templates.
pub(crate) fn quote_for(os: Os, s: &str) -> String {
    match os {
        Os::Windows => powershell_quote(s),
        Os::Macos | Os::Linux | Os::Termux => posix_quote(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_quote_wraps_simple_input_in_single_quotes() {
        assert_eq!(posix_quote("lazygit"), "'lazygit'");
    }

    #[test]
    fn posix_quote_neutralises_metachars() {
        assert_eq!(posix_quote("$(rm -rf ~)"), "'$(rm -rf ~)'");
        assert_eq!(posix_quote("a;b`c"), "'a;b`c'");
        assert_eq!(posix_quote("with\nnewline"), "'with\nnewline'");
    }

    #[test]
    fn posix_quote_handles_embedded_single_quote() {
        assert_eq!(posix_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn powershell_quote_doubles_inner_single_quotes() {
        assert_eq!(powershell_quote("it's"), "'it''s'");
        assert_eq!(powershell_quote("plain"), "'plain'");
    }

    #[test]
    fn quote_for_dispatches_by_os() {
        assert_eq!(quote_for(Os::Linux, "it's"), "'it'\\''s'");
        assert_eq!(quote_for(Os::Macos, "it's"), "'it'\\''s'");
        assert_eq!(quote_for(Os::Termux, "it's"), "'it'\\''s'");
        assert_eq!(quote_for(Os::Windows, "it's"), "'it''s'");
    }
}
