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
        "name: girth-captions\ndescription: captions here\nmetadata:\n  girth: {}",
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
fn search_warns_about_an_invalid_config_exactly_once() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("proj/.girth")).unwrap();
    std::fs::write(tmp.path().join("proj/.girth/config.json"), "{not json").unwrap();
    let assert = girth()
        .args(["search", "anything"])
        .env("HOME", tmp.path().join("home"))
        .env_remove("GIRTH_SKILL_ROOTS")
        .env_remove("GIRTH_OUTPUT_DIR")
        .current_dir(tmp.path().join("proj"))
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert_eq!(
        stderr.matches("girth: ignoring invalid").count(),
        1,
        "{stderr}"
    );
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

fn ledger_lines(home: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(home.join("ledger.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn record_use_reads_hook_json_from_stdin() {
    let tmp = TempDir::new().unwrap();
    girth()
        .env("GIRTH_HOME", tmp.path())
        .args(["record", "use"])
        .write_stdin(r#"{"tool_name":"Skill","tool_input":{"skill":"girth:girth"},"cwd":"/proj"}"#)
        .assert()
        .success()
        .stdout("");
    let lines = ledger_lines(tmp.path());
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["kind"], "use");
    assert_eq!(lines[0]["skill"], "girth:girth");
    assert_eq!(lines[0]["project"], "/proj");
}

#[test]
fn record_use_with_bad_stdin_records_nothing_and_exits_zero() {
    let tmp = TempDir::new().unwrap();
    for input in [
        "",
        "garbage",
        r#"{"tool_input":{}}"#,
        r#"{"tool_input":{"skill":""}}"#,
    ] {
        girth()
            .env("GIRTH_HOME", tmp.path())
            .args(["record", "use"])
            .write_stdin(input)
            .assert()
            .success();
    }
    assert!(ledger_lines(tmp.path()).is_empty());
}

#[test]
fn record_use_with_flag_uses_cwd_as_project() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir(&project).unwrap();
    girth()
        .env("GIRTH_HOME", tmp.path())
        .current_dir(&project)
        .args(["record", "use", "--skill", "girth-captions"])
        .assert()
        .success();
    let lines = ledger_lines(tmp.path());
    assert_eq!(lines[0]["skill"], "girth-captions");
    assert_eq!(lines[0]["project"], project.display().to_string());
}

#[test]
fn record_use_never_fails_even_when_the_ledger_is_unwritable() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("not-a-dir");
    std::fs::write(&file, "").unwrap();
    girth()
        .env("GIRTH_HOME", &file)
        .args(["record", "use", "--skill", "x"])
        .assert()
        .success();
}

#[test]
fn feedback_records_rating_and_optional_note() {
    let tmp = TempDir::new().unwrap();
    girth()
        .env("GIRTH_HOME", tmp.path())
        .args(["feedback", "girth-captions", "up", "--note", "great pacing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("girth-captions").and(predicate::str::contains("👍")));
    girth()
        .env("GIRTH_HOME", tmp.path())
        .args(["feedback", "girth-captions", "down"])
        .assert()
        .success()
        .stdout(predicate::str::contains("👎").and(predicate::str::contains("👍").not()));
    let lines = ledger_lines(tmp.path());
    assert_eq!(
        (lines[0]["rating"].as_str(), lines[0]["note"].as_str()),
        (Some("up"), Some("great pacing"))
    );
    assert_eq!(lines[1]["rating"], "down");
    assert!(lines[1].get("note").is_none());
    girth()
        .env("GIRTH_HOME", tmp.path())
        .args(["feedback", "girth-captions", "meh"])
        .assert()
        .code(2);
}

#[test]
fn stats_summarises_the_ledger_in_text_and_json() {
    let tmp = TempDir::new().unwrap();
    girth()
        .env("GIRTH_HOME", tmp.path())
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("empty"));
    girth()
        .env("GIRTH_HOME", tmp.path())
        .args(["record", "use", "--skill", "hyperframes"])
        .assert()
        .success();
    girth()
        .env("GIRTH_HOME", tmp.path())
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("Most used skills"))
        .stdout(predicate::str::contains("hyperframes 1"));
    let out = girth()
        .env("GIRTH_HOME", tmp.path())
        .args(["stats", "--json"])
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(json["events"], 1);
    assert_eq!(json["sharing"], "unset");
}

