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
    let out = run(
        &script,
        &[("GIRTH_DOWNLOAD_BASE", "file:///nonexistent-girth-release")],
    );
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

/// The Rust target triple the launcher should pick on this machine.
fn host_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        other => panic!("unsupported test host {other:?}"),
    }
}

fn sha256(path: &Path) -> String {
    let out = Command::new("sh")
        .arg("-c")
        .arg("sha256sum \"$1\" 2>/dev/null || shasum -a 256 \"$1\"")
        .arg("sh")
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

/// A fake GitHub release dir: `<root>/v<version>/{girth-<target>,SHA256SUMS}`.
fn fake_release(root: &Path, checksum_override: Option<&str>) -> String {
    let dir = root.join(format!("v{}", env!("CARGO_PKG_VERSION")));
    let asset = dir.join(format!("girth-{}", host_target()));
    fake_bin(&asset, "downloaded");
    let sum = checksum_override.map_or_else(|| sha256(&asset), str::to_string);
    std::fs::write(
        dir.join("SHA256SUMS"),
        format!("{sum}  girth-{}\n", host_target()),
    )
    .unwrap();
    format!("file://{}", root.display())
}

#[test]
fn downloads_verifies_and_caches_the_release_binary() {
    let tmp = TempDir::new().unwrap();
    let script = install(&tmp.path().join("install"));
    let base = fake_release(&tmp.path().join("release"), None);
    let out = run(&script, &[("GIRTH_DOWNLOAD_BASE", &base)]);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "downloaded --flag\n",
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(tmp.path().join("install/skills/girth/bin/girth").is_file());
    let again = run(
        &script,
        &[("GIRTH_DOWNLOAD_BASE", "file:///nonexistent-girth-release")],
    );
    assert_eq!(
        String::from_utf8_lossy(&again.stdout),
        "downloaded --flag\n"
    );
}

#[test]
fn checksum_mismatch_refuses_and_leaves_nothing_behind() {
    let tmp = TempDir::new().unwrap();
    let script = install(&tmp.path().join("install"));
    let base = fake_release(&tmp.path().join("release"), Some(&"0".repeat(64)));
    let out = run(&script, &[("GIRTH_DOWNLOAD_BASE", &base)]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("checksum mismatch"));
    assert!(!tmp.path().join("install/skills/girth/bin/girth").exists());
}

#[test]
fn launcher_version_matches_the_crate() {
    let text = std::fs::read_to_string(launcher()).unwrap();
    assert!(text.contains(&format!("GIRTH_VERSION={}\n", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn launcher_exports_its_own_path_for_the_taste_record_line() {
    let tmp = TempDir::new().unwrap();
    let script = install(tmp.path());
    let bin = tmp.path().join("skills/girth/bin/girth");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, "#!/bin/sh\necho \"$GIRTH_LAUNCHER\"\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = run(&script, &[]);
    let expected = std::fs::canonicalize(&script).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        expected.display().to_string()
    );
}
