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
use pathlint::CommonDeps;
use pathlint::doctor::{
    AnalyzeDeps, Diagnostic, Filter, Kind as DoctorKind, Severity as DoctorSeverity,
    all_kind_names, analyze, analyze_real, env_lookup_real, fs_exists_real, fs_list_dir_real,
    has_error, is_writable_dir_real, kind_name, user_diagnostic_names, validate_filter_names,
};
use pathlint::expand::{
    expand_and_normalize, expand_and_normalize_with, expand_env, expand_env_with, normalize,
};
use pathlint::lint::{
    CheckOutcomeView, Diagnosis, EvaluateDeps, Outcome, Status, check_shape_filesystem, diagnose,
    evaluate, evaluate_real, exit_code, has_config_error, is_failure,
};
use pathlint::os_detect::{Os, os_filter_applies};
use pathlint::path_entry::PathEntry;
use pathlint::sort::{EntryMove, SortDeps, SortNote, SortPlan, sort_path, sort_path_real};
use pathlint::source_match::{
    Match, SourceWarning, SourceWarningReason, find, find_with, names_only, names_only_with,
    validate_sources, validate_sources_with,
};
use pathlint::trace::{
    Found, LocateDeps, Provenance, TraceOutcome, UninstallHint, locate, locate_real,
};

#[test]
fn public_api_compiles() {
    // The use-statements above are the actual contract; this test
    // body is just a marker so the test runner reports something.
}

#[test]
fn evaluate_callable_with_evaluate_deps() {
    // 0.0.27 BREAKING: lint::evaluate now takes an EvaluateDeps
    // carrier instead of two positional closures. Pin the public
    // surface so external embedders can still wire a deterministic
    // resolver and shape_check.
    use std::collections::BTreeMap;
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let deps = EvaluateDeps {
        common: CommonDeps {
            env_lookup: |_var: &str| -> Option<String> { None },
        },
        resolver: |_cmd: &str| -> Option<std::path::PathBuf> { None },
        shape_check: |_path: &std::path::Path, _kind: Kind| -> Result<(), String> { Ok(()) },
    };
    let outcomes = evaluate(&[], &sources, Os::Linux, deps);
    assert!(outcomes.is_empty());
}

#[test]
fn locate_callable_with_locate_deps() {
    // 0.0.27 BREAKING: trace::locate now takes a LocateDeps carrier
    // instead of a single positional resolver closure.
    use std::collections::BTreeMap;
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let relations: Vec<Relation> = Vec::new();
    let deps = LocateDeps {
        common: CommonDeps {
            env_lookup: |_var: &str| -> Option<String> { None },
        },
        resolver: |_cmd: &str| -> Option<std::path::PathBuf> { None },
    };
    let outcome = locate(
        "definitely_no_such_command",
        &sources,
        &relations,
        Os::Linux,
        deps,
    );
    assert!(matches!(outcome, TraceOutcome::NotFound));
}

#[test]
fn analyze_callable_with_analyze_deps() {
    // 0.0.27 BREAKING: doctor::analyze now takes an AnalyzeDeps
    // carrier instead of 4 positional closures. The carrier groups
    // the 4 detectors' deps + the shared env_lookup (in CommonDeps).
    use std::collections::BTreeMap;
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let relations: Vec<Relation> = Vec::new();
    let deps = AnalyzeDeps {
        common: CommonDeps {
            env_lookup: |_var: &str| -> Option<String> { None },
        },
        fs_exists: |_path: &str| -> bool { false },
        fs_list_dir: |_path: &str| -> Vec<String> { Vec::new() },
        is_writable_dir: |_path: &str| -> bool { false },
    };
    let diags = analyze(&[], &sources, &relations, Os::Linux, deps);
    assert!(diags.is_empty());
    // Real wrappers are also part of the surface.
    let _real: Vec<String> = fs_list_dir_real("/this/path/does/not/exist");
    let _w: bool = is_writable_dir_real("/this/path/does/not/exist");
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
    // type explicitly here. 0.0.27 updated the trailing closures
    // into an AnalyzeDeps carrier; the PathEntry slice pin is
    // unchanged.
    use std::collections::BTreeMap;
    let entries: Vec<PathEntry> = vec![PathEntry::from_raw("/usr/bin", |_| -> Option<String> {
        None
    })];
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let relations: Vec<Relation> = Vec::new();
    let deps = AnalyzeDeps {
        common: CommonDeps {
            env_lookup: |_var: &str| -> Option<String> { None },
        },
        fs_exists: |_path: &str| -> bool { true },
        fs_list_dir: |_path: &str| -> Vec<String> { Vec::new() },
        is_writable_dir: |_path: &str| -> bool { false },
    };
    let diags = analyze(&entries, &sources, &relations, Os::Linux, deps);
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

#[test]
fn expand_and_normalize_with_pinned_on_public_surface() {
    // 0.0.26: pin expand::expand_and_normalize_with as a public
    // surface entry point. The closure-receiving form lets embedders
    // run pathlint without ever touching `std::env::var` themselves;
    // the existing `expand_and_normalize` stays as a wrapper that
    // reads the process env.
    let out =
        expand_and_normalize_with(r"$ROOT\BIN", |k| (k == "ROOT").then(|| "/Var".to_string()));
    // normalize() lowercases + slash-unifies, and expand_env_with
    // resolved $ROOT through the closure.
    assert_eq!(out, "/var/bin");
}

#[test]
fn source_match_with_variants_pinned_on_public_surface() {
    // 0.0.26: pin source_match::{find_with, validate_sources_with,
    // names_only_with} as part of the lib boundary so embedders can
    // resolve catalog source paths without leaking the process env.
    use pathlint::config::SourceDef;
    use std::collections::BTreeMap;

    let mut sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    sources.insert(
        "stub".into(),
        SourceDef {
            unix: Some(r"$STUB_HOME/.cargo/bin".into()),
            ..Default::default()
        },
    );

    let env_lookup =
        |k: &str| -> Option<String> { (k == "STUB_HOME").then(|| "/home/stub".to_string()) };

    // find_with expands the source's `unix` path via the closure, so
    // the haystack matches only when the closure provides $STUB_HOME.
    let hits = find_with(
        "/home/stub/.cargo/bin/runex",
        &sources,
        Os::Linux,
        env_lookup,
    );
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "stub");

    // names_only_with chains through find_with.
    let names = names_only_with(
        "/home/stub/.cargo/bin/runex",
        &sources,
        Os::Linux,
        env_lookup,
    );
    assert_eq!(names, vec!["stub".to_string()]);

    // validate_sources_with sees the same closure. With $STUB_HOME
    // resolved, the needle is long enough to clear the
    // NeedleTooShort heuristic, so no warnings fire.
    let warnings = validate_sources_with(&sources, Os::Linux, env_lookup);
    assert!(warnings.is_empty(), "got: {warnings:?}");
}