#[test]
fn share_status_allow_deny_round_trip() {
    let tmp = TempDir::new().unwrap();
    let share = |arg: &str| {
        girth()
            .env("GIRTH_HOME", tmp.path())
            .args(["share", arg])
            .assert()
            .success()
    };
    share("status").stdout("sharing: unset\n");
    share("allow").stdout(predicate::str::contains("sharing: allowed"));
    share("status").stdout("sharing: allowed\n");
    share("deny").stdout(predicate::str::contains("sharing: denied"));
    share("status").stdout("sharing: denied\n");
}

#[test]
fn share_preview_prints_payload_json_and_says_nothing_is_uploaded() {
    let tmp = TempDir::new().unwrap();
    let out = girth()
        .env("GIRTH_HOME", tmp.path())
        .args(["share", "preview"])
        .assert()
        .success()
        .stderr(predicate::str::contains("nothing is uploaded"));
    let json: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(json["girth"], env!("CARGO_PKG_VERSION"));
    assert!(json["sections"].as_array().unwrap().is_empty());
}

#[test]
fn fetch_prints_the_local_path_and_rejects_bad_specs() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    let fake = tmp.path().join("fake-git");
    std::fs::write(
        &fake,
        "#!/bin/sh\nfor last; do :; done\nmkdir -p \"$last/skills/x\" && echo hi > \"$last/skills/x/SKILL.md\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let home = tmp.path().join("home");
    girth()
        .env("GIRTH_HOME", &home)
        .env("GIRTH_GIT", &fake)
        .args(["fetch", "acme/skills/skills/x"])
        .assert()
        .success()
        .stdout(format!(
            "fetched: {}\n",
            home.join("fetch/acme__skills/skills/x").display()
        ));
    girth()
        .env("GIRTH_HOME", &home)
        .args(["fetch", "https://evil.example/x"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("expected owner/repo"));
    girth()
        .env("GIRTH_HOME", &home)
        .env("GIRTH_GIT", "/bin/false")
        .args(["fetch", "acme/skills"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("git clone failed"));
}

#[test]
fn show_prints_overview_or_only_the_requested_sections() {
    let tmp = TempDir::new().unwrap();
    let dir = write_skill(
        tmp.path(),
        "video-mega",
        "name: video-mega\ndescription: All video.",
        "## Caption timing\nHold 1.2 s.\n## Audio mixing\nDuck 12 dB.\n",
    );
    girth()
        .arg("show")
        .arg(&dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "SKILL.md: Caption timing · Audio mixing",
        ));
    girth()
        .arg("show")
        .arg(&dir)
        .args(["--heading", "Caption timing"])
        .assert()
        .success()
        .stdout("=== SKILL.md ===\n## Caption timing\nHold 1.2 s.\n");
    girth()
        .arg("show")
        .arg(tmp.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("no SKILL.md"));
}

const LISTING: &str = "The following skills are available for use with the Skill tool:\n\n\
- girth:girth: Use when a task needs know-how.\n\
- alpha: Use when alpha.\n\
- beta: Use when beta.\n\
- plug:gamma: Use when gamma.\n";

fn lens(
    home: &std::path::Path,
    project: &std::path::Path,
    args: &[&str],
) -> assert_cmd::assert::Assert {
    let mut cmd = Command::cargo_bin("girth").unwrap();
    cmd.env("GIRTH_HOME", home)
        .env_remove("GIRTH_LENS")
        .arg("lens")
        .arg("--project")
        .arg(project)
        .args(args)
        .write_stdin(LISTING)
        .assert()
}

#[test]
fn lens_collapses_unused_skills_and_records_the_baseline_once() {
    let tmp = TempDir::new().unwrap();
    let (home, project) = (tmp.path().join("home"), tmp.path().join("proj"));
    std::fs::create_dir_all(&project).unwrap();
    let out = lens(&home, &project, &[]).success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(
        text.contains("- girth:girth: Use when a task needs know-how.\n"),
        "{text}"
    );
    assert!(text.ends_with("- Also installed (short hints; ask `girth search \"<need>\"` which fits): alpha (alpha.); beta (beta.); plug:gamma (gamma.)\n"), "{text}");
    assert!(!text.contains("Use when beta"), "{text}");
    lens(&home, &project, &[]).success().stdout(text);
    let ledger = std::fs::read_to_string(home.join("ledger.jsonl")).unwrap();
    assert_eq!(ledger.lines().count(), 1, "{ledger}");
    assert!(ledger.contains(r#""kind":"seen""#) && ledger.contains("plug:gamma"));
}

#[test]
fn lens_json_reports_names_and_bytes() {
    let tmp = TempDir::new().unwrap();
    let (home, project) = (tmp.path().join("home"), tmp.path().join("proj"));
    std::fs::create_dir_all(&project).unwrap();
    let out = lens(&home, &project, &["--json"]).success();
    let json: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(json["described"], serde_json::json!(["girth:girth"]));
    assert_eq!(
        json["collapsed"],
        serde_json::json!(["alpha", "beta", "plug:gamma"])
    );
    assert_eq!(json["bytes_before"], LISTING.len());
    assert_eq!(json["bytes_after"], json["text"].as_str().unwrap().len());
}

#[test]
fn lens_can_be_turned_off_and_pins_names() {
    let tmp = TempDir::new().unwrap();
    let (home, project) = (tmp.path().join("home"), tmp.path().join("proj"));
    std::fs::create_dir_all(project.join(".girth")).unwrap();
    let mut cmd = Command::cargo_bin("girth").unwrap();
    cmd.env("GIRTH_HOME", &home)
        .env("GIRTH_LENS", "off")
        .args(["lens", "--project"])
        .arg(&project)
        .write_stdin(LISTING)
        .assert()
        .success()
        .stdout(LISTING);
    std::fs::write(
        project.join(".girth/config.json"),
        r#"{"lens": {"enabled": false}}"#,
    )
    .unwrap();
    lens(&home, &project, &[]).success().stdout(LISTING);
    std::fs::write(
        project.join(".girth/config.json"),
        r#"{"lens": {"keep": ["alpha"]}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("config.json"), r#"{"lens": {"keep": ["beta"]}}"#).unwrap();
    // alpha and beta pinned: only gamma would collapse, so nothing changes.
    lens(&home, &project, &[]).success().stdout(LISTING);
}

fn home_cmd(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("girth").unwrap();
    cmd.env("GIRTH_HOME", home);
    cmd
}

#[test]
fn meter_record_appends_a_turn_and_stats_reports_it() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let turn = r#"{"project":"/work/a","model":"claude-opus-5-5","input":2,"output":7,"cache_read":8605,"cache_write":26631,"agent":false,"aborted":false,"lens_before":30027,"lens_after":2896}"#;
    home_cmd(&home)
        .args(["meter", "record"])
        .write_stdin(turn)
        .assert()
        .success()
        .stdout("");
    let other = turn
        .replace("/work/a", "/work/b")
        .replace(r#","lens_before":30027,"lens_after":2896"#, "");
    home_cmd(&home)
        .args(["meter", "record"])
        .write_stdin(other)
        .assert()
        .success();
    // Bad input is ignored quietly: the hook must never disturb a session.
    home_cmd(&home)
        .args(["meter", "record"])
        .write_stdin("not json")
        .assert()
        .success()
        .stdout("");
    let ledger = std::fs::read_to_string(home.join("ledger.jsonl")).unwrap();
    assert_eq!(ledger.lines().count(), 2, "{ledger}");

    let out = home_cmd(&home).args(["stats"]).assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(
        text.contains("meter: 2 turns · input 4 · output 14"),
        "{text}"
    );
    assert!(
        text.contains("meter: Lens trimmed the skill listing in 1 turns: ≥ 6,782 tokens not sent"),
        "{text}"
    );

    let out = home_cmd(&home)
        .args(["stats", "--meter", "--project", "/work/b"])
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(text.starts_with("meter: 1 turns · input 2"), "{text}");
    assert!(
        !text.contains("Lens") && !text.contains("Girth ledger"),
        "{text}"
    );

    let out = home_cmd(&home).args(["stats", "--json"]).assert().success();
    let json: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(json["meter"]["turns"], 2);
    assert_eq!(json["meter"]["lens_tokens_saved"], 6782);
}

#[test]
fn stats_meter_without_turns_says_so() {
    let tmp = TempDir::new().unwrap();
    home_cmd(&tmp.path().join("home"))
        .args(["stats", "--meter"])
        .assert()
        .success()
        .stdout("meter: no turns recorded yet\n");
}
