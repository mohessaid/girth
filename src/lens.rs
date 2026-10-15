//! Lens: a per-project skill listing.
//!
//! Claude Code sends every installed skill's description on every request.
//! Lens keeps the descriptions this project needs and collapses the rest into
//! one line of names, so every skill stays invocable and none is uninstalled.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::ledger::{Event, Record};

/// The line that carries the collapsed skills. Frozen: SKILL.md refers to it.
pub const COLLAPSE_PREFIX: &str =
    "- Also installed (short hints; ask `girth search \"<need>\"` which fits): ";
/// A collapsed skill keeps at most this many characters of its description:
/// names alone lose routing (SkillRouter), whole descriptions are the cost.
pub const HINT_CHARS: usize = 60;
/// A skill first seen this recently (and not in the baseline) keeps its description.
pub const NEW_SECS: u64 = 14 * 24 * 3600;
/// A skill used in any project this recently keeps its description.
pub const RECENT_SECS: u64 = 30 * 24 * 3600;

/// One piece of a listing, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// `- name: description` plus any continuation lines, exactly as received.
    Entry { name: String, text: String },
    /// Anything else (the header, blank lines), exactly as received.
    Raw(String),
}

/// What Lens needs to know besides the listing.
pub struct Inputs<'a> {
    pub records: &'a [Record],
    /// The project path as the ledger records it.
    pub project: &'a str,
    /// The project directory, to find project skills.
    pub project_dir: &'a Path,
    /// Pinned names (`lens.keep`).
    pub keep: &'a [String],
    pub now: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    pub text: String,
    pub described: Vec<String>,
    pub collapsed: Vec<String>,
}

/// The skill name an entry line starts, if it starts one: `- name: …` or `- name`.
fn entry_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("- ")?;
    let rest = rest.trim_end_matches(['\n', '\r']);
    let name = rest.find(": ").map_or(rest, |i| &rest[..i]);
    (!name.is_empty() && !name.chars().any(char::is_whitespace)).then_some(name)
}

/// Split a listing into entries and raw lines, keeping every byte. Non-blank
/// lines after an entry continue it; a blank line ends it.
pub fn parse(text: &str) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for line in text.split_inclusive('\n') {
        if let Some(name) = entry_name(line) {
            items.push(Item::Entry {
                name: name.to_string(),
                text: line.to_string(),
            });
            continue;
        }
        match items.last_mut() {
            Some(Item::Entry { text, .. }) if !line.trim().is_empty() => text.push_str(line),
            _ => items.push(Item::Raw(line.to_string())),
        }
    }
    items
}

/// Prefix of server names in `seen` events; skills have none.
pub const SERVER_PREFIX: &str = "mcp:";

/// Names first seen within this long of a namespace's first sighting belong to
/// its baseline: one session can report skills and servers in several texts.
pub const BASELINE_WINDOW_SECS: u64 = 24 * 3600;

/// For one namespace (servers when `servers`, else skills): when the first
/// `seen` event holding any of its names happened (the baseline), and when each
/// name was first seen. Namespaces get separate baselines, so servers recorded
/// for the first time later never make every skill look new, or the reverse.
pub(crate) fn first_seen(records: &[Record], servers: bool) -> (Option<u64>, HashMap<&str, u64>) {
    let mut baseline = None;
    let mut first = HashMap::new();
    for record in records {
        if let Event::Seen { names } = &record.event {
            for name in names
                .iter()
                .filter(|n| n.starts_with(SERVER_PREFIX) == servers)
            {
                baseline.get_or_insert(record.ts);
                first.entry(name.as_str()).or_insert(record.ts);
            }
        }
    }
    (baseline, first)
}

/// Whether `name` arrived after its namespace's baseline window and recently.
pub(crate) fn is_new(baseline: Option<u64>, first: Option<&u64>, now: u64) -> bool {
    let Some(start) = baseline else {
        // No baseline yet: this run is the baseline.
        return false;
    };
    let arrived = first.copied().unwrap_or(now);
    arrived > start + BASELINE_WINDOW_SECS && now.saturating_sub(arrived) < NEW_SECS
}