#[test]
fn common_deps_production_pinned_on_public_surface() {
    // 0.0.27: the shared CommonDeps carrier lives at the crate
    // root so every *Deps variant can embed it. Pin both the type
    // and the production() constructor.
    let _common: CommonDeps<fn(&str) -> Option<String>> = CommonDeps::production();
}

#[test]
fn analyze_deps_production_pinned_on_public_surface() {
    // 0.0.27: AnalyzeDeps::production() bundles the four `_real`
    // closures so the CLI binary can keep calling analyze_real
    // without re-wiring them.
    let deps = AnalyzeDeps::production();
    // The type can be assigned to a concrete fn-pointer-shaped
    // alias so external callers see the production wiring shape.
    let _typed: AnalyzeDeps<
        fn(&str) -> Option<String>,
        fn(&str) -> bool,
        fn(&str) -> Vec<String>,
        fn(&str) -> bool,
    > = deps;
}

#[test]
fn evaluate_real_pinned_on_public_surface() {
    // 0.0.27 Added: lint::evaluate_real is the production wrapper
    // mirroring analyze_real / locate_real / sort_path_real.
    use std::collections::BTreeMap;
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let entries: Vec<PathEntry> = Vec::new();
    let outcomes = evaluate_real(&[], &sources, Os::Linux, &entries);
    assert!(outcomes.is_empty());
}

#[test]
fn locate_real_pinned_on_public_surface() {
    // 0.0.27 Added.
    use std::collections::BTreeMap;
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let relations: Vec<Relation> = Vec::new();
    let entries: Vec<PathEntry> = Vec::new();
    let outcome = locate_real(
        "definitely_no_such_command",
        &sources,
        &relations,
        Os::Linux,
        &entries,
    );
    assert!(matches!(outcome, TraceOutcome::NotFound));
}

#[test]
fn sort_path_real_pinned_on_public_surface() {
    // 0.0.27 Added.
    use std::collections::BTreeMap;
    let entries: Vec<PathEntry> = Vec::new();
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let relations: Vec<Relation> = Vec::new();
    let plan = sort_path_real(&entries, &[], &sources, &relations, Os::Linux);
    // The plan can be inspected; just confirm the signature pins.
    let _ = plan;
}

#[test]
fn sort_path_callable_with_sort_deps() {
    // 0.0.27 BREAKING: sort::sort_path now takes a SortDeps carrier
    // (env_lookup only). The pin makes the closure-only carrier
    // explicit so future field additions are caught here.
    use std::collections::BTreeMap;
    let entries: Vec<PathEntry> = Vec::new();
    let sources: BTreeMap<String, SourceDef> = BTreeMap::new();
    let relations: Vec<Relation> = Vec::new();
    let deps = SortDeps {
        common: CommonDeps {
            env_lookup: |_var: &str| -> Option<String> { None },
        },
    };
    let _plan = sort_path(&entries, &[], &sources, &relations, Os::Linux, deps);
}
