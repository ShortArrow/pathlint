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

use pathlint::catalog::{
    builtin, builtin_relations, check_acyclic, embedded_version, merge_with_user,
    merge_with_user_relations, version_check,
};
use pathlint::config::{Config, Expectation, Kind, Relation, Severity, SourceDef};
use pathlint::doctor::{
    Diagnostic, Filter, Kind as DoctorKind, Severity as DoctorSeverity, all_kind_names, analyze,
    analyze_real, env_lookup_real, fs_exists_real, fs_list_dir_real, has_error, kind_name,
    user_diagnostic_names, validate_filter_names,
};
use pathlint::expand::{expand_and_normalize, expand_env, normalize};
use pathlint::lint::{
    CheckOutcomeView, Diagnosis, Outcome, Status, check_shape_filesystem, diagnose, evaluate,
    exit_code, has_config_error, is_failure,
};
use pathlint::os_detect::{Os, os_filter_applies};
use pathlint::sort::{EntryMove, SortNote, SortPlan, sort_path};
use pathlint::source_match::{
    Match, SourceWarning, SourceWarningReason, find, names_only, validate_sources,
};
use pathlint::trace::{Found, Provenance, TraceOutcome, UninstallHint, locate};

#[test]
fn public_api_compiles() {
    // The use-statements above are the actual contract; this test
    // body is just a marker so the test runner reports something.
}

#[test]
fn evaluate_callable_with_pathbuf_resolver() {
    // 0.0.16 BLOCKER fix: lint::evaluate is part of the published
    // surface, so the resolver closure must be expressible with
    // public types alone. PathBuf is from std; pathlint's internal
    // Resolution wrapper would not compile here from an external
    // crate. integration tests run as a separate crate so this
    // test catches the regression.
    use std::collections::BTreeMap;
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let outcomes = evaluate(
        &[],
        &sources,
        Os::Linux,
        |_cmd: &str| -> Option<std::path::PathBuf> { None },
        |_path: &std::path::Path, _kind: Kind| -> Result<(), String> { Ok(()) },
    );
    assert!(outcomes.is_empty());
}

#[test]
fn locate_callable_with_pathbuf_resolver() {
    // Same DIP gate for trace::locate — the resolver closure must
    // return Option<PathBuf>, not the internal Resolution wrapper.
    use std::collections::BTreeMap;
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let relations: Vec<Relation> = Vec::new();
    let outcome = locate(
        "definitely_no_such_command",
        &sources,
        &relations,
        Os::Linux,
        |_cmd: &str| -> Option<std::path::PathBuf> { None },
    );
    assert!(matches!(outcome, TraceOutcome::NotFound));
}

#[test]
fn analyze_callable_with_fs_list_dir_closure() {
    // 0.0.19 BREAKING: doctor::analyze gains a 7th closure parameter
    // for the duplicate_but_shadowed detector. Pin the signature so
    // the public surface stays usable from external crates without
    // pulling in pathlint internals.
    use std::collections::BTreeMap;
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let relations: Vec<Relation> = Vec::new();
    let diags = analyze(
        &[],
        &sources,
        &relations,
        Os::Linux,
        |_path: &str| -> bool { false },
        |_var: &str| -> Option<String> { None },
        |_path: &str| -> Vec<String> { Vec::new() },
        |_path: &str| -> bool { false },
    );
    assert!(diags.is_empty());
    // Real wrapper is also part of the surface.
    let _real: Vec<String> = fs_list_dir_real("/this/path/does/not/exist");
}
