//! Taste memory: house rules and a variation history per domain, in two tiers.
//!
//! Project tier: `<cwd>/.girth/taste.jsonl` (can be committed). Personal tier:
//! `GIRTH_HOME/taste.jsonl` (carries across projects). Local only: `share` never reads it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ledger::{SCHEMA_VERSION, append_json_line, now};
use crate::sections::slug;

pub const TASTE_FILE: &str = "taste.jsonl";
pub const MAX_RULES: usize = 12;
pub const MAX_KEYS: usize = 8;
pub const MAX_VALUES: usize = 3;
pub const MAX_VALUE_CHARS: usize = 80;
pub const MAX_DOMAINS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    Add,
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TasteEvent {
    Rule {
        domain: String,
        text: String,
        op: Op,
    },
    Made {
        domain: String,
        project: String,
        choices: BTreeMap<String, String>,
    },
    Link {
        domain: String,
        skill: String,
    },
}

/// One taste line: schema version, unix timestamp, event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TasteRecord {
    pub v: u32,
    pub ts: u64,
    #[serde(flatten)]
    pub event: TasteEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Personal,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleRow {
    pub n: usize,
    pub text: String,
    pub tier: Tier,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct TasteView {
    pub domain: String,
    pub rules: Vec<RuleRow>,
    /// `(key, last distinct values, newest first)`, keys by most recent use.
    pub recent: Vec<(String, Vec<String>)>,
}

impl TasteView {
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.recent.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainRow {
    pub domain: String,
    pub rules: usize,
    pub made: usize,
}

pub fn path(tier: Tier, home: &Path, cwd: &Path) -> PathBuf {
    match tier {
        Tier::Personal => home.join(TASTE_FILE),
        Tier::Project => cwd.join(".girth").join(TASTE_FILE),
    }
}

pub fn append(path: &Path, event: TasteEvent) -> std::io::Result<()> {
    let record = TasteRecord {
        v: SCHEMA_VERSION,
        ts: now(),
        event,
    };
    append_json_line(path, &record)
}

/// All readable records; malformed, unknown or unreadable input reads as empty.
pub fn read(path: &Path) -> Vec<TasteRecord> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<TasteRecord>(line).ok())
        .collect()
}

/// Like `read`, but only lines containing one of `needles` are parsed (a cheap pre-filter).
pub fn read_matching(path: &Path, needles: &[&str]) -> Vec<TasteRecord> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| needles.iter().any(|n| line.contains(n)))
        .filter_map(|line| serde_json::from_str::<TasteRecord>(line).ok())
        .collect()
}

