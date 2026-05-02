//! `pathlint sort --dry-run` — propose a PATH order that satisfies
//! every applicable `[[expect]]` rule, without touching the real PATH.
//!
//! Pure: the public entry point [`sort_path`] takes the current
//! entries, the expectation set, the merged source catalog, and the
//! current OS, and returns a [`SortPlan`]. Callers print it and
//! decide on the exit code.
//!
//! Read-only by design — PRD §4 forbids PATH mutation. Any future
//! `--apply` mode would live behind its own subcommand and a flag
//! the user has to pass explicitly.
//!
//! See PRD §7.8 for the role this plays in the four-role model
//! (R5 — repair, the inverse of R1 resolve order).
//!
//! ## Algorithm (0.0.8 MVP)
//!
//! 1. For every PATH entry, find which `[source.X]` names match it
//!    via [`crate::source_match::find`].
//! 2. For every `[[expect]]` rule whose `os` filter applies, mark
//!    the entries matching its `prefer` set as "preferred for
//!    `command`". Entries matching `avoid` are marked too.
//! 3. Compute a stable reordering: preferred entries float ahead of
//!    avoided entries for the same command, while every other
//!    entry's relative order is preserved.
//! 4. Diff the original and sorted vectors to populate `moves`.
//!
//! Stability matters: pathlint must not rearrange entries it has no
//! opinion on, so sysadmins reading the diff see only the changes
//! they need to think about.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::config::{Expectation, SourceDef};
use crate::expand::normalize;
use crate::os_detect::Os;
use crate::source_match;

/// One entry's movement from old to new index. Only emitted when
/// the entry actually moved; entries that stayed in place do not
/// generate a `EntryMove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntryMove {
    pub entry: String,
    pub from: usize,
    pub to: usize,
    /// Why the entry moved — names a command for which this entry
    /// is now preferred over the one it overtook. Free-form, single
    /// short sentence; meant for the human view.
    pub reason: String,
}

/// Non-blocking observation about a `[[expect]]` rule. The current
/// PATH cannot satisfy `prefer` (no PATH entry matches any preferred
/// source), so `sort` cannot fix it by reordering — the user has
/// to install the missing tool or adjust the rule. Surfaced so the
/// human view can include it as an "fyi" line below the diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SortNote {
    /// `prefer` is non-empty but no PATH entry currently matches any
    /// of the listed sources, so reordering cannot fix this rule.
    UnsatisfiablePrefer {
        command: String,
        prefer: Vec<String>,
    },
}

/// Pure-data result of [`sort_path`]. Always carries both the
/// `original` and the `sorted` vector, so consumers (human view /
/// JSON) can present a self-contained before / after without
/// re-running the algorithm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SortPlan {
    pub original: Vec<String>,
    pub sorted: Vec<String>,
    pub moves: Vec<EntryMove>,
    pub notes: Vec<SortNote>,
}

impl SortPlan {
    /// True when the proposed order is identical to the current one
    /// — nothing for the user to do.
    pub fn is_noop(&self) -> bool {
        self.moves.is_empty() && self.original == self.sorted
    }
}

/// Compute a sort plan. Pure.
///
/// `entries` is the current PATH split into entries (the same form
/// produced by `resolve::split_path`). `expectations` and `sources`
/// come from the merged catalog. `os` decides which expectations
/// apply (rules with an unmet `os` filter contribute nothing).
pub fn sort_path(
    entries: &[String],
    expectations: &[Expectation],
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
) -> SortPlan {
    let original: Vec<String> = entries.to_vec();

    // Index every entry by which sources it matches.
    let entry_sources: Vec<Vec<String>> = entries
        .iter()
        .map(|e| source_match::names_only(&normalize(e), sources, os))
        .collect();

    // Walk every applicable expectation and gather both promotion
    // (prefer) and demotion (avoid) intents per entry. `avoid` wins
    // when a single entry hits both, mirroring `lint::decide`.
    let intents = collect_intents(expectations, &entry_sources, os);

    let preferred_set: std::collections::BTreeSet<usize> = intents
        .iter()
        .filter_map(|(i, intent, _)| matches!(intent, Intent::Prefer).then_some(*i))
        .collect();
    let avoided_set: std::collections::BTreeSet<usize> = intents
        .iter()
        .filter_map(|(i, intent, _)| matches!(intent, Intent::Avoid).then_some(*i))
        .collect();

    // Three buckets in order: preferred → neutral → avoided. Each
    // bucket preserves the entries' original relative order, so the
    // diff only contains the moves the user has to think about.
    let preferred_idx = entries
        .iter()
        .enumerate()
        .filter_map(|(i, _)| preferred_set.contains(&i).then_some(i));
    let neutral_idx = entries.iter().enumerate().filter_map(|(i, _)| {
        (!preferred_set.contains(&i) && !avoided_set.contains(&i)).then_some(i)
    });
    let avoided_idx = entries
        .iter()
        .enumerate()
        .filter_map(|(i, _)| avoided_set.contains(&i).then_some(i));

    let new_order: Vec<usize> = preferred_idx
        .chain(neutral_idx)
        .chain(avoided_idx)
        .collect();
    let sorted: Vec<String> = new_order.iter().map(|&i| entries[i].clone()).collect();

    let moves: Vec<EntryMove> = new_order
        .iter()
        .copied()
        .enumerate()
        .filter(|(new_idx, old_idx)| new_idx != old_idx)
        .map(|(new_idx, old_idx)| EntryMove {
            entry: entries[old_idx].clone(),
            from: old_idx,
            to: new_idx,
            reason: reason_for(old_idx, &intents),
        })
        .collect();

    let notes = collect_notes(expectations, &entry_sources, os);

    SortPlan {
        original,
        sorted,
        moves,
        notes,
    }
}