/// Names in `names` that no `seen` event has recorded yet.
pub fn unseen(records: &[Record], names: &[String]) -> Vec<String> {
    let (_, skills) = first_seen(records, false);
    let (_, servers) = first_seen(records, true);
    names
        .iter()
        .filter(|n| !skills.contains_key(n.as_str()) && !servers.contains_key(n.as_str()))
        .cloned()
        .collect()
}

fn base(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// A short hint from an entry's description: its first line without a leading
/// "Use when", cut at a word boundary within `HINT_CHARS`.
fn hint(entry: &str) -> Option<String> {
    let first = entry.lines().next()?;
    let (_, description) = first.split_once(": ")?;
    let description = description.trim();
    let description = description.strip_prefix("Use when ").unwrap_or(description);
    if description.is_empty() {
        return None;
    }
    if description.chars().count() <= HINT_CHARS {
        return Some(description.to_string());
    }
    let cut: String = description.chars().take(HINT_CHARS).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > 0 => cut[..i].trim_end().to_string(),
        _ => cut,
    };
    Some(format!("{cut}…"))
}

/// Rewrite `text` for this project. Returns it unchanged when there is no
/// listing to parse or fewer than two entries would be collapsed.
pub fn rewrite(text: &str, inputs: &Inputs) -> Rewrite {
    let unchanged = || Rewrite {
        text: text.to_string(),
        described: Vec::new(),
        collapsed: Vec::new(),
    };
    let items = parse(text);
    let (baseline, first) = first_seen(inputs.records, false);
    let mut used_here = HashSet::new();
    let mut used_recently = HashSet::new();
    for record in inputs.records {
        if let Event::Use { skill, project } = &record.event {
            if project == inputs.project {
                used_here.insert(base(skill));
            }
            if inputs.now.saturating_sub(record.ts) < RECENT_SECS {
                used_recently.insert(base(skill));
            }
        }
    }
    let keep = |name: &str| {
        let b = base(name);
        b == "girth"
            || b.starts_with("girth-")
            || inputs.keep.iter().any(|k| k == name || k == b)
            || used_here.contains(b)
            || used_recently.contains(b)
            || is_new(baseline, first.get(name), inputs.now)
            || inputs
                .project_dir
                .join(".claude/skills")
                .join(b)
                .join("SKILL.md")
                .is_file()
    };
    let (mut described, mut collapsed, mut shown) = (Vec::new(), Vec::new(), Vec::new());
    let mut last_entry = None;
    for (i, item) in items.iter().enumerate() {
        if let Item::Entry { name, text } = item {
            last_entry = Some(i);
            if keep(name) {
                described.push(name.clone());
            } else {
                collapsed.push(name.clone());
                shown.push(match hint(text) {
                    Some(h) => format!("{name} ({h})"),
                    None => name.clone(),
                });
            }
        }
    }
    let Some(last_entry) = last_entry else {
        return unchanged();
    };
    if collapsed.len() < 2 {
        return Rewrite {
            described,
            ..unchanged()
        };
    }
    let mut out = String::with_capacity(text.len());
    for (i, item) in items.iter().enumerate() {
        match item {
            Item::Entry { name, text } if described.contains(name) => out.push_str(text),
            Item::Entry { .. } => {}
            Item::Raw(raw) => out.push_str(raw),
        }
        if i == last_entry {
            out.push_str(COLLAPSE_PREFIX);
            out.push_str(&shown.join("; "));
            out.push('\n');
        }
    }
    Rewrite {
        text: out,
        described,
        collapsed,
    }
}

