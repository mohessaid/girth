//! The anonymous share payload: source-section hashes and counts only.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

use crate::ledger::{Event, Rating, Record, SourceUse};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SectionStat {
    pub h: String,
    /// Crafted skills that ever used this source section.
    pub used: u64,
    /// Crafted skills whose latest version still uses it.
    pub kept: u64,
    /// Times it was removed from a crafted skill.
    pub removed: u64,
    pub up: u64,
    pub down: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SkillStat {
    pub h: String,
    pub total: u32,
    /// Distinct sections of this skill used by any crafted skill.
    pub used: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SharePayload {
    pub girth: String,
    pub sections: Vec<SectionStat>,
    pub skills: Vec<SkillStat>,
}

#[derive(Default)]
struct Counts {
    used: BTreeSet<(String, String)>,
    kept: u64,
    removed: u64,
    up: u64,
    down: u64,
}

fn source_hashes(sources: &[SourceUse]) -> impl Iterator<Item = &String> {
    sources.iter().flat_map(|s| s.hashes.values())
}

/// `proj:girth-x` and `girth-x` both name the crafted skill `girth-x`.
fn base_name(skill: &str) -> &str {
    skill.rsplit(':').next().unwrap_or(skill)
}

pub fn payload(records: &[Record]) -> SharePayload {
    let mut counts: BTreeMap<String, Counts> = BTreeMap::new();
    let mut src_to_hash: HashMap<String, String> = HashMap::new();
    let mut latest: HashMap<(String, String), &[SourceUse]> = HashMap::new();
    let mut skills: BTreeMap<String, (u32, BTreeSet<String>)> = BTreeMap::new();

    for record in records {
        let Event::Craft {
            skill,
            project,
            sources,
            ..
        } = &record.event
        else {
            continue;
        };
        let key = (project.clone(), skill.clone());
        for source in sources {
            for (slug, hash) in &source.hashes {
                src_to_hash.insert(format!("{}#{slug}", source.skill), hash.clone());
                counts
                    .entry(hash.clone())
                    .or_default()
                    .used
                    .insert(key.clone());
            }
            if !source.fingerprint.is_empty() {
                let entry = skills.entry(source.fingerprint.clone()).or_default();
                entry.0 = source.total;
                entry.1.extend(source.used.iter().cloned());
            }
        }
        latest.insert(key, sources);
    }
    for record in records {
        match &record.event {
            Event::Craft { removed, .. } => {
                for src in removed.iter().flat_map(|r| &r.src) {
                    if let Some(hash) = src_to_hash.get(src) {
                        counts.entry(hash.clone()).or_default().removed += 1;
                    }
                }
            }
            Event::Feedback {
                skill,
                project,
                rating,
                ..
            } => {
                let key = (project.clone(), base_name(skill).to_string());
                for hash in latest.get(&key).into_iter().flat_map(|s| source_hashes(s)) {
                    let c = counts.entry(hash.clone()).or_default();
                    match rating {
                        Rating::Up => c.up += 1,
                        Rating::Down => c.down += 1,
                    }
                }
            }
            Event::Use { .. } | Event::Seen { .. } | Event::Turn(_) => {}
        }
    }
    for sources in latest.values() {
        for hash in source_hashes(sources) {
            counts.entry(hash.clone()).or_default().kept += 1;
        }
    }
    SharePayload {
        girth: env!("CARGO_PKG_VERSION").to_string(),
        sections: counts
            .into_iter()
            .map(|(h, c)| SectionStat {
                h,
                used: c.used.len() as u64,
                kept: c.kept,
                removed: c.removed,
                up: c.up,
                down: c.down,
            })
            .collect(),
        skills: skills
            .into_iter()
            .map(|(h, (total, used))| SkillStat {
                h,
                total,
                used: used.len() as u64,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{CraftSection, Event, Rating, RemovedSection, SourceUse};
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    fn rec(event: Event) -> Record {
        Record { v: 1, ts: 1, event }
    }

    /// A craft of `skill` in `project` using `slugs` of source "vm" (fingerprint "fp"), removing `removed`.
    fn craft(skill: &str, project: &str, slugs: &[&str], removed: &[&str]) -> Record {
        let hashes: BTreeMap<String, String> = slugs
            .iter()
            .map(|s| (s.to_string(), format!("h-{s}")))
            .collect();
        rec(Event::Craft {
            skill: skill.into(),
            project: project.into(),
            sections: slugs
                .iter()
                .map(|s| CraftSection {
                    heading: format!("Heading {s}"),
                    hash: format!("c-{s}"),
                    src: vec![format!("vm#{s}")],
                })
                .collect(),
            sources: vec![SourceUse {
                skill: "vm".into(),
                total: 10,
                used: slugs.iter().map(|s| s.to_string()).collect(),
                hashes,
                fingerprint: "fp".into(),
            }],
            added: vec![],
            removed: removed
                .iter()
                .map(|s| RemovedSection {
                    hash: format!("c-{s}"),
                    src: vec![format!("vm#{s}")],
                })
                .collect(),
            body_bytes: 1,
        })
    }

    fn stat<'a>(p: &'a SharePayload, h: &str) -> &'a SectionStat {
        p.sections.iter().find(|s| s.h == h).unwrap()
    }

    #[test]
    fn aggregates_used_kept_removed_and_ratings_per_source_section() {
        let records = vec![
            craft("girth-a", "/p", &["timing", "export"], &[]),
            craft("girth-a", "/p", &["timing"], &["export"]),
            craft("girth-b", "/q", &["timing"], &[]),
            rec(Event::Feedback {
                skill: "girth-a".into(),
                project: "/p".into(),
                rating: Rating::Up,
                note: Some("secret note".into()),
            }),
            rec(Event::Feedback {
                skill: "girth-b".into(),
                project: "/q".into(),
                rating: Rating::Down,
                note: None,
            }),
        ];
        let p = payload(&records);
        assert_eq!(p.girth, env!("CARGO_PKG_VERSION"));
        let timing = stat(&p, "h-timing");
        assert_eq!(
            (
                timing.used,
                timing.kept,
                timing.removed,
                timing.up,
                timing.down
            ),
            (2, 2, 0, 1, 1)
        );
        let export = stat(&p, "h-export");
        assert_eq!(
            (
                export.used,
                export.kept,
                export.removed,
                export.up,
                export.down
            ),
            (1, 0, 1, 0, 0)
        );
        assert_eq!(
            p.skills,
            [SkillStat {
                h: "fp".into(),
                total: 10,
                used: 2
            }]
        );
    }

    #[test]
    fn sources_without_a_fingerprint_are_left_out_of_skills() {
        let mut record = craft("girth-a", "/p", &["timing"], &[]);
        if let Event::Craft { sources, .. } = &mut record.event {
            sources[0].fingerprint = String::new();
        }
        assert!(payload(&[record]).skills.is_empty());
    }

    #[test]
    fn output_is_sorted_by_hash() {
        let p = payload(&[craft("girth-a", "/p", &["zz", "aa", "mm"], &[])]);
        let hs: Vec<&str> = p.sections.iter().map(|s| s.h.as_str()).collect();
        assert_eq!(hs, ["h-aa", "h-mm", "h-zz"]);
    }

    proptest! {
        #[test]
        fn payload_never_contains_names_paths_headings_or_notes(
            skill in "zq[a-z]{4,10}",
            project in "/zq[a-z]{4,10}/[a-z]{3}",
            heading in "zq[A-Za-z ]{4,20}",
            note in "zq[a-z ]{4,30}",
        ) {
            let mut record = craft(&skill, &project, &["timing"], &[]);
            if let Event::Craft { sections, .. } = &mut record.event {
                sections[0].heading = heading.clone();
            }
            let records = vec![
                record,
                rec(Event::Use { skill: skill.clone(), project: project.clone() }),
                rec(Event::Feedback { skill: skill.clone(), project: project.clone(), rating: Rating::Up, note: Some(note.clone()) }),
                rec(Event::Seen { names: vec![skill.clone(), note.clone()] }),
                rec(Event::Turn(crate::ledger::Turn {
                    project: project.clone(), model: heading.clone(), input: 1, output: 1,
                    cache_read: 0, cache_write: 0, agent: false, aborted: false,
                    lens_before: None, lens_after: None, requests: None, mcp: vec![skill.clone()],
                })),
            ];
            let json = serde_json::to_string(&payload(&records)).unwrap();
            for secret in [&skill, &project, &heading, &note, &"vm#timing".to_string(), &"c-timing".to_string()] {
                prop_assert!(!json.contains(secret.as_str()), "leaked {secret} in {json}");
            }
        }
    }
}
