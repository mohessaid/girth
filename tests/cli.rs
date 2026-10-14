use assert_cmd::Command;
use predicates::prelude::*;

/// The girth binary with its ledger isolated from the real `~/.girth`.
fn girth() -> Command {
    let mut cmd = Command::cargo_bin("girth").unwrap();
    cmd.env(
        "GIRTH_HOME",
        std::env::temp_dir().join(format!("girth-test-home-{}", std::process::id())),
    );
    cmd
}

#[test]
fn version_flag_prints_name_and_version() {
    girth()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(concat!(
            "girth ",
            env!("CARGO_PKG_VERSION")
        )));
}

mod common;
use common::write_skill;
use tempfile::TempDir;

#[test]
fn measure_json_reports_totals() {
    let tmp = TempDir::new().unwrap();
    write_skill(tmp.path(), "a", "name: a\ndescription: d", "");
    let out = girth()
        .args(["measure", "--json", "--roots"])
        .arg(tmp.path())
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(json["totals"]["count"], 1);
}

#[test]
fn measure_table_and_lenient_warning() {
    let tmp = TempDir::new().unwrap();
    write_skill(tmp.path(), "a", "name: a\ndescription: Use when: x", "");
    girth()
        .args(["measure", "--roots"])
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("TOTAL: 1 skills"))
        .stderr(predicate::str::contains(
            "girth: recovered invalid YAML frontmatter leniently in 1 skill(s): a",
        ));
}

#[test]
fn search_json_includes_output_dir_and_searches_cache() {
    let tmp = TempDir::new().unwrap();
    let (vendor, out) = (tmp.path().join("vendor"), tmp.path().join("out"));
    write_skill(
        &vendor,
        "video-mega",
        "name: video-mega\ndescription: video captions",
        "",
    );
    write_skill(
        &out,
        "girth-captions",
        "name: girth-captions\ndescription: captions here",
        "",
    );
    let assert = girth()
        .args(["search", "captions", "--json", "--output"])
        .arg(&out)
        .arg("--roots")
        .arg(&vendor)
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(json["output_dir"], out.display().to_string());
    let names: Vec<&str> = json["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["girth-captions", "video-mega"]);
    assert_eq!(json["hits"][0]["crafted"], true);
}

#[test]
fn search_text_output_and_no_hits() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "video-mega",
        "name: video-mega",
        "## Caption timing\n",
    );
    let out = tmp.path().join("o");
    girth()
        .args(["search", "caption", "--output"])
        .arg(&out)
        .arg("--roots")
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::starts_with(format!(
            "output_dir: {}",
            out.display()
        )))
        .stdout(predicate::str::contains("SKILL.md > Caption timing"));
    girth()
        .args(["search", "cobol", "--output"])
        .arg(&out)
        .arg("--roots")
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no matching skills"));
}

const VALID_FM: &str = "name: girth-captions\ndescription: Caption rules.\nmetadata:\n  girth:\n    sources:\n      - path-or-repo: x\n    crafted_for: captions\n    created: 2026-10-14\n    last_used: 2026-10-14";

#[test]
fn validate_reports_valid_and_invalid_with_exit_code() {
    let tmp = TempDir::new().unwrap();
    let good = write_skill(tmp.path(), "girth-captions", VALID_FM, "body\n");
    let bad = write_skill(
        tmp.path(),
        "girth-bad",
        "name: girth-bad\ndescription: d",
        "",
    );
    girth()
        .args(["validate", "--no-record"])
        .arg(&good)
        .assert()
        .success()
        .stdout("VALID girth-captions\n");
    girth()
        .arg("validate")
        .arg(&good)
        .arg(&bad)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("VALID girth-captions"))
        .stdout(predicate::str::contains(
            "INVALID girth-bad: metadata.girth block is required",
        ));
}

#[test]
fn closed_stdout_does_not_panic() {
    use std::process::{Command as Std, Stdio};
    let tmp = TempDir::new().unwrap();
    for i in 0..50 {
        write_skill(
            tmp.path(),
            &format!("s{i}"),
            &format!("name: s{i}\ndescription: captions"),
            "",
        );
    }
    let mut child = Std::new(env!("CARGO_BIN_EXE_girth"))
        .args(["search", "captions", "--roots"])
        .arg(tmp.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn defaults_come_from_home_and_config_warnings_reach_stderr() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        &tmp.path().join("home/.claude/skills"),
        "a",
        "name: a\ndescription: d",
        "",
    );
    std::fs::create_dir_all(tmp.path().join("proj/.girth")).unwrap();
    std::fs::write(tmp.path().join("proj/.girth/config.json"), "{not json").unwrap();
    girth()
        .args(["measure", "--json"])
        .env("HOME", tmp.path().join("home"))
        .env_remove("GIRTH_SKILL_ROOTS")
        .current_dir(tmp.path().join("proj"))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\": 1"))
        .stderr(predicate::str::contains("girth: ignoring invalid"));
}

#[test]
fn validate_records_craft_events_and_reports_the_diff() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let src = tmp.path().join("src");
    write_skill(
        &src,
        "video-mega",
        "name: video-mega\ndescription: d",
        "## Caption timing\nrule\n## Export\nx\n",
    );
    let fm = VALID_FM;
    let body = "## Caption timing <!-- src: video-mega#caption-timing -->\nrule\n## House\nours\n";
    let crafted = write_skill(tmp.path(), "girth-captions", fm, body);
    let run = || {
        girth()
            .env("GIRTH_HOME", &home)
            .args(["validate", "--roots"])
            .arg(&src)
            .arg("--")
            .arg(&crafted)
            .assert()
            .success()
    };
    run().stdout("VALID girth-captions (recorded: +2 −0 sections)\n");
    run().stdout("VALID girth-captions (recorded: +0 −0 sections)\n");
    std::fs::write(
        crafted.join("SKILL.md"),
        format!("---\n{fm}\n---\n## House\nours\n"),
    )
    .unwrap();
    run().stdout("VALID girth-captions (recorded: +0 −1 sections)\n");
    let ledger = std::fs::read_to_string(home.join("ledger.jsonl")).unwrap();
    assert_eq!(ledger.lines().count(), 3);
    assert!(ledger.contains(r#""total":2"#) && ledger.contains(r#""used":["caption-timing"]"#));
}

#[test]
fn validate_no_record_and_invalid_skills_write_nothing() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let good = write_skill(tmp.path(), "girth-captions", VALID_FM, "body\n");
    let bad = write_skill(
        tmp.path(),
        "girth-bad",
        "name: girth-bad\ndescription: d",
        "",
    );
    girth()
        .env("GIRTH_HOME", &home)
        .args(["validate", "--no-record"])
        .arg(&good)
        .assert()
        .success();
    girth()
        .env("GIRTH_HOME", &home)
        .arg("validate")
        .arg(&bad)
        .assert()
        .code(1);
    assert!(!home.join("ledger.jsonl").exists());
}