/// `lens` settings from the project's `.girth/config.json` and `GIRTH_HOME/config.json`:
/// off when either sets `enabled: false`; `keep` lists from both.
pub fn settings(project_dir: &Path, home: &Path) -> (bool, Vec<String>) {
    let mut enabled = true;
    let mut keep = Vec::new();
    for file in [
        project_dir.join(crate::config::CONFIG_PATH),
        home.join(crate::ledger::CONFIG_FILE),
    ] {
        let lens = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.get("lens").cloned());
        let Some(lens) = lens else { continue };
        if lens.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
            enabled = false;
        }
        let names = lens
            .get("keep")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten();
        keep.extend(names.filter_map(|v| v.as_str()).map(str::to_string));
    }
    (enabled, keep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::Event;
    use proptest::prelude::*;

    const NOW: u64 = 1_800_000_000;
    const DAY: u64 = 24 * 3600;

    const LISTING: &str = "\
The following skills are available for use with the Skill tool:

- beat-synced-sfx: Use when a video needs sound effects.
- claude-api: Reference for the Claude API.
TRIGGER — read BEFORE opening the target file
SKIP only when another provider is used.
- girth:girth: Use when a task needs specialised know-how.
- girth-captions: Use when captions.
- superpowers:brainstorming: You MUST use this before any creative work.
- hyperframes: Mandatory entry point for video.
- anthropic-skills:pdf
- anthropic-skills:xlsx
";

    fn rec(ts: u64, event: Event) -> Record {
        Record { v: 1, ts, event }
    }

    fn used(ts: u64, skill: &str, project: &str) -> Record {
        rec(
            ts,
            Event::Use {
                skill: skill.into(),
                project: project.into(),
            },
        )
    }

    fn seen(ts: u64, names: &[&str]) -> Record {
        rec(
            ts,
            Event::Seen {
                names: names.iter().map(|n| n.to_string()).collect(),
            },
        )
    }

    fn names(items: &[Item]) -> Vec<&str> {
        items
            .iter()
            .filter_map(|i| match i {
                Item::Entry { name, .. } => Some(name.as_str()),
                Item::Raw(_) => None,
            })
            .collect()
    }

    fn inputs<'a>(records: &'a [Record], keep: &'a [String], dir: &'a Path) -> Inputs<'a> {
        Inputs {
            records,
            project: "/work/app",
            project_dir: dir,
            keep,
            now: NOW,
        }
    }

    #[test]
    fn parse_keeps_every_byte_and_attaches_continuation_lines() {
        let items = parse(LISTING);
        assert_eq!(
            names(&items),
            [
                "beat-synced-sfx",
                "claude-api",
                "girth:girth",
                "girth-captions",
                "superpowers:brainstorming",
                "hyperframes",
                "anthropic-skills:pdf",
                "anthropic-skills:xlsx",
            ]
        );
        let joined: String = items
            .iter()
            .map(|i| match i {
                Item::Entry { text, .. } | Item::Raw(text) => text.as_str(),
            })
            .collect();
        assert_eq!(joined, LISTING);
        let Item::Entry { text, .. } = &items[3] else {
            panic!("{items:?}")
        };
        assert!(
            text.contains("TRIGGER") && text.contains("SKIP only"),
            "{text}"
        );
        assert_eq!(
            items[0],
            Item::Raw("The following skills are available for use with the Skill tool:\n".into())
        );
    }

    #[test]
    fn entry_names_need_a_dash_and_no_spaces() {
        assert_eq!(entry_name("- a-b: desc"), Some("a-b"));
        assert_eq!(entry_name("- plug:a: desc: more"), Some("plug:a"));
        assert_eq!(entry_name("- plug:a\n"), Some("plug:a"));
        assert_eq!(entry_name("- two words: x"), None);
        assert_eq!(entry_name("-x: y"), None);
        assert_eq!(entry_name("  - a: indented"), None);
        assert_eq!(entry_name("- : empty"), None);
    }

    #[test]
    fn keeps_girth_crafted_used_recent_new_project_and_pinned_skills() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".claude/skills/hyperframes")).unwrap();
        std::fs::write(tmp.path().join(".claude/skills/hyperframes/SKILL.md"), "x").unwrap();
        let records = vec![
            // Baseline: every name but brainstorming existed at Lens's first run.
            seen(
                NOW - 60 * DAY,
                &[
                    "beat-synced-sfx",
                    "claude-api",
                    "girth:girth",
                    "girth-captions",
                    "hyperframes",
                    "anthropic-skills:pdf",
                    "anthropic-skills:xlsx",
                ],
            ),
            // brainstorming arrived 3 days ago: new.
            seen(NOW - 3 * DAY, &["superpowers:brainstorming"]),
            // claude-api used in another project 10 days ago: recent.
            used(NOW - 10 * DAY, "claude-api", "/elsewhere"),
            // beat-synced-sfx used here long ago: still kept for this project.
            used(NOW - 90 * DAY, "beat-synced-sfx", "/work/app"),
        ];
        let keep = vec!["anthropic-skills:xlsx".to_string()];
        let out = rewrite(LISTING, &inputs(&records, &keep, tmp.path()));
        assert_eq!(
            out.described,
            [
                "beat-synced-sfx",
                "claude-api",
                "girth:girth",
                "girth-captions",
                "superpowers:brainstorming",
                "hyperframes",
                "anthropic-skills:xlsx"
            ]
        );
        // Only one would collapse: below the threshold, so unchanged.
        assert_eq!(out.text, LISTING);
        assert!(out.collapsed.is_empty());
    }

    #[test]
    fn collapses_the_rest_into_one_line_of_names() {
        let tmp = tempfile::TempDir::new().unwrap();
        let records = vec![seen(
            NOW - 60 * DAY,
            &[
                "beat-synced-sfx",
                "claude-api",
                "girth:girth",
                "girth-captions",
                "superpowers:brainstorming",
                "hyperframes",
                "anthropic-skills:pdf",
                "anthropic-skills:xlsx",
            ],
        )];
        let out = rewrite(LISTING, &inputs(&records, &[], tmp.path()));
        assert_eq!(out.described, ["girth:girth", "girth-captions"]);
        assert_eq!(
            out.collapsed,
            [
                "beat-synced-sfx",
                "claude-api",
                "superpowers:brainstorming",
                "hyperframes",
                "anthropic-skills:pdf",
                "anthropic-skills:xlsx"
            ]
        );
        assert_eq!(
            out.text,
            format!(
                "The following skills are available for use with the Skill tool:\n\n\
                 - girth:girth: Use when a task needs specialised know-how.\n\
                 - girth-captions: Use when captions.\n\
                 {COLLAPSE_PREFIX}beat-synced-sfx (a video needs sound effects.); claude-api (Reference for the Claude API.); superpowers:brainstorming (You MUST use this before any creative work.); hyperframes (Mandatory entry point for video.); anthropic-skills:pdf; anthropic-skills:xlsx\n"
            )
        );
    }

    #[test]
    fn hints_are_short_word_bounded_and_drop_use_when() {
        assert_eq!(
            hint("- a: Use when a video needs sound.\n"),
            Some("a video needs sound.".into())
        );
        assert_eq!(hint("- a\n"), None);
        assert_eq!(hint("- a: \n"), None);
        let long = "- a: SEO and AEO best practices for metadata, Open Graph, sitemaps, robots.txt, hreflang.\nmore\n";
        let h = hint(long).unwrap();
        assert_eq!(h, "SEO and AEO best practices for metadata, Open Graph,…");
        assert!(h.chars().count() <= HINT_CHARS + 1);
        // No word boundary: cut at the limit.
        let word = format!("- a: {}\n", "x".repeat(100));
        assert_eq!(hint(&word).unwrap().chars().count(), HINT_CHARS + 1);
    }

    #[test]
    fn on_the_first_run_everything_is_baseline_not_new() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No seen events at all: nothing counts as new.
        let out = rewrite(LISTING, &inputs(&[], &[], tmp.path()));
        assert_eq!(out.described, ["girth:girth", "girth-captions"]);
        // Once a baseline exists, an unrecorded name is new.
        let records = vec![seen(NOW - 60 * DAY, &["claude-api"])];
        let out = rewrite(LISTING, &inputs(&records, &[], tmp.path()));
        assert!(
            out.described.contains(&"hyperframes".to_string()),
            "{out:?}"
        );
        assert!(
            !out.described.contains(&"claude-api".to_string()),
            "{out:?}"
        );
    }

    #[test]
    fn the_baseline_window_is_a_full_day() {
        let start = NOW - 60 * DAY;
        let new = |arrived: u64| is_new(Some(start), Some(&arrived), arrived + 60);
        // Within the day after the baseline (even 23 h later): still baseline.
        assert!(!new(start + 23 * 3600));
        // Exactly at the window's end: baseline; one second later: new.
        assert!(!new(start + BASELINE_WINDOW_SECS));
        assert!(new(start + BASELINE_WINDOW_SECS + 1));
    }

    #[test]
    fn server_names_in_seen_events_do_not_touch_the_skill_baseline() {
        let tmp = tempfile::TempDir::new().unwrap();
        let all: Vec<String> = names(&parse(LISTING))
            .iter()
            .map(|s| s.to_string())
            .collect();
        let all: Vec<&str> = all.iter().map(String::as_str).collect();
        // Servers were recorded first; the skills baseline is still the first skills event.
        // The skills' first sighting is recent: if the server event counted as the
        // baseline, every skill would look new and keep its description.
        let records = vec![
            seen(NOW - 90 * DAY, &["mcp:claude_ai_Figma"]),
            seen(NOW - 3 * DAY, &all),
        ];
        let out = rewrite(LISTING, &inputs(&records, &[], tmp.path()));
        assert_eq!(out.described, ["girth:girth", "girth-captions"]);
        assert_eq!(
            unseen(&records, &["mcp:claude_ai_Figma".to_string()]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn old_uses_elsewhere_and_old_arrivals_do_not_keep_a_description() {
        let tmp = tempfile::TempDir::new().unwrap();
        let records = vec![
            seen(NOW - 60 * DAY, &["claude-api"]),
            seen(NOW - 20 * DAY, &["hyperframes"]),
            used(NOW - 40 * DAY, "claude-api", "/elsewhere"),
        ];
        let all: Vec<String> = names(&parse(LISTING))
            .iter()
            .map(|s| s.to_string())
            .collect();
        let records = [
            records,
            vec![seen(
                NOW - 59 * DAY,
                &all.iter().map(String::as_str).collect::<Vec<_>>(),
            )],
        ]
        .concat();
        let out = rewrite(LISTING, &inputs(&records, &[], tmp.path()));
        assert!(out.collapsed.contains(&"claude-api".to_string()));
        assert!(out.collapsed.contains(&"hyperframes".to_string()));
    }

    #[test]
    fn a_blank_line_ends_an_entry_and_survives_a_collapse() {
        let items = parse("H:\n- a: A.\n\n- b: B.\n");
        assert_eq!(items[2], Item::Raw("\n".into()));
        let tmp = tempfile::TempDir::new().unwrap();
        let text = "H:\n- girth:girth: G.\n- a: A.\n\n- b: B.\n";
        let out = rewrite(text, &inputs(&[], &[], tmp.path()));
        assert_eq!(
            out.text,
            format!("H:\n- girth:girth: G.\n\n{COLLAPSE_PREFIX}a (A.); b (B.)\n")
        );
    }

    #[test]
    fn windows_are_strict_and_two_collapsed_entries_are_enough() {
        let tmp = tempfile::TempDir::new().unwrap();
        let text = "H:\n- a: A.\n- b: B.\n- c: C.\n";
        let records = vec![
            seen(NOW - 60 * DAY, &["a"]),
            // Exactly at the window edges: neither recent nor new.
            used(NOW - RECENT_SECS, "a", "/elsewhere"),
            seen(NOW - NEW_SECS, &["b", "c"]),
        ];
        let out = rewrite(text, &inputs(&records, &[], tmp.path()));
        assert_eq!(out.collapsed, ["a", "b", "c"]);
        // One second inside each window keeps the description.
        let records = vec![
            seen(NOW - 60 * DAY, &["a"]),
            used(NOW - RECENT_SECS + 1, "a", "/elsewhere"),
            seen(NOW - NEW_SECS + 1, &["b"]),
            seen(NOW - 60 * DAY, &["c"]),
        ];
        let out = rewrite(text, &inputs(&records, &[], tmp.path()));
        assert_eq!(out.described, ["a", "b"]);
        // Exactly two collapsed entries still rewrite.
        let records = vec![
            seen(NOW - 60 * DAY, &["a", "b", "c"]),
            used(NOW - DAY, "a", "/work/app"),
        ];
        let out = rewrite(text, &inputs(&records, &[], tmp.path()));
        assert_eq!(out.collapsed, ["b", "c"]);
        assert!(out.text.contains(COLLAPSE_PREFIX));
    }

    #[test]
    fn text_without_entries_passes_through() {
        let tmp = tempfile::TempDir::new().unwrap();
        for text in ["", "no listing here\n", "header only:\n\n"] {
            let out = rewrite(text, &inputs(&[], &[], tmp.path()));
            assert_eq!(out.text, text);
            assert!(out.described.is_empty() && out.collapsed.is_empty());
        }
    }

    #[test]
    fn unseen_lists_names_no_seen_event_has() {
        let records = vec![seen(1, &["a", "b"]), seen(2, &["c"])];
        let names = vec![
            "a".to_string(),
            "d".to_string(),
            "c".to_string(),
            "e".to_string(),
        ];
        assert_eq!(unseen(&records, &names), ["d", "e"]);
    }

    fn arb_listing() -> impl Strategy<Value = String> {
        let entry = (
            "[a-z]{1,6}(:[a-z]{1,6})?",
            prop::option::of("[a-z ]{0,20}"),
            prop::collection::vec("[A-Z][a-z :-]{0,15}", 0..2),
        )
            .prop_map(|(name, desc, cont)| {
                let mut s = match desc {
                    Some(d) => format!("- {name}: {d}\n"),
                    None => format!("- {name}\n"),
                };
                for c in cont {
                    s.push_str(&c);
                    s.push('\n');
                }
                s
            });
        prop::collection::vec(entry, 0..12).prop_map(|es| format!("Header:\n\n{}", es.concat()))
    }

    proptest! {
        #[test]
        fn never_loses_a_name_and_keeps_described_entries_verbatim(text in arb_listing(), used_mask in any::<u16>()) {
            let tmp = tempfile::TempDir::new().unwrap();
            let items = parse(&text);
            let all: Vec<String> = names(&items).iter().map(|s| s.to_string()).collect();
            let mut records = vec![seen(NOW - 60 * DAY, &all.iter().map(String::as_str).collect::<Vec<_>>())];
            for (i, n) in all.iter().enumerate() {
                if used_mask & (1 << (i % 16)) != 0 {
                    records.push(used(NOW - DAY, n, "/work/app"));
                }
            }
            let out = rewrite(&text, &inputs(&records, &[], tmp.path()));
            let mut got: Vec<String> = out.described.iter().chain(&out.collapsed).cloned().collect();
            got.sort();
            let mut want = all.clone();
            want.sort();
            if !out.collapsed.is_empty() {
                prop_assert_eq!(got, want);
            }
            for item in &items {
                if let Item::Entry { name, text: entry } = item {
                    if out.described.contains(name) {
                        prop_assert!(out.text.contains(entry.as_str()));
                    }
                    prop_assert!(out.text.contains(name.as_str()));
                }
            }
            prop_assert!(out.text.len() <= text.len() + 200);
        }

        #[test]
        fn parse_never_panics_and_round_trips(text in "(- ?[a-z: ]{0,10}\n|[ -~]{0,20}\n){0,20}") {
            let joined: String = parse(&text)
                .iter()
                .map(|i| match i {
                    Item::Entry { text, .. } | Item::Raw(text) => text.as_str(),
                })
                .collect();
            prop_assert_eq!(joined, text);
        }
    }
}
