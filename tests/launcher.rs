//! The POSIX launcher the skill calls: `skills/girth/scripts/girth`.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

fn launcher() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/girth/scripts/girth")
}

fn fake_bin(path: &Path, says: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\necho {says} \"$@\"\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Copy the launcher into a fake install at `<root>/skills/girth/scripts/girth`.
fn install(root: &Path) -> PathBuf {
    let target = root.join("skills/girth/scripts/girth");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::copy(launcher(), &target).unwrap();
    target
}

fn run(script: &Path, env: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = Command::new("sh");
    cmd.arg(script)
        .arg("--flag")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(std::env::temp_dir());
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

#[test]
fn girth_bin_env_wins() {
    let tmp = TempDir::new().unwrap();
    let script = install(tmp.path());
    fake_bin(&tmp.path().join("skills/girth/bin/girth"), "bundled");
    fake_bin(&tmp.path().join("custom"), "custom");
    let out = run(
        &script,
        &[("GIRTH_BIN", tmp.path().join("custom").to_str().unwrap())],
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "custom --flag\n");
}

#[test]
fn bundled_binary_beats_repo_build() {
    let tmp = TempDir::new().unwrap();
    let script = install(tmp.path());
    fake_bin(&tmp.path().join("skills/girth/bin/girth"), "bundled");
    std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
    fake_bin(&tmp.path().join("target/release/girth"), "repo");
    assert_eq!(
        String::from_utf8_lossy(&run(&script, &[]).stdout),
        "bundled --flag\n"
    );
}

#[test]
fn repo_release_build_is_used_in_a_checkout() {
    let tmp = TempDir::new().unwrap();
    let script = install(tmp.path());
    std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
    fake_bin(&tmp.path().join("target/release/girth"), "repo");
    assert_eq!(
        String::from_utf8_lossy(&run(&script, &[]).stdout),
        "repo --flag\n"
    );
}

#[test]
fn missing_binary_explains_how_to_install() {
    let tmp = TempDir::new().unwrap();
    let script = install(tmp.path());
    let out = run(&script, &[]);
    assert_eq!(out.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&out.stderr).contains("girth: binary not found"));
}

#[test]
fn real_launcher_runs_the_real_binary_from_another_cwd() {
    let out = run(&launcher(), &[("GIRTH_BIN", env!("CARGO_BIN_EXE_girth"))]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "unknown --flag should be a clap usage error"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("Usage: girth"));
}
