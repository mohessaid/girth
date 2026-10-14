//! Building `craft` events: section provenance, source coverage and diffs.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use crate::frontmatter::split_frontmatter;
use crate::ledger::{CraftSection, Event, Record, RemovedSection, SourceUse};
use crate::sections::{Section, split_sections};
use crate::skill::SkillInfo;

/// The most recent `craft` event for (project, skill).
pub fn last_craft<'a>(records: &'a [Record], project: &str, skill: &str) -> Option<&'a Event> {
    records.iter().rev().map(|r| &r.event).find(
        |e| matches!(e, Event::Craft { skill: s, project: p, .. } if s == skill && p == project),
    )
}

fn body_sections(skill_md: &Path) -> Option<(String, Vec<Section>)> {
    let bytes = std::fs::read(skill_md).ok()?;
    let (_, body) = split_frontmatter(&String::from_utf8_lossy(&bytes));
    let sections = split_sections(&body);
    Some((body, sections))
}

/// Group `src` entries (`skill#slug`) by skill, keeping first-seen order.
fn referenced_sources(sections: &[Section]) -> Vec<(String, Vec<String>)> {
    let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
    for entry in sections.iter().flat_map(|s| &s.src) {
        let (skill, slug) = entry.split_once('#').unwrap_or((entry.as_str(), ""));
        let index = match grouped.iter().position(|(name, _)| name == skill) {
            Some(i) => i,
            None => {
                grouped.push((skill.to_string(), Vec::new()));
                grouped.len() - 1
            }
        };
        let slugs = &mut grouped[index].1;
        if !slug.is_empty() && !slugs.iter().any(|s| s == slug) {
            slugs.push(slug.to_string());
        }
    }
    grouped
}

fn source_use(skill: String, used: Vec<String>, installed: &[SkillInfo]) -> SourceUse {
    let source_sections = installed
        .iter()
        .find(|s| s.name == skill)
        .and_then(|s| body_sections(&s.path.join("SKILL.md")))
        .map(|(_, sections)| sections)
        .unwrap_or_default();
    let hashes: BTreeMap<String, String> = source_sections
        .iter()
        .filter(|s| used.contains(&s.slug))
        .map(|s| (s.slug.clone(), s.hash.clone()))
        .collect();
    let all: Vec<String> = source_sections.iter().map(|s| s.hash.clone()).collect();
    let fingerprint = if all.is_empty() {
        String::new()
    } else {
        crate::sections::fingerprint(&all)
    };
    SourceUse {
        skill,
        total: source_sections.len() as u32,
        used,
        hashes,
        fingerprint,
    }
}

