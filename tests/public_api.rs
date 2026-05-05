//! 0.0.15 Step A2: compile-time pin for the lib public API
//! surface that 0.0.15 chose to expose.
//!
//! Each `use` line below is a contract: pathlint promises that
//! the named module + symbol is part of the supported library
//! surface. If a future change moves or removes one, this test
//! fails to compile, forcing the change to be either undone or
//! flagged as a breaking change in the release notes.
//!
//! Conversely, modules NOT mentioned here are intentionally
//! internal (`pub(crate)` in src/lib.rs) and may move freely.

#![allow(unused_imports)]

use pathlint::config::{Config, Expectation, Kind, Relation, Severity, SourceDef};
use pathlint::lint::{
    CheckOutcomeView, Diagnosis, Outcome, Status, check_shape_filesystem, diagnose, evaluate,
    exit_code, has_config_error, is_failure,
};
use pathlint::trace::{Found, Provenance, TraceOutcome, UninstallHint, locate};
use pathlint::sort::{EntryMove, SortNote, SortPlan, sort_path};
use pathlint::doctor::{
    Diagnostic, Filter, Kind as DoctorKind, Severity as DoctorSeverity, all_kind_names, analyze,
    analyze_real, env_lookup_real, fs_exists_real, has_error, kind_name,
    user_diagnostic_names, validate_filter_names,
};
use pathlint::catalog::{
    builtin, builtin_relations, check_acyclic, embedded_version, merge_with_user,
    merge_with_user_relations, version_check,
};
use pathlint::source_match::{
    Match, SourceWarning, SourceWarningReason, find, names_only, validate_sources,
};
use pathlint::os_detect::{Os, os_filter_applies};
use pathlint::expand::{expand_and_normalize, expand_env, normalize};

#[test]
fn public_api_compiles() {
    // The use-statements above are the actual contract; this test
    // body is just a marker so the test runner reports something.
}
