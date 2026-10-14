//! Girth's own SKILL.md, held to the rules in .claude/skills/girth-skill-authoring.

use std::path::Path;

use girth::frontmatter::{parse_strict, split_frontmatter};
use girth::skill::load_skill;

fn skill_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/girth"))
}

#[test]
fn description_is_a_trigger_not_a_workflow_summary() {
    let info = load_skill(skill_dir()).expect("skills/girth/SKILL.md exists");
    assert_eq!(info.name, "girth");
    assert!(
        info.description.starts_with("Use when"),
        "{}",
        info.description
    );
    assert!(info.description.len() < 500);
    for workflow_word in ["search", "validate", "distil", "write", "step"] {
        assert!(
            !info.description.to_lowercase().contains(workflow_word),
            "description mentions '{workflow_word}'"
        );
    }
}

#[test]
fn frontmatter_is_strict_yaml() {
    let text = std::fs::read_to_string(skill_dir().join("SKILL.md")).unwrap();
    let (frontmatter, _) = split_frontmatter(&text);
    assert!(parse_strict(&frontmatter).is_ok());
}

#[test]
fn body_names_every_helper_and_rule() {
    let text = std::fs::read_to_string(skill_dir().join("SKILL.md")).unwrap();
    let (_, body) = split_frontmatter(&text);
    for needle in [
        "scripts/girth search",
        "scripts/girth validate",
        "output_dir",
        "matched_headings",
        "no source found",
        "verbatim",
        "3 KB",
        "never run",
    ] {
        assert!(body.contains(needle), "body lacks '{needle}'");
    }
    assert!(
        body.split_whitespace().count() <= 650,
        "body has {} words",
        body.split_whitespace().count()
    );
}