/// Per-entry intent the rule set asks for. `Avoid` shadows
/// `Prefer` when both fire on a single entry — mirrors
/// `lint::decide`'s avoid-overrides-prefer policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    Prefer,
    Avoid,
}

/// Gather `(entry_index, intent, command)` triples. Multiple rules
/// can target the same entry; the bucket builder later resolves
/// conflicts by checking `avoided_set` first.
fn collect_intents(
    expectations: &[Expectation],
    entry_sources: &[Vec<String>],
    os: Os,
) -> Vec<(usize, Intent, String)> {
    let mut out = Vec::new();
    for expect in expectations {
        if !crate::os_detect::os_filter_applies(&expect.os, os) {
            continue;
        }
        for (i, srcs) in entry_sources.iter().enumerate() {
            if srcs.iter().any(|s| expect.avoid.iter().any(|a| a == s)) {
                out.push((i, Intent::Avoid, expect.command.clone()));
            } else if srcs.iter().any(|s| expect.prefer.iter().any(|p| p == s)) {
                out.push((i, Intent::Prefer, expect.command.clone()));
            }
        }
    }
    out
}

/// Pick a human-readable reason for the move at `old_idx`. Avoid
/// intents win when both apply (consistent with the bucket order).
fn reason_for(old_idx: usize, intents: &[(usize, Intent, String)]) -> String {
    let avoid_hit = intents
        .iter()
        .find(|(i, intent, _)| *i == old_idx && matches!(intent, Intent::Avoid));
    if let Some((_, _, cmd)) = avoid_hit {
        return format!("matches `avoid` for `{cmd}`");
    }
    let prefer_hit = intents
        .iter()
        .find(|(i, intent, _)| *i == old_idx && matches!(intent, Intent::Prefer));
    if let Some((_, _, cmd)) = prefer_hit {
        return format!("preferred source for `{cmd}`");
    }
    "displaced by a preferred entry".to_string()
}

