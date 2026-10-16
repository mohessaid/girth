//! `girth stats`: what the local ledger says about skills.

use std::collections::HashMap;

use serde::Serialize;

use crate::ledger::{Event, Rating, Record, Sharing};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SourceRow {
    pub skill: String,
    pub used: usize,
    /// 0 when the source skill was not installed.
    pub total: u32,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CraftedRow {
    pub skill: String,
    pub project: String,
    pub uses: u64,
    pub sections: usize,
    pub removed_total: usize,
    pub up: u64,
    pub down: u64,
    pub sources: Vec<SourceRow>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct UseRow {
    pub skill: String,
    pub uses: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Stats {
    pub events: usize,
    pub sharing: String,
    pub crafted: Vec<CraftedRow>,
    pub top_uses: Vec<UseRow>,
}

pub const TOP_USES: usize = 10;

pub fn sharing_label(sharing: Sharing) -> &'static str {
    match sharing {
        Sharing::Unset => "unset",
        Sharing::Allowed => "allowed",
        Sharing::Denied => "denied",
    }
}

/// `proj:girth-x` and `girth-x` both name the crafted skill `girth-x`.
fn base_name(skill: &str) -> &str {
    skill.rsplit(':').next().unwrap_or(skill)
}

pub fn compute(records: &[Record], sharing: Sharing) -> Stats {
    let mut crafted: Vec<CraftedRow> = Vec::new();
    let mut uses_by_skill: HashMap<&str, u64> = HashMap::new();
    for record in records {
        match &record.event {
            Event::Craft {
                skill,
                project,
                sections,
                sources,
                removed,
                ..
            } => {
                let index = match crafted
                    .iter()
                    .position(|r| &r.skill == skill && &r.project == project)
                {
                    Some(i) => i,
                    None => {
                        crafted.push(CraftedRow {
                            skill: skill.clone(),
                            project: project.clone(),
                            uses: 0,
                            sections: 0,
                            removed_total: 0,
                            up: 0,
                            down: 0,
                            sources: Vec::new(),
                        });
                        crafted.len() - 1
                    }
                };
                let row = &mut crafted[index];
                row.sections = sections.len();
                row.removed_total += removed.len();
                row.sources = sources
                    .iter()
                    .map(|s| SourceRow {
                        skill: s.skill.clone(),
                        used: s.used.len(),
                        total: s.total,
                    })
                    .collect();
            }
            Event::Use { skill, .. } => *uses_by_skill.entry(skill.as_str()).or_default() += 1,
            Event::Feedback { .. }
            | Event::Seen { .. }
            | Event::Turn(_)
            | Event::Ping(_)
            | Event::Compact(_) => {}
        }
    }
    for record in records {
        let row_for = |crafted: &mut Vec<CraftedRow>, skill: &str, project: &str| {
            crafted
                .iter_mut()
                .position(|r| r.skill == base_name(skill) && r.project == project)
        };
        match &record.event {
            Event::Use { skill, project } => {
                if let Some(i) = row_for(&mut crafted, skill, project) {
                    crafted[i].uses += 1;
                }
            }
            Event::Feedback {
                skill,
                project,
                rating,
                ..
            } => {
                if let Some(i) = row_for(&mut crafted, skill, project) {
                    match rating {
                        Rating::Up => crafted[i].up += 1,
                        Rating::Down => crafted[i].down += 1,
                    }
                }
            }
            Event::Craft { .. }
            | Event::Seen { .. }
            | Event::Turn(_)
            | Event::Ping(_)
            | Event::Compact(_) => {}
        }
    }
    crafted.sort_by(|a, b| b.uses.cmp(&a.uses).then_with(|| a.skill.cmp(&b.skill)));
    let mut top_uses: Vec<UseRow> = uses_by_skill
        .into_iter()
        .map(|(skill, uses)| UseRow {
            skill: skill.to_string(),
            uses,
        })
        .collect();
    top_uses.sort_by(|a, b| b.uses.cmp(&a.uses).then_with(|| a.skill.cmp(&b.skill)));
    top_uses.truncate(TOP_USES);
    Stats {
        events: records.len(),
        sharing: sharing_label(sharing).to_string(),
        crafted,
        top_uses,
    }
}

fn coverage(source: &SourceRow) -> String {
    if source.total == 0 {
        format!("{} {}/n/a", source.skill, source.used)
    } else {
        let percent = source.used * 100 / source.total as usize;
        format!(
            "{} {}/{} ({percent}%)",
            source.skill, source.used, source.total
        )
    }
}

pub fn render(stats: &Stats) -> String {
    if stats.events == 0 {
        return format!(
            "Girth ledger is empty. It fills up as you craft and use skills.\nsharing: {}",
            stats.sharing
        );
    }
    let mut lines = vec![format!(
        "Girth ledger · {} events · sharing: {}",
        stats.events, stats.sharing
    )];
    if !stats.crafted.is_empty() {
        lines.push(String::new());
        lines.push("Crafted skills".into());
        for row in &stats.crafted {
            lines.push(format!("  {}  ({})", row.skill, row.project));
            lines.push(format!(
                "    uses {} · {} sections · {} removed · 👍 {} 👎 {}",
                row.uses, row.sections, row.removed_total, row.up, row.down
            ));
            if !row.sources.is_empty() {
                let sources: Vec<String> = row.sources.iter().map(coverage).collect();
                lines.push(format!("    from {}", sources.join(" · ")));
            }
        }
    }
    if !stats.top_uses.is_empty() {
        lines.push(String::new());
        lines.push("Most used skills".into());
        let uses: Vec<String> = stats
            .top_uses
            .iter()
            .map(|u| format!("{} {}", u.skill, u.uses))
            .collect();
        lines.push(format!("  {}", uses.join(" · ")));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{CraftSection, RemovedSection, SourceUse};

    fn rec(event: Event) -> Record {
        Record { v: 1, ts: 1, event }
    }

    fn use_(skill: &str, project: &str) -> Record {
        rec(Event::Use {
            skill: skill.into(),
            project: project.into(),
        })
    }

    fn craft(
        skill: &str,
        project: &str,
        sections: usize,
        removed: usize,
        sources: Vec<(&str, usize, u32)>,
    ) -> Record {
        rec(Event::Craft {
            skill: skill.into(),
            project: project.into(),
            sections: (0..sections)
                .map(|i| CraftSection {
                    heading: format!("h{i}"),
                    hash: format!("{i:016x}"),
                    src: vec![],
                })
                .collect(),
            sources: sources
                .into_iter()
                .map(|(skill, used, total)| SourceUse {
                    skill: skill.into(),
                    total,
                    used: (0..used).map(|i| format!("s{i}")).collect(),
                    hashes: Default::default(),
                    fingerprint: String::new(),
                })
                .collect(),
            added: vec![],
            removed: (0..removed)
                .map(|i| RemovedSection {
                    hash: format!("r{i}"),
                    src: vec![],
                })
                .collect(),
            body_bytes: 10,
        })
    }

    fn feedback(skill: &str, project: &str, rating: Rating) -> Record {
        rec(Event::Feedback {
            skill: skill.into(),
            project: project.into(),
            rating,
            note: None,
        })
    }

    #[test]
    fn crafted_rows_use_latest_craft_and_accumulate_removals_uses_and_ratings() {
        let records = vec![
            craft("girth-captions", "/p", 4, 0, vec![("video-mega", 2, 8)]),
            use_("girth-captions", "/p"),
            use_("girth-captions", "/p"),
            use_("girth-captions", "/other"),
            craft(
                "girth-captions",
                "/p",
                3,
                1,
                vec![("video-mega", 1, 8), ("ghost", 1, 0)],
            ),
            feedback("girth-captions", "/p", Rating::Up),
            feedback("girth-captions", "/p", Rating::Down),
            feedback("girth-captions", "/p", Rating::Up),
        ];
        let stats = compute(&records, Sharing::Unset);
        assert_eq!(stats.events, 8);
        assert_eq!(stats.sharing, "unset");
        assert_eq!(stats.crafted.len(), 1);
        let row = &stats.crafted[0];
        assert_eq!(
            (row.uses, row.sections, row.removed_total, row.up, row.down),
            (2, 3, 1, 2, 1)
        );
        assert_eq!(
            row.sources,
            [
                SourceRow {
                    skill: "video-mega".into(),
                    used: 1,
                    total: 8
                },
                SourceRow {
                    skill: "ghost".into(),
                    used: 1,
                    total: 0
                }
            ]
        );
    }

    #[test]
    fn same_crafted_skill_in_two_projects_gets_two_rows() {
        let records = vec![
            craft("girth-x", "/p", 1, 0, vec![]),
            craft("girth-x", "/q", 2, 0, vec![]),
            craft("girth-y", "/p", 3, 0, vec![]),
        ];
        let rows = compute(&records, Sharing::Unset).crafted;
        let mut keys: Vec<(&str, &str, usize)> = rows
            .iter()
            .map(|r| (r.skill.as_str(), r.project.as_str(), r.sections))
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                ("girth-x", "/p", 1),
                ("girth-x", "/q", 2),
                ("girth-y", "/p", 3)
            ]
        );
    }

    #[test]
    fn namespaced_use_counts_for_the_crafted_skill() {
        let records = vec![
            craft("girth-x", "/p", 1, 0, vec![]),
            use_("proj:girth-x", "/p"),
        ];
        assert_eq!(compute(&records, Sharing::Unset).crafted[0].uses, 1);
    }

    #[test]
    fn top_uses_aggregate_across_projects_sorted_and_capped() {
        let mut records = vec![use_("b", "/1"), use_("a", "/1"), use_("a", "/2")];
        for i in 0..12 {
            records.push(use_(&format!("z{i:02}"), "/1"));
        }
        let top = compute(&records, Sharing::Allowed).top_uses;
        assert_eq!(top.len(), TOP_USES);
        assert_eq!((top[0].skill.as_str(), top[0].uses), ("a", 2));
        assert_eq!(top[1].skill, "b");
    }

    #[test]
    fn render_shows_rows_percentages_and_na_for_unknown_totals() {
        let records = vec![
            craft(
                "girth-captions",
                "/p",
                3,
                1,
                vec![("video-mega", 1, 8), ("ghost", 1, 0)],
            ),
            use_("girth-captions", "/p"),
            feedback("girth-captions", "/p", Rating::Up),
        ];
        let text = render(&compute(&records, Sharing::Denied));
        assert!(text.contains("sharing: denied"), "{text}");
        assert!(text.contains("girth-captions"), "{text}");
        assert!(
            text.contains("uses 1 · 3 sections · 1 removed · 👍 1 👎 0"),
            "{text}"
        );
        assert!(text.contains("video-mega 1/8 (12%)"), "{text}");
        assert!(text.contains("ghost 1/n/a"), "{text}");
    }

    #[test]
    fn empty_ledger_renders_a_friendly_message() {
        let text = render(&compute(&[], Sharing::Unset));
        assert!(text.contains("empty"), "{text}");
    }

    #[test]
    fn json_shape_is_frozen() {
        let json = serde_json::to_value(compute(&[], Sharing::Unset)).unwrap();
        let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["crafted", "events", "sharing", "top_uses"]);
    }
}
