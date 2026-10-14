//! `girth taste`, `girth made`, and taste delivered through the Skill hook.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// A temp home (personal tier) and a project dir (project tier) per test.
struct Env {
    _tmp: TempDir,
    home: PathBuf,
    proj: PathBuf,
}

impl Env {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home/.girth");
        let proj = tmp.path().join("proj");
        std::fs::create_dir_all(&proj).unwrap();
        Self {
            _tmp: tmp,
            home,
            proj,
        }
    }

    fn girth(&self) -> Command {
        let mut cmd = Command::cargo_bin("girth").unwrap();
        cmd.env("GIRTH_HOME", &self.home)
            .env_remove("GIRTH_LAUNCHER")
            .current_dir(&self.proj);
        cmd
    }

    fn run(&self, args: &[&str]) -> String {
        let out = self.girth().args(args).assert().success();
        String::from_utf8_lossy(&out.get_output().stdout).into_owned()
    }

    fn hook(&self, skill: &str) -> String {
        let input = serde_json::json!({
            "tool_name": "Skill",
            "tool_input": {"skill": skill},
            "cwd": self.proj.display().to_string(),
        });
        let out = self
            .girth()
            .current_dir(self.home.parent().unwrap().parent().unwrap())
            .args(["record", "use"])
            .write_stdin(input.to_string())
            .assert()
            .success();
        String::from_utf8_lossy(&out.get_output().stdout).into_owned()
    }
}

fn lines(path: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn taste_list_is_friendly_when_empty() {
    let env = Env::new();
    assert_eq!(env.run(&["taste"]), "no taste remembered yet\n");
}

#[test]
fn add_show_and_list_across_both_tiers() {
    let env = Env::new();
    assert_eq!(
        env.run(&["taste", "add", "launch-video", "End on the logo."]),
        "remembered for launch-video: End on the logo.\n"
    );
    env.run(&[
        "taste",
        "add",
        "launch-video",
        "Sentence case.",
        "--project",
    ]);
    assert_eq!(lines(&env.home.join("taste.jsonl")).len(), 1);
    assert_eq!(lines(&env.proj.join(".girth/taste.jsonl")).len(), 1);
    let show = env.run(&["taste", "show", "launch-video"]);
    assert!(show.starts_with("taste: launch-video\nhouse rules (always apply):\n"));
    assert!(show.contains("  1. End on the logo.\n  2. (project) Sentence case.\n"));
    assert!(show.contains("record this one when done: girth made launch-video"));
    assert_eq!(env.run(&["taste"]), "launch-video  2 rules, 0 made\n");
}

#[test]
fn show_json_shape_is_frozen() {
    let env = Env::new();
    env.run(&["taste", "add", "d", "rule one"]);
    env.run(&["made", "d", "palette=neon"]);
    let json: serde_json::Value =
        serde_json::from_str(&env.run(&["taste", "show", "d", "--json"])).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "domain": "d",
            "rules": [{"n": 1, "text": "rule one", "tier": "personal"}],
            "recent": [["palette", ["neon"]]],
        })
    );
}

#[test]
fn show_of_an_unknown_domain_says_nothing_is_remembered() {
    let env = Env::new();
    assert_eq!(
        env.run(&["taste", "show", "captions"]),
        "taste: captions (nothing remembered yet)\n"
    );
}

