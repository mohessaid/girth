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
