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
