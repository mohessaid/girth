//! `girth drift`: has a crafted skill's source moved on since it was distilled?
//!
//! `girth validate` records, per `<!-- src: skill#slug -->` marker, the hash of the source
//! section it distilled. Drift re-hashes those sections in the skills installed now and
//! compares. Report only: re-crafting stays the model's job.

use crate::ledger::{Event, SourceUse};
use crate::record::body_sections;
use crate::skill::SkillInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// The source section's text changed.
    Changed { source: String, slug: String },
    /// The source no longer has a section with that slug (renamed or removed).
    Missing { source: String, slug: String },
    /// The source skill is on no configured root (uninstalled, or a fetch that was cleaned up).
    Unavailable { source: String },
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Finding::Changed { source, slug } => write!(f, "changed {source}#{slug}"),
            Finding::Missing { source, slug } => write!(f, "missing {source}#{slug}"),
            Finding::Unavailable { source } => write!(f, "unavailable {source}"),
        }
    }
}

/// Drift of one craft event's sources against the skills installed now.
pub fn drift(craft: &Event, installed: &[SkillInfo]) -> Vec<Finding> {
    match craft {
        Event::Craft { sources, .. } => sources
            .iter()
            .flat_map(|s| source_drift(s, installed))
            .collect(),
        _ => Vec::new(),
    }
}

fn source_drift(source: &SourceUse, installed: &[SkillInfo]) -> Vec<Finding> {
    let Some((_, now)) = installed
        .iter()
        .find(|s| s.name == source.skill)
        .and_then(|s| body_sections(&s.path.join("SKILL.md")))
    else {
        return vec![Finding::Unavailable {
            source: source.skill.clone(),
        }];
    };
    // Only slugs with a recorded hash have a baseline to compare against.
    source
        .hashes
        .iter()
        .filter_map(|(slug, then)| {
            let (source, slug) = (source.skill.clone(), slug.clone());
            match now.iter().find(|s| s.slug == slug) {
                None => Some(Finding::Missing { source, slug }),
                Some(s) if &s.hash != then => Some(Finding::Changed { source, slug }),
                Some(_) => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::craft_event;
    use crate::skill::discover_skills;
    use crate::testutil::write_skill;
    use tempfile::TempDir;

    const SOURCE: &str = "## Caption timing\nNever more than 2 lines.\n## Export\nH.264\n";
    const CRAFTED: &str = "## Captions <!-- src: video-mega#caption-timing, video-mega#export -->\nTwo lines.\n## Ours\nHouse rule.\n";

    /// A craft recorded against `video-mega`, then the source rewritten to `now`.
    fn findings(now: Option<&str>) -> Vec<String> {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        write_skill(
            &src,
            "video-mega",
            "name: video-mega\ndescription: d",
            SOURCE,
            &[],
        );
        let crafted = write_skill(
            &tmp.path().join("out"),
            "girth-captions",
            "name: girth-captions\ndescription: d",
            CRAFTED,
            &[],
        );
        let event = craft_event(
            &crafted,
            "/proj",
            &discover_skills(std::slice::from_ref(&src)),
            None,
        );
        match now {
            Some(body) => {
                write_skill(
                    &src,
                    "video-mega",
                    "name: video-mega\ndescription: d",
                    body,
                    &[],
                );
            }
            None => std::fs::remove_dir_all(src.join("video-mega")).unwrap(),
        }
        drift(&event, &discover_skills(&[src]))
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn unchanged_source_has_no_drift() {
        assert!(findings(Some(SOURCE)).is_empty());
    }

    #[test]
    fn whitespace_and_emphasis_edits_are_not_drift() {
        let reflowed = "## Caption timing\n\n**Never** more than   2 lines.\n\n## Export\nH.264\n";
        assert!(findings(Some(reflowed)).is_empty());
    }

    #[test]
    fn edited_section_is_changed() {
        let edited = "## Caption timing\nNever more than 3 lines.\n## Export\nH.264\n";
        assert_eq!(
            findings(Some(edited)),
            ["changed video-mega#caption-timing"]
        );
    }

    #[test]
    fn renamed_section_is_missing() {
        let renamed = "## Caption timing\nNever more than 2 lines.\n## Encoding\nH.264\n";
        assert_eq!(findings(Some(renamed)), ["missing video-mega#export"]);
    }

    #[test]
    fn uninstalled_source_is_unavailable() {
        assert_eq!(findings(None), ["unavailable video-mega"]);
    }

    #[test]
    fn non_craft_events_have_no_drift() {
        let e = Event::Use {
            skill: "girth-x".into(),
            project: "/p".into(),
        };
        assert!(drift(&e, &[]).is_empty());
    }
}
