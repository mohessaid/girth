use assert_cmd::Command;
use predicates::prelude::*;

fn girth() -> Command {
    Command::cargo_bin("girth").unwrap()
}

#[test]
fn version_flag_prints_name_and_version() {
    girth()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("girth 0.1.0"));
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