/// A crafted-skill domain slug: `[a-z0-9]+(-[a-z0-9]+)*`.
pub fn is_domain(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// Parse `key=value`: the key is slugged, the value trimmed and capped.
pub fn parse_choice(pair: &str) -> Result<(String, String), String> {
    let (key, value) = pair
        .split_once('=')
        .ok_or_else(|| format!("expected key=value, got '{pair}'"))?;
    let key = slug(key);
    let value: String = value.trim().chars().take(MAX_VALUE_CHARS).collect();
    if key.is_empty() || value.is_empty() {
        return Err(format!("expected key=value, got '{pair}'"));
    }
    Ok((key, value))
}

/// Rules after replaying add/drop for one domain in one tier, in add order.
fn rules_in(records: &[TasteRecord], domain: &str) -> Vec<String> {
    let mut rules: Vec<String> = Vec::new();
    for record in records {
        if let TasteEvent::Rule {
            domain: d,
            text,
            op,
        } = &record.event
        {
            if d != domain {
                continue;
            }
            match op {
                Op::Add if !rules.contains(text) => rules.push(text.clone()),
                Op::Add => {}
                Op::Drop => rules.retain(|r| r != text),
            }
        }
    }
    rules
}

/// The merged view of one domain: personal rules first, then project rules not already shown.
pub fn view(personal: &[TasteRecord], project: &[TasteRecord], domain: &str) -> TasteView {
    let mut rules: Vec<RuleRow> = Vec::new();
    for (tier, records) in [(Tier::Personal, personal), (Tier::Project, project)] {
        for text in rules_in(records, domain) {
            if rules.len() < MAX_RULES && !rules.iter().any(|r| r.text == text) {
                rules.push(RuleRow {
                    n: rules.len() + 1,
                    text,
                    tier,
                });
            }
        }
    }
    let mut made: Vec<(u64, &BTreeMap<String, String>)> = personal
        .iter()
        .chain(project)
        .filter_map(|r| match &r.event {
            TasteEvent::Made {
                domain: d, choices, ..
            } if d == domain => Some((r.ts, choices)),
            _ => None,
        })
        .collect();
    // Newest first; the sort is stable, so equal timestamps keep file order reversed below.
    made.reverse();
    made.sort_by_key(|m| std::cmp::Reverse(m.0));
    let mut recent: Vec<(String, Vec<String>)> = Vec::new();
    for (_, choices) in made {
        for (key, value) in choices {
            match recent.iter().position(|(k, _)| k == key) {
                Some(i) => {
                    let values = &mut recent[i].1;
                    if values.len() < MAX_VALUES && !values.contains(value) {
                        values.push(value.clone());
                    }
                }
                None if recent.len() < MAX_KEYS => recent.push((key.clone(), vec![value.clone()])),
                None => {}
            }
        }
    }
    TasteView {
        domain: domain.to_string(),
        rules,
        recent,
    }
}

/// Every domain with rules, made events or links, sorted by name.
pub fn domains(personal: &[TasteRecord], project: &[TasteRecord]) -> Vec<DomainRow> {
    let mut names: Vec<&str> = personal
        .iter()
        .chain(project)
        .map(|r| match &r.event {
            TasteEvent::Rule { domain, .. }
            | TasteEvent::Made { domain, .. }
            | TasteEvent::Link { domain, .. } => domain.as_str(),
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .map(|domain| DomainRow {
            domain: domain.to_string(),
            rules: rules_in(personal, domain).len() + rules_in(project, domain).len(),
            made: personal
                .iter()
                .chain(project)
                .filter(|r| matches!(&r.event, TasteEvent::Made { domain: d, .. } if d == domain))
                .count(),
        })
        .collect()
}

/// Domains a used skill belongs to: `girth-<d>` itself, then linked domains. At most 2.
pub fn resolve_domains(skill: &str, records: &[TasteRecord]) -> Vec<String> {
    let name = skill.rsplit(':').next().unwrap_or(skill);
    let mut found: Vec<String> = Vec::new();
    if let Some(domain) = name.strip_prefix(crate::skill::CRAFTED_PREFIX) {
        if is_domain(domain) {
            found.push(domain.to_string());
        }
    }
    for record in records {
        if let TasteEvent::Link { domain, skill: s } = &record.event {
            if (s == name || s == skill) && !found.contains(domain) {
                found.push(domain.clone());
            }
        }
    }
    found.truncate(MAX_DOMAINS);
    found
}

/// The text the model sees: rules, recent choices, and how to record this deliverable.
pub fn render(view: &TasteView, launcher: &str) -> String {
    render_with(view, launcher, false)
}

fn render_with(view: &TasteView, launcher: &str, more: bool) -> String {
    use std::fmt::Write;
    let domain = &view.domain;
    if view.is_empty() {
        return format!("taste: {domain} (nothing remembered yet)\n");
    }
    let mut out = format!("taste: {domain}\n");
    if !view.rules.is_empty() {
        out.push_str("house rules (always apply):\n");
        for row in &view.rules {
            let tag = if row.tier == Tier::Project {
                "(project) "
            } else {
                ""
            };
            let _ = writeln!(out, "  {}. {tag}{}", row.n, row.text);
        }
    }
    if !view.recent.is_empty() {
        out.push_str(
            "recently made (newest first; vary these unless a rule or the user pins them):\n",
        );
        for (key, values) in &view.recent {
            let _ = writeln!(out, "  {key}: {}", values.join(", "));
        }
    }
    if more {
        let _ = writeln!(out, "(more: {launcher} taste show {domain})");
    }
    let _ = writeln!(
        out,
        "record this one when done: {launcher} made {domain} key=value ..."
    );
    out
}

/// `render`, trimmed from the end to fit `max` bytes; rules and the record line are kept first.
pub fn render_capped(view: &TasteView, launcher: &str, max: usize) -> String {
    let full = render(view, launcher);
    if full.len() <= max {
        return full;
    }
    let mut trimmed = view.clone();
    loop {
        let text = render_with(&trimmed, launcher, true);
        if text.len() <= max || (trimmed.recent.is_empty() && trimmed.rules.is_empty()) {
            return text;
        }
        if trimmed.recent.pop().is_none() {
            trimmed.rules.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::TempDir;

    fn rec(ts: u64, event: TasteEvent) -> TasteRecord {
        TasteRecord {
            v: SCHEMA_VERSION,
            ts,
            event,
        }
    }

    fn rule(ts: u64, domain: &str, text: &str, op: Op) -> TasteRecord {
        rec(
            ts,
            TasteEvent::Rule {
                domain: domain.into(),
                text: text.into(),
                op,
            },
        )
    }

    fn made(ts: u64, domain: &str, choices: &[(&str, &str)]) -> TasteRecord {
        rec(
            ts,
            TasteEvent::Made {
                domain: domain.into(),
                project: "/p".into(),
                choices: choices
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            },
        )
    }

    fn link(domain: &str, skill: &str) -> TasteRecord {
        rec(
            1,
            TasteEvent::Link {
                domain: domain.into(),
                skill: skill.into(),
            },
        )
    }

    #[test]
    fn tier_paths() {
        let (home, cwd) = (Path::new("/h/.girth"), Path::new("/proj"));
        assert_eq!(
            path(Tier::Personal, home, cwd),
            PathBuf::from("/h/.girth/taste.jsonl")
        );
        assert_eq!(
            path(Tier::Project, home, cwd),
            PathBuf::from("/proj/.girth/taste.jsonl")
        );
    }

    #[test]
    fn append_read_round_trip_and_bad_lines_are_skipped() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("deep/taste.jsonl");
        append(
            &file,
            TasteEvent::Link {
                domain: "d".into(),
                skill: "s".into(),
            },
        )
        .unwrap();
        let mut text = std::fs::read_to_string(&file).unwrap();
        text.push_str("not json\n{\"v\":1,\"ts\":2,\"kind\":\"mystery\"}\n{\"v\":1,\"ts\":3,\"kind\":\"rule\",\"domain\":\"d\",\"te");
        std::fs::write(&file, text).unwrap();
        append(
            &file,
            TasteEvent::Rule {
                domain: "d".into(),
                text: "t".into(),
                op: Op::Add,
            },
        )
        .unwrap();
        let records = read(&file);
        assert_eq!(records.len(), 2, "{records:?}");
        assert!(matches!(records[0].event, TasteEvent::Link { .. }));
        assert!(matches!(records[1].event, TasteEvent::Rule { .. }));
        assert!(records[1].ts > 0 && records[1].v == SCHEMA_VERSION);
    }

    #[test]
    fn each_kind_needs_its_fields_and_extra_fields_are_ignored() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("taste.jsonl");
        std::fs::write(
            &file,
            [
                r#"{"v":1,"ts":1,"kind":"rule","domain":"d","text":"t","op":"add","extra":true}"#,
                r#"{"v":1,"ts":2,"kind":"made","domain":"d","project":"/p","choices":{"k":"v"}}"#,
                r#"{"v":1,"ts":3,"kind":"link","domain":"d","skill":"s"}"#,
                r#"{"v":1,"ts":4,"kind":"rule","domain":"d","text":"t"}"#,
                r#"{"v":1,"ts":5,"kind":"rule","domain":"d","op":"add"}"#,
                r#"{"v":1,"ts":6,"kind":"made","domain":"d","choices":{"k":"v"}}"#,
                r#"{"v":1,"ts":7,"kind":"made","domain":"d","project":"/p"}"#,
                r#"{"v":1,"ts":8,"kind":"link","domain":"d"}"#,
                r#"{"v":1,"ts":9,"kind":"link","skill":"s"}"#,
                r#"{"ts":10,"kind":"link","domain":"d","skill":"s"}"#,
            ]
            .join("\n"),
        )
        .unwrap();
        let ts: Vec<u64> = read(&file).iter().map(|r| r.ts).collect();
        assert_eq!(ts, vec![1, 2, 3]);
    }

    #[test]
    fn read_matching_parses_only_lines_containing_a_needle() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("taste.jsonl");
        for (domain, skill) in [
            ("captions", "video-mega"),
            ("copy", "copy-mega"),
            ("launch-video", "x"),
        ] {
            append(
                &file,
                TasteEvent::Link {
                    domain: domain.into(),
                    skill: skill.into(),
                },
            )
            .unwrap();
        }
        let domains_of = |needles: &[&str]| -> Vec<String> {
            read_matching(&file, needles)
                .into_iter()
                .map(|r| match r.event {
                    TasteEvent::Link { domain, .. } => domain,
                    _ => unreachable!(),
                })
                .collect()
        };
        assert_eq!(domains_of(&["video-mega"]), vec!["captions"]);
        assert_eq!(
            domains_of(&["copy", "launch-video"]),
            vec!["copy", "launch-video"]
        );
        assert!(domains_of(&[]).is_empty());
        assert!(read_matching(&tmp.path().join("none"), &["x"]).is_empty());
    }

    #[test]
    fn missing_or_directory_path_reads_as_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(read(&tmp.path().join("nope.jsonl")).is_empty());
        assert!(read(tmp.path()).is_empty());
    }

    #[test]
    fn domain_slugs() {
        for ok in ["captions", "launch-video", "a1-b2"] {
            assert!(is_domain(ok), "{ok}");
        }
        for bad in ["", "Launch", "a--b", "-a", "a-", "a b", "a_b", "é"] {
            assert!(!is_domain(bad), "{bad}");
        }
    }

    #[test]
    fn choices_parse_slug_trim_and_cap() {
        assert_eq!(
            parse_choice("Color Palette = neon on black "),
            Ok(("color-palette".into(), "neon on black".into()))
        );
        assert_eq!(parse_choice("url=a=b"), Ok(("url".into(), "a=b".into())));
        let long = format!("k={}", "é".repeat(100));
        assert_eq!(
            parse_choice(&long).unwrap().1.chars().count(),
            MAX_VALUE_CHARS
        );
        for bad in ["novalue", "=v", "k=", " = ", "!!=v"] {
            assert!(parse_choice(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rules_replay_add_drop_and_duplicates() {
        let recs = vec![
            rule(1, "d", "a", Op::Add),
            rule(2, "d", "b", Op::Add),
            rule(3, "d", "a", Op::Add),
            rule(4, "other", "x", Op::Add),
            rule(5, "d", "a", Op::Drop),
            rule(6, "d", "c", Op::Add),
        ];
        assert_eq!(rules_in(&recs, "d"), vec!["b", "c"]);
        let twice = vec![rule(1, "d", "a", Op::Add), rule(2, "d", "a", Op::Add)];
        assert_eq!(rules_in(&twice, "d"), vec!["a"]);
    }

    #[test]
    fn view_numbers_personal_then_project_and_collapses_duplicates() {
        let personal = vec![rule(1, "d", "Logo last", Op::Add)];
        let project = vec![
            rule(2, "d", "Logo last", Op::Add),
            rule(3, "d", "Sentence case", Op::Add),
        ];
        let v = view(&personal, &project, "d");
        assert_eq!(
            v.rules,
            vec![
                RuleRow {
                    n: 1,
                    text: "Logo last".into(),
                    tier: Tier::Personal
                },
                RuleRow {
                    n: 2,
                    text: "Sentence case".into(),
                    tier: Tier::Project
                },
            ]
        );
        assert_eq!(v.domain, "d");
    }

    #[test]
    fn dropping_in_one_tier_keeps_the_other_tiers_copy() {
        let personal = vec![
            rule(1, "d", "Logo last", Op::Add),
            rule(5, "d", "Logo last", Op::Drop),
        ];
        let project = vec![rule(2, "d", "Logo last", Op::Add)];
        let v = view(&personal, &project, "d");
        assert_eq!(v.rules.len(), 1);
        assert_eq!(v.rules[0].tier, Tier::Project);
    }

    #[test]
    fn rules_are_capped() {
        let personal: Vec<_> = (0..20)
            .map(|i| rule(i, "d", &format!("r{i}"), Op::Add))
            .collect();
        let v = view(&personal, &[], "d");
        assert_eq!(v.rules.len(), MAX_RULES);
        assert_eq!(v.rules.last().unwrap().n, MAX_RULES);
    }

    #[test]
    fn recent_is_newest_first_distinct_and_capped_across_tiers() {
        let personal = vec![
            made(1, "d", &[("palette", "neon"), ("music", "synth")]),
            made(3, "d", &[("palette", "paper")]),
            made(4, "other", &[("palette", "nope")]),
        ];
        let project = vec![
            made(2, "d", &[("palette", "neon")]),
            made(5, "d", &[("palette", "neon"), ("font", "serif")]),
            made(0, "d", &[("palette", "pastel"), ("palette2", "x")]),
        ];
        let v = view(&personal, &project, "d");
        assert_eq!(
            v.recent,
            vec![
                ("font".to_string(), vec!["serif".to_string()]),
                (
                    "palette".to_string(),
                    vec![
                        "neon".to_string(),
                        "paper".to_string(),
                        "pastel".to_string()
                    ]
                ),
                ("music".to_string(), vec!["synth".to_string()]),
                ("palette2".to_string(), vec!["x".to_string()]),
            ]
        );
    }

    #[test]
    fn recent_keys_and_values_are_capped() {
        let personal: Vec<_> = (0..12)
            .map(|i| {
                made(
                    i,
                    "d",
                    &[(&format!("k{i}"), "v"), ("shared", &format!("v{i}"))],
                )
            })
            .collect();
        let v = view(&personal, &[], "d");
        assert_eq!(v.recent.len(), MAX_KEYS);
        let shared = &v.recent.iter().find(|(k, _)| k == "shared").unwrap().1;
        assert_eq!(shared, &vec!["v11", "v10", "v9"]);
    }

    #[test]
    fn empty_view() {
        let v = view(&[], &[link("d", "s")], "d");
        assert!(v.is_empty());
        assert!(!view(&[made(1, "d", &[("k", "v")])], &[], "d").is_empty());
        assert!(!view(&[rule(1, "d", "r", Op::Add)], &[], "d").is_empty());
    }

    #[test]
    fn domains_list_counts_and_includes_link_only() {
        let personal = vec![
            rule(1, "b", "r", Op::Add),
            made(2, "b", &[("k", "v")]),
            made(3, "b", &[("k", "w")]),
            link("a", "video-mega"),
        ];
        let project = vec![rule(4, "b", "r2", Op::Add)];
        assert_eq!(
            domains(&personal, &project),
            vec![
                DomainRow {
                    domain: "a".into(),
                    rules: 0,
                    made: 0
                },
                DomainRow {
                    domain: "b".into(),
                    rules: 2,
                    made: 2
                },
            ]
        );
    }

    #[test]
    fn resolve_domains_from_prefix_and_links_capped() {
        let recs = vec![
            link("captions", "video-mega"),
            link("launch-video", "video-mega"),
            link("third", "video-mega"),
            link("captions", "video-mega"),
        ];
        assert_eq!(resolve_domains("girth-captions", &recs), vec!["captions"]);
        assert_eq!(
            resolve_domains("my-plugin:girth-launch-video", &[]),
            vec!["launch-video"]
        );
        assert_eq!(
            resolve_domains("video-mega", &recs),
            vec!["captions", "launch-video"]
        );
        assert_eq!(
            resolve_domains("plug:video-mega", &recs),
            vec!["captions", "launch-video"]
        );
        assert!(resolve_domains("girth-", &recs).is_empty());
        assert!(resolve_domains("unrelated", &recs).is_empty());
    }

    #[test]
    fn render_empty_and_full() {
        let empty = view(&[], &[], "launch-video");
        assert_eq!(
            render(&empty, "girth"),
            "taste: launch-video (nothing remembered yet)\n"
        );
        let v = view(
            &[
                rule(1, "launch-video", "End on the logo.", Op::Add),
                made(2, "launch-video", &[("palette", "neon")]),
            ],
            &[rule(3, "launch-video", "Sentence case.", Op::Add)],
            "launch-video",
        );
        assert_eq!(
            render(&v, "/x/scripts/girth"),
            "taste: launch-video\n\
             house rules (always apply):\n  \
             1. End on the logo.\n  \
             2. (project) Sentence case.\n\
             recently made (newest first; vary these unless a rule or the user pins them):\n  \
             palette: neon\n\
             record this one when done: /x/scripts/girth made launch-video key=value ...\n"
        );
    }

    #[test]
    fn render_omits_empty_sections() {
        let only_made = view(&[made(1, "d", &[("k", "v")])], &[], "d");
        let text = render(&only_made, "girth");
        assert!(!text.contains("house rules"));
        assert!(text.contains("recently made"));
        let only_rules = view(&[rule(1, "d", "r", Op::Add)], &[], "d");
        assert!(!render(&only_rules, "girth").contains("recently made"));
    }

    #[test]
    fn render_capped_keeps_the_record_line_and_points_to_the_rest() {
        let personal: Vec<_> = (0..12)
            .map(|i| rule(i, "d", &format!("rule {i} {}", "x".repeat(200)), Op::Add))
            .chain([made(20, "d", &[("palette", "neon")])])
            .collect();
        let v = view(&personal, &[], "d");
        let text = render_capped(&v, "girth", 1500);
        assert!(text.len() <= 1500, "{}", text.len());
        assert!(text.contains("rule 0 "));
        assert!(text.ends_with(
            "(more: girth taste show d)\nrecord this one when done: girth made d key=value ...\n"
        ), "{text}");
        let small = view(&[rule(1, "d", "r", Op::Add)], &[], "d");
        assert_eq!(
            render_capped(&small, "girth", 1500),
            render(&small, "girth")
        );
    }

    fn arb_record() -> impl Strategy<Value = TasteRecord> {
        let domain = prop::sample::select(vec!["a", "b"]);
        let text = prop::sample::select(vec!["x", "y", "z"]);
        prop_oneof![
            (0u64..50, domain.clone(), text.clone(), any::<bool>()).prop_map(|(ts, d, t, add)| {
                rule(ts, d, t, if add { Op::Add } else { Op::Drop })
            }),
            (0u64..50, domain, text, text_strategy()).prop_map(|(ts, d, k, v)| made(
                ts,
                d,
                &[(k, v.as_str())]
            )),
        ]
    }

    fn text_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["p", "q", "r", "s"]).prop_map(str::to_string)
    }

    proptest! {
        #[test]
        fn view_never_shows_a_dropped_rule_or_repeats_a_value(
            personal in prop::collection::vec(arb_record(), 0..30),
            project in prop::collection::vec(arb_record(), 0..30),
        ) {
            let v = view(&personal, &project, "a");
            for row in &v.rules {
                let tier_recs = if row.tier == Tier::Personal { &personal } else { &project };
                prop_assert!(rules_in(tier_recs, "a").contains(&row.text));
            }
            for (_, values) in &v.recent {
                let mut seen = values.clone();
                seen.sort();
                seen.dedup();
                prop_assert_eq!(seen.len(), values.len());
                prop_assert!(values.len() <= MAX_VALUES);
            }
            let ns: Vec<usize> = v.rules.iter().map(|r| r.n).collect();
            prop_assert_eq!(ns, (1..=v.rules.len()).collect::<Vec<_>>());
        }

        #[test]
        fn parsing_never_panics(line in ".{0,200}") {
            let _ = parse_choice(&line);
            let _ = is_domain(&line);
            let _ = serde_json::from_str::<TasteRecord>(&line);
        }
    }
}
