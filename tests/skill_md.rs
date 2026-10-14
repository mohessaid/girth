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
    assert!(
        info.description.contains("lasting preference"),
        "taste must be able to trigger the skill"
    );
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
        "scripts/girth share status",
        "<!-- src:",
        "references/review.md",
        "references/feedback.md",
        "references/stats.md",
        "references/sharing.md",
        "references/taste.md",
        "scripts/girth taste",
        "scripts/girth made",
        "scripts/girth taste show",
        "never drop",
    ] {
        assert!(body.contains(needle), "body lacks '{needle}'");
    }
    assert!(
        body.split_whitespace().count() <= 700,
        "body has {} words",
        body.split_whitespace().count()
    );
}

#[test]
fn reference_flows_exist_are_small_and_name_their_command() {
    for (file, command) in [
        ("review.md", "scripts/girth validate"),
        ("feedback.md", "scripts/girth feedback"),
        ("stats.md", "scripts/girth stats"),
        ("sharing.md", "scripts/girth share"),
        ("taste.md", "scripts/girth taste add"),
    ] {
        let path = skill_dir().join("references").join(file);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing {file}"));
        assert!(text.len() <= 1536, "{file} is {} bytes", text.len());
        assert!(text.contains(command), "{file} lacks '{command}'");
    }
    let sharing = std::fs::read_to_string(skill_dir().join("references/sharing.md")).unwrap();
    for promise in ["Never", "Nothing is uploaded", "preview"] {
        assert!(sharing.contains(promise), "sharing.md lacks '{promise}'");
    }
}

#[test]
fn consent_never_blocks_and_comes_after_the_task_is_served() {
    let text = std::fs::read_to_string(skill_dir().join("SKILL.md")).unwrap();
    let (_, body) = split_frontmatter(&text);
    let consent = body
        .find("scripts/girth share status")
        .expect("consent check present");
    let report = body
        .find("## 6. Report and continue")
        .expect("report step present");
    assert!(
        consent > report,
        "consent must come after the task is served"
    );
    assert!(
        body.contains("Never block"),
        "consent must say it never blocks the task"
    );
}