/// A `craft` event for the crafted skill in `skill_dir`, diffed against `previous`.
pub fn craft_event(
    skill_dir: &Path,
    project: &str,
    installed: &[SkillInfo],
    previous: Option<&Event>,
) -> Event {
    let (body, sections) = body_sections(&skill_dir.join("SKILL.md")).unwrap_or_default();
    let previous_sections: &[CraftSection] = match previous {
        Some(Event::Craft { sections, .. }) => sections,
        _ => &[],
    };
    let current: HashSet<&str> = sections.iter().map(|s| s.hash.as_str()).collect();
    let before: HashSet<&str> = previous_sections.iter().map(|s| s.hash.as_str()).collect();
    let added = sections
        .iter()
        .filter(|s| !before.contains(s.hash.as_str()))
        .map(|s| s.hash.clone())
        .collect();
    let removed = previous_sections
        .iter()
        .filter(|s| !current.contains(s.hash.as_str()))
        .map(|s| RemovedSection {
            hash: s.hash.clone(),
            src: s.src.clone(),
        })
        .collect();
    let sources = referenced_sources(&sections)
        .into_iter()
        .map(|(skill, used)| source_use(skill, used, installed))
        .collect();
    Event::Craft {
        skill: skill_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        project: project.to_string(),
        sections: sections
            .into_iter()
            .map(|s| CraftSection {
                heading: s.heading,
                hash: s.hash,
                src: s.src,
            })
            .collect(),
        sources,
        added,
        removed,
        body_bytes: body.len() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{RemovedSection, SourceUse};
    use crate::sections::split_sections;
    use crate::skill::discover_skills;
    use crate::testutil::write_skill;
    use tempfile::TempDir;

    const SOURCE_BODY: &str = "# Video\n## Color grading\nUse a LUT.\n## Caption timing\nNever more than 2 lines.\n## Export\nH.264\n";

    fn setup(crafted_body: &str) -> (TempDir, std::path::PathBuf, Vec<SkillInfo>) {
        let tmp = TempDir::new().unwrap();
        write_skill(
            &tmp.path().join("src"),
            "video-mega",
            "name: video-mega\ndescription: d",
            SOURCE_BODY,
            &[],
        );
        let crafted = write_skill(
            &tmp.path().join("out"),
            "girth-captions",
            "name: girth-captions\ndescription: d",
            crafted_body,
            &[],
        );
        let installed = discover_skills(&[tmp.path().join("src")]);
        (tmp, crafted, installed)
    }

    fn parts(
        event: &Event,
    ) -> (
        &Vec<crate::ledger::CraftSection>,
        &Vec<SourceUse>,
        &Vec<String>,
        &Vec<RemovedSection>,
    ) {
        match event {
            Event::Craft {
                sections,
                sources,
                added,
                removed,
                ..
            } => (sections, sources, added, removed),
            other => panic!("not a craft event: {other:?}"),
        }
    }

    #[test]
    fn first_craft_captures_sections_sources_and_adds_everything() {
        let body = "## Caption timing <!-- src: video-mega#caption-timing -->\nNever more than 2 lines.\n## House style\nOurs.\n";
        let (_tmp, dir, installed) = setup(body);
        let event = craft_event(&dir, "/proj", &installed, None);
        let (sections, sources, added, removed) = parts(&event);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].src, ["video-mega#caption-timing"]);
        assert!(sections[1].src.is_empty());
        assert_eq!(added.len(), 2);
        assert!(removed.is_empty());
        assert_eq!(sources.len(), 1);
        let source = &sources[0];
        assert_eq!((source.skill.as_str(), source.total), ("video-mega", 4));
        assert_eq!(source.used, ["caption-timing"]);
        let expected = split_sections(SOURCE_BODY)
            .into_iter()
            .find(|s| s.slug == "caption-timing")
            .unwrap()
            .hash;
        assert_eq!(source.hashes["caption-timing"], expected);
        let all: Vec<String> = split_sections(SOURCE_BODY)
            .into_iter()
            .map(|s| s.hash)
            .collect();
        assert_eq!(source.fingerprint, crate::sections::fingerprint(&all));
        match &event {
            Event::Craft {
                skill,
                project,
                body_bytes,
                ..
            } => {
                assert_eq!(
                    (skill.as_str(), project.as_str()),
                    ("girth-captions", "/proj")
                );
                assert_eq!(*body_bytes, body.len() as u64);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn unknown_source_records_zero_total_without_failing() {
        let (_tmp, dir, installed) = setup("## A <!-- src: ghost#a, video-mega#export -->\nx\n");
        let event = craft_event(&dir, "/p", &installed, None);
        let (_, sources, _, _) = parts(&event);
        let ghost = sources.iter().find(|s| s.skill == "ghost").unwrap();
        assert_eq!(
            (ghost.total, ghost.used.as_slice()),
            (0, ["a".to_string()].as_slice())
        );
        assert!(ghost.hashes.is_empty() && ghost.fingerprint.is_empty());
        assert_eq!(
            sources
                .iter()
                .find(|s| s.skill == "video-mega")
                .unwrap()
                .used,
            ["export"]
        );
    }

    #[test]
    fn identical_revalidate_is_plus_zero_minus_zero() {
        let (_tmp, dir, installed) = setup("## A <!-- src: video-mega#export -->\nx\n");
        let first = craft_event(&dir, "/p", &installed, None);
        let second = craft_event(&dir, "/p", &installed, Some(&first));
        let (_, _, added, removed) = parts(&second);
        assert!(added.is_empty() && removed.is_empty());
    }

    #[test]
    fn removing_a_section_records_its_hash_and_src() {
        let (_tmp, dir, installed) =
            setup("## Keep\nk\n## Drop <!-- src: video-mega#color-grading -->\nd\n");
        let first = craft_event(&dir, "/p", &installed, None);
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: girth-captions\ndescription: d\n---\n## Keep\nk\n",
        )
        .unwrap();
        let second = craft_event(&dir, "/p", &installed, Some(&first));
        let (_, _, added, removed) = parts(&second);
        assert!(added.is_empty());
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].src, ["video-mega#color-grading"]);
        let (first_sections, ..) = parts(&first);
        assert_eq!(removed[0].hash, first_sections[1].hash);
    }

    #[test]
    fn last_craft_picks_latest_for_project_and_skill() {
        let craft = |skill: &str, project: &str, body_bytes: u64| Record {
            v: 1,
            ts: 1,
            event: Event::Craft {
                skill: skill.into(),
                project: project.into(),
                sections: vec![],
                sources: vec![],
                added: vec![],
                removed: vec![],
                body_bytes,
            },
        };
        let records = vec![
            craft("girth-a", "/p", 1),
            craft("girth-a", "/other", 2),
            craft("girth-b", "/p", 3),
            craft("girth-a", "/p", 4),
        ];
        match last_craft(&records, "/p", "girth-a") {
            Some(Event::Craft { body_bytes, .. }) => assert_eq!(*body_bytes, 4),
            other => panic!("{other:?}"),
        }
        assert!(last_craft(&records, "/none", "girth-a").is_none());
    }
}
