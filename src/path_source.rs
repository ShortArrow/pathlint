//! Acquire the PATH string for a chosen `--target`.
//!
//! * `process` — `getenv("PATH")` on every OS.
//! * `user` — `HKCU\Environment\Path` on Windows; warn and fall back
//!   to `process` on Unix.
//! * `machine` — `HKLM\System\CurrentControlSet\Control\Session Manager\Environment\Path`
//!   on Windows; warn and fall back on Unix.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Process,
    User,
    Machine,
}

#[derive(Debug)]
pub struct PathRead {
    pub value: String,
    pub warning: Option<String>,
}

pub fn read_path(target: Target) -> PathRead {
    match target {
        Target::Process => PathRead {
            value: std::env::var("PATH").unwrap_or_default(),
            warning: None,
        },
        Target::User => read_registry(target),
        Target::Machine => read_registry(target),
    }
}

#[cfg(windows)]
fn read_registry(target: Target) -> PathRead {
    use winreg::RegKey;
    use winreg::enums::*;

    let (root, subkey) = match target {
        Target::User => (RegKey::predef(HKEY_CURRENT_USER), "Environment"),
        Target::Machine => (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"System\CurrentControlSet\Control\Session Manager\Environment",
        ),
        Target::Process => unreachable!(),
    };

    match root.open_subkey(subkey) {
        Ok(key) => match key.get_value::<String, _>("Path") {
            Ok(v) => PathRead {
                value: v,
                warning: None,
            },
            Err(e) => PathRead {
                value: String::new(),
                warning: Some(format!("could not read Path value: {e}")),
            },
        },
        Err(e) => PathRead {
            value: String::new(),
            warning: Some(format!("could not open registry key: {e}")),
        },
    }
}

#[cfg(not(windows))]
fn read_registry(target: Target) -> PathRead {
    let label = match target {
        Target::User => "user",
        Target::Machine => "machine",
        Target::Process => unreachable!(),
    };
    PathRead {
        value: std::env::var("PATH").unwrap_or_default(),
        warning: Some(format!(
            "--target {label} is Windows-only; falling back to process PATH"
        )),
    }
}
