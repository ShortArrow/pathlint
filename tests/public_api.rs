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
use pathlint::expand::{expand_and_normalize, expand_env, expand_env_with, normalize};
use pathlint::lint::{
    CheckOutcomeView, Diagnosis, Outcome, Status, check_shape_filesystem, diagnose, evaluate,
    exit_code, has_config_error, is_failure,
};
use pathlint::os_detect::{Os, os_filter_applies};
use pathlint::path_entry::PathEntry;
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

#[test]
fn path_entry_from_raw_takes_env_lookup_closure() {
    // 0.0.23 BREAKING: PathEntry::from_raw is closure-receiving so
    // env injection is uniform across the lib. Pin both the field
    // shape and the closure signature here so a future change can't
    // silently revert.
    let entry = PathEntry::from_raw("$FOO/bin", |k| {
        (k == "FOO").then(|| "/expanded".to_string())
    });
    assert_eq!(entry.raw, "$FOO/bin");
    assert_eq!(entry.expanded, "/expanded/bin");
}

#[test]
fn analyze_signature_pinned_with_pathentry_slice() {
    // 0.0.23 BREAKING: doctor::analyze takes &[PathEntry]. A bare
    // `&[]` would still type-check via inference, so pin the slice
    // type explicitly here.
    use std::collections::BTreeMap;
    let entries: Vec<PathEntry> = vec![PathEntry::from_raw("/usr/bin", |_| -> Option<String> {
        None
    })];
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let relations: Vec<Relation> = Vec::new();
    let diags = analyze(
        &entries,
        &sources,
        &relations,
        Os::Linux,
        |_path: &str| -> bool { true },
        |_var: &str| -> Option<String> { None },
        |_path: &str| -> Vec<String> { Vec::new() },
        |_path: &str| -> bool { false },
    );
    // No expectations / no detectors that fire on an existing single
    // entry — just confirm the signature compiles and runs.
    assert!(diags.is_empty(), "got: {diags:?}");
}

#[test]
fn expand_env_with_pinned_on_public_surface() {
    // 0.0.23: pin expand::expand_env_with as part of the public lib
    // surface. The closure-receiving form is what `PathEntry::from_raw`
    // delegates to, and embedders building their own PathEntry-like
    // values may want it directly.
    let out = expand_env_with("$X", |k| (k == "X").then(|| "ok".to_string()));
    assert_eq!(out, "ok");
}

#[test]
fn path_entry_provenance_raw_pinned_on_public_surface() {
    // 0.0.24 BREAKING: PathEntry gains a third public field
    // `provenance_raw: Option<String>` for the Windows process-target
    // registry overlay. Pin both the field default and the accessor
    // shape so a future change can't silently revert.
    let pe = PathEntry::from_raw("/usr/bin", |_| -> Option<String> { None });
    assert_eq!(pe.provenance_raw, None);
    assert_eq!(pe.effective_raw_for_user_intent(), "/usr/bin");

    // `with_provenance` is the only public way for an embedder to set
    // the overlay; production code goes through the path_source
    // reconciler.
    let with_prov = pe.with_provenance("%CUSTOM%/bin".to_string());
    assert_eq!(with_prov.provenance_raw.as_deref(), Some("%CUSTOM%/bin"));
    assert_eq!(with_prov.effective_raw_for_user_intent(), "%CUSTOM%/bin");
}