#[test]
fn drop_removes_by_number_from_the_right_tier_only() {
    let env = Env::new();
    env.run(&["taste", "add", "d", "a"]);
    env.run(&["taste", "add", "d", "b", "--project"]);
    env.girth()
        .args(["taste", "drop", "d", "2"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("rule 2 is a project rule"));
    env.girth()
        .args(["taste", "drop", "d", "9"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("no rule 9 in d"));
    assert_eq!(
        env.run(&["taste", "drop", "d", "2", "--project"]),
        "dropped from d: b\n"
    );
    let show = env.run(&["taste", "show", "d"]);
    assert!(show.contains("1. a") && !show.contains(" b\n"), "{show}");
}

#[test]
fn bad_domains_and_empty_rules_are_usage_errors() {
    let env = Env::new();
    for args in [
        vec!["taste", "add", "Launch Video", "x"],
        vec!["taste", "add", "d", "   "],
        vec!["taste", "show", "a--b"],
        vec!["made", "BAD", "k=v"],
        vec!["made", "d", "novalue"],
        vec!["made", "d"],
    ] {
        env.girth().args(&args).assert().code(2);
    }
    assert!(!env.home.join("taste.jsonl").exists());
}

#[test]
fn made_records_choices_and_show_lists_them_newest_first() {
    let env = Env::new();
    assert_eq!(
        env.run(&["made", "d", "Palette=neon", "url=https://x.dev/?a=b"]),
        "remembered 2 choices for d\n"
    );
    env.run(&["made", "d", "palette=paper"]);
    let recorded = lines(&env.home.join("taste.jsonl"));
    assert_eq!(recorded[0]["choices"]["url"], "https://x.dev/?a=b");
    assert_eq!(recorded[0]["project"], env.proj.display().to_string());
    let show = env.run(&["taste", "show", "d"]);
    assert!(show.contains("  palette: paper, neon\n"), "{show}");
}

#[test]
fn launcher_path_names_the_record_command() {
    let env = Env::new();
    env.run(&["made", "d", "k=v"]);
    env.girth()
        .env("GIRTH_LAUNCHER", "/plug/scripts/girth")
        .args(["taste", "show", "d"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "record this one when done: /plug/scripts/girth made d key=value",
        ));
}

#[test]
fn hook_injects_taste_for_the_crafted_skill_and_its_linked_sources() {
    let env = Env::new();
    env.run(&[
        "taste",
        "add",
        "launch-video",
        "End on the logo.",
        "--project",
    ]);
    let out = env.hook("girth-launch-video");
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(json["hookSpecificOutput"]["hookEventName"], "PostToolUse");
    let context = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.contains("End on the logo."), "{context}");
    // The use itself is still recorded.
    assert_eq!(lines(&env.home.join("ledger.jsonl")).len(), 1);
    // An unrelated skill or no taste: stdout stays empty.
    assert_eq!(env.hook("video-mega"), "");
    assert_eq!(env.hook("girth-captions"), "");
}

#[test]
fn hook_context_is_capped() {
    let env = Env::new();
    for i in 0..12 {
        env.run(&[
            "taste",
            "add",
            "d",
            &format!("rule {i} {}", "x".repeat(200)),
        ]);
    }
    let out = env.hook("girth-d");
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    let context = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.len() <= 1500, "{}", context.len());
    assert!(
        context.contains("made d key=value"),
        "record line survives the cap"
    );
}

#[test]
fn validate_links_source_skills_so_their_use_brings_taste() {
    let env = Env::new();
    let vendor = env.proj.join("vendor/video-mega");
    std::fs::create_dir_all(&vendor).unwrap();
    std::fs::write(
        vendor.join("SKILL.md"),
        "---\nname: video-mega\ndescription: d\n---\n## Caption timing\nx\n",
    )
    .unwrap();
    let crafted = env.proj.join("crafted/girth-launch-video");
    std::fs::create_dir_all(&crafted).unwrap();
    std::fs::write(
        crafted.join("SKILL.md"),
        "---\nname: girth-launch-video\ndescription: Use when making launch videos.\nmetadata:\n  girth:\n    sources:\n      - path-or-repo: vendor/video-mega\n    crafted_for: launch\n    created: 2026-10-14\n    last_used: 2026-10-14\n---\n## Captions <!-- src: video-mega#caption-timing -->\nx\n",
    )
    .unwrap();
    for _ in 0..2 {
        env.girth()
            .arg("validate")
            .arg(&crafted)
            .args(["--roots", "vendor"])
            .assert()
            .success();
    }
    let links: Vec<_> = lines(&env.home.join("taste.jsonl"))
        .into_iter()
        .filter(|l| l["kind"] == "link")
        .collect();
    assert_eq!(links.len(), 1, "one link per pair: {links:?}");
    assert_eq!(links[0]["skill"], "video-mega");
    assert_eq!(links[0]["domain"], "launch-video");
    env.run(&["made", "launch-video", "palette=neon"]);
    assert!(env.hook("video-mega").contains("palette: neon"));
}

#[test]
fn share_preview_never_contains_taste() {
    let env = Env::new();
    let sentinel = "SENTINEL-video-mega-/home/me";
    env.run(&["taste", "add", "d", sentinel]);
    env.run(&["taste", "add", "d", sentinel, "--project"]);
    env.run(&["made", "d", &format!("k={sentinel}")]);
    let preview = env.run(&["share", "preview"]);
    assert!(!preview.contains("SENTINEL"), "{preview}");
}

#[test]
fn stats_counts_taste_without_its_text() {
    let env = Env::new();
    env.run(&["taste", "add", "d", "secret rule"]);
    env.run(&["made", "d", "k=v"]);
    env.run(&["made", "e", "k=v"]);
    let text = env.run(&["stats"]);
    assert!(text.contains("taste: 2 domains, 1 rules, 2 made"), "{text}");
    assert!(!text.contains("secret rule"));
    let json: serde_json::Value = serde_json::from_str(&env.run(&["stats", "--json"])).unwrap();
    assert_eq!(
        json["taste"],
        serde_json::json!({"domains": 2, "rules": 1, "made": 2})
    );
}