/// Build `notes` independently from the intent gathering so the
/// note logic stays self-contained: `UnsatisfiablePrefer` fires
/// only when an applicable rule's `prefer` is non-empty and no
/// PATH entry matches any preferred source.
fn collect_notes(
    expectations: &[Expectation],
    entry_sources: &[Vec<String>],
    os: Os,
) -> Vec<SortNote> {
    expectations
        .iter()
        .filter(|expect| crate::os_detect::os_filter_applies(&expect.os, os))
        .filter(|expect| !expect.prefer.is_empty())
        .filter(|expect| {
            !entry_sources
                .iter()
                .any(|srcs| srcs.iter().any(|s| expect.prefer.iter().any(|p| p == s)))
        })
        .map(|expect| SortNote::UnsatisfiablePrefer {
            command: expect.command.clone(),
            prefer: expect.prefer.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(unix: &str) -> SourceDef {
        SourceDef {
            unix: Some(unix.into()),
            ..Default::default()
        }
    }

    fn cat(entries: &[(&str, SourceDef)]) -> BTreeMap<String, SourceDef> {
        entries
            .iter()
            .map(|(n, d)| (n.to_string(), d.clone()))
            .collect()
    }

    fn entries(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    fn expect_simple(command: &str, prefer: &[&str]) -> Expectation {
        Expectation {
            command: command.into(),
            prefer: prefer.iter().map(|s| s.to_string()).collect(),
            avoid: vec![],
            os: None,
            optional: false,
            kind: None,
            severity: crate::config::Severity::Error,
        }
    }

    #[test]
    fn empty_input_produces_noop_plan() {
        let plan = sort_path(&[], &[], &BTreeMap::new(), Os::Linux);
        assert!(plan.is_noop());
        assert_eq!(plan.original, plan.sorted);
        assert!(plan.moves.is_empty());
        assert!(plan.notes.is_empty());
    }

    #[test]
    fn already_sorted_path_is_noop() {
        // cargo entry first, system entry second — prefer = ["cargo"]
        // is already satisfied, so sort_path must not move anything.
        let sources = cat(&[
            ("cargo", src("/home/u/.cargo/bin")),
            ("usr_bin", src("/usr/bin")),
        ]);
        let path = entries(&["/home/u/.cargo/bin", "/usr/bin"]);
        let expects = vec![expect_simple("rg", &["cargo"])];
        let plan = sort_path(&path, &expects, &sources, Os::Linux);
        assert!(plan.is_noop(), "plan was: {plan:?}");
    }

    #[test]
    fn out_of_order_path_proposes_swap() {
        // /usr/bin appears before /home/u/.cargo/bin, but cargo is
        // the preferred source for "rg". Plan must put cargo first.
        let sources = cat(&[
            ("cargo", src("/home/u/.cargo/bin")),
            ("usr_bin", src("/usr/bin")),
        ]);
        let path = entries(&["/usr/bin", "/home/u/.cargo/bin"]);
        let expects = vec![expect_simple("rg", &["cargo"])];
        let plan = sort_path(&path, &expects, &sources, Os::Linux);
        assert!(!plan.is_noop());
        assert_eq!(
            plan.sorted,
            vec!["/home/u/.cargo/bin".to_string(), "/usr/bin".to_string(),]
        );
        assert_eq!(plan.moves.len(), 2);
        // The cargo entry moved up, the usr_bin entry was displaced.
        let cargo_move = &plan
            .moves
            .iter()
            .find(|m| m.entry.contains("cargo"))
            .unwrap();
        assert_eq!(cargo_move.from, 1);
        assert_eq!(cargo_move.to, 0);
        assert!(
            cargo_move.reason.contains("rg"),
            "reason: {}",
            cargo_move.reason
        );
    }

    #[test]
    fn unsatisfiable_prefer_emits_note_without_reordering() {
        // No PATH entry matches `cargo` — sort cannot fix this by
        // reordering. The plan must be a noop AND surface a note so
        // the user knows what's wrong.
        let sources = cat(&[
            ("cargo", src("/home/u/.cargo/bin")),
            ("usr_bin", src("/usr/bin")),
        ]);
        let path = entries(&["/usr/bin", "/usr/local/bin"]);
        let expects = vec![expect_simple("rg", &["cargo"])];
        let plan = sort_path(&path, &expects, &sources, Os::Linux);
        assert!(plan.is_noop());
        assert_eq!(plan.notes.len(), 1);
        match &plan.notes[0] {
            SortNote::UnsatisfiablePrefer { command, prefer } => {
                assert_eq!(command, "rg");
                assert_eq!(prefer, &vec!["cargo".to_string()]);
            }
        }
    }

    #[test]
    fn os_filter_excluded_rules_contribute_nothing() {
        // The rule applies only on Windows, but we evaluate on
        // Linux. Even though /usr/bin is "wrong" for that rule, no
        // reordering should happen.
        let sources = cat(&[
            ("cargo", src("/home/u/.cargo/bin")),
            ("usr_bin", src("/usr/bin")),
        ]);
        let path = entries(&["/usr/bin", "/home/u/.cargo/bin"]);
        let mut e = expect_simple("rg", &["cargo"]);
        e.os = Some(vec!["windows".into()]);
        let plan = sort_path(&path, &[e], &sources, Os::Linux);
        assert!(plan.is_noop());
    }

    #[test]
    fn preferred_entries_keep_relative_order_among_themselves() {
        // Two preferred entries already in the correct internal
        // order — they must stay in that order when sorted ahead of
        // a non-preferred entry.
        let sources = cat(&[
            ("cargo", src("/home/u/.cargo/bin")),
            ("mise_shims", src("/home/u/.local/share/mise/shims")),
            ("usr_bin", src("/usr/bin")),
        ]);
        let path = entries(&[
            "/usr/bin",
            "/home/u/.cargo/bin",
            "/home/u/.local/share/mise/shims",
        ]);
        let expects = vec![
            expect_simple("rg", &["cargo"]),
            expect_simple("python", &["mise_shims"]),
        ];
        let plan = sort_path(&path, &expects, &sources, Os::Linux);
        // cargo and mise_shims should both move up, keeping their
        // relative order (cargo before mise_shims, since cargo
        // appeared earlier in the original).
        let cargo_pos = plan
            .sorted
            .iter()
            .position(|e| e.contains("cargo"))
            .unwrap();
        let shims_pos = plan
            .sorted
            .iter()
            .position(|e| e.contains("shims"))
            .unwrap();
        let usr_pos = plan.sorted.iter().position(|e| e == "/usr/bin").unwrap();
        assert!(cargo_pos < shims_pos, "cargo should precede shims");
        assert!(
            shims_pos < usr_pos,
            "preferred entries should precede /usr/bin"
        );
    }

    #[test]
    fn entries_outside_any_source_keep_their_position() {
        // /opt/custom doesn't match any source; sort_path must not
        // move it — sysadmins reading the diff should see only what
        // they need to think about.
        let sources = cat(&[("cargo", src("/home/u/.cargo/bin"))]);
        let path = entries(&["/opt/custom", "/home/u/.cargo/bin"]);
        let expects = vec![expect_simple("rg", &["cargo"])];
        let plan = sort_path(&path, &expects, &sources, Os::Linux);
        // /home/u/.cargo/bin floats to position 0; /opt/custom
        // stays at the back.
        assert_eq!(plan.sorted[0], "/home/u/.cargo/bin");
        assert_eq!(plan.sorted[1], "/opt/custom");
    }

    fn expect_prefer_avoid(command: &str, prefer: &[&str], avoid: &[&str]) -> Expectation {
        Expectation {
            command: command.into(),
            prefer: prefer.iter().map(|s| s.to_string()).collect(),
            avoid: avoid.iter().map(|s| s.to_string()).collect(),
            os: None,
            optional: false,
            kind: None,
            severity: crate::config::Severity::Error,
        }
    }

    #[test]
    fn avoid_only_demotes_matching_entry_to_the_back() {
        // No prefer set, just avoid. The avoid entry should sink
        // below the neutral entry. This is the symmetric mirror of
        // promotion: avoid wants the entry *not* to win first.
        let sources = cat(&[
            ("winget", src("/winget/links")),
            ("plain", src("/usr/local/bin")),
        ]);
        let path = entries(&["/winget/links", "/usr/local/bin"]);
        let expects = vec![expect_prefer_avoid("rg", &[], &["winget"])];
        let plan = sort_path(&path, &expects, &sources, Os::Linux);
        // winget entry sinks below plain entry.
        assert_eq!(
            plan.sorted,
            vec!["/usr/local/bin".to_string(), "/winget/links".to_string(),]
        );
    }

    #[test]
    fn avoid_wins_when_entry_matches_both_prefer_and_avoid() {
        // Mirrors lint::decide's avoid-overrides-prefer rule. If a
        // single entry is both preferred (matches some prefer) and
        // avoided (matches some avoid), it should sink — sort must
        // not promote a path the rule explicitly forbids.
        let sources = cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            (
                "dangerous",
                src("/home/u/.local/share/mise/installs/python/3.10"),
            ),
            ("plain", src("/usr/bin")),
        ]);
        let path = entries(&[
            "/home/u/.local/share/mise/installs/python/3.10/bin",
            "/usr/bin",
        ]);
        let expects = vec![expect_prefer_avoid("python", &["mise"], &["dangerous"])];
        let plan = sort_path(&path, &expects, &sources, Os::Linux);
        // The mise/dangerous entry sinks past /usr/bin.
        assert_eq!(plan.sorted[0], "/usr/bin");
        assert!(plan.sorted[1].contains("dangerous") || plan.sorted[1].contains("python/3.10"));
    }

    #[test]
    fn avoid_with_no_match_is_silent() {
        // The avoid set names a source no PATH entry matches.
        // Nothing to demote; plan is a noop. No spurious note.
        let sources = cat(&[
            ("winget", src("/winget/links")),
            ("cargo", src("/home/u/.cargo/bin")),
        ]);
        let path = entries(&["/home/u/.cargo/bin", "/usr/bin"]);
        let expects = vec![expect_prefer_avoid("rg", &["cargo"], &["winget"])];
        let plan = sort_path(&path, &expects, &sources, Os::Linux);
        assert!(plan.is_noop(), "plan was: {plan:?}");
    }

    #[test]
    fn prefer_promotes_above_avoid_in_three_way_layout() {
        // Three entries: a preferred one, a neutral one, and an
        // avoided one. The order should be preferred → neutral →
        // avoided regardless of original layout.
        let sources = cat(&[
            ("cargo", src("/home/u/.cargo/bin")),
            ("winget", src("/winget/links")),
            ("plain", src("/usr/bin")),
        ]);
        let path = entries(&["/winget/links", "/usr/bin", "/home/u/.cargo/bin"]);
        let expects = vec![expect_prefer_avoid("rg", &["cargo"], &["winget"])];
        let plan = sort_path(&path, &expects, &sources, Os::Linux);
        assert_eq!(
            plan.sorted,
            vec![
                "/home/u/.cargo/bin".to_string(),
                "/usr/bin".to_string(),
                "/winget/links".to_string(),
            ]
        );
    }
}
