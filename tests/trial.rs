//! `girth trial` end to end, with fake agents (shell scripts) in place of a model.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    _tmp: TempDir,
    repo: PathBuf,
    skill: PathBuf,
    home: PathBuf,
    root: PathBuf,
}

fn git(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

fn fixture() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let repo = root.join("repo");
    fs::create_dir_all(&repo).unwrap();
    fs::write(repo.join("README"), "hi\n").unwrap();
    // A committed copy of the skill: the without arm must remove it.
    fs::create_dir_all(repo.join(".claude/skills/pdf-fix")).unwrap();
    fs::write(repo.join(".claude/skills/pdf-fix/SKILL.md"), "old\n").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "init"]);
    // Uncommitted work must not reach the trial's clones.
    fs::write(repo.join("ok"), "leaked\n").unwrap();
    let skill = root.join("pdf-fix");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: pdf-fix\ndescription: Use when fixing PDFs.\n---\nWrite ok.\n",
    )
    .unwrap();
    fs::write(root.join("task.md"), "Fix the PDF.\n").unwrap();
    Fixture {
        home: root.join("home"),
        _tmp: tmp,
        repo,
        skill,
        root,
    }
}

/// A fake agent: a stream-json init line, then `body`, then a result line costing `cost`.
fn agent(f: &Fixture, name: &str, body: &str) -> String {
    let path = f.root.join(name);
    fs::write(
        &path,
        format!(
            "cat > prompt.txt\necho '{{\"type\":\"system\"}}'\n{body}\necho '{{\"type\":\"result\",\"total_cost_usd\":0.25}}'\n"
        ),
    )
    .unwrap();
    format!("sh {}", path.display())
}

const HAS_SKILL: &str =
    "test \"$(cat .claude/skills/pdf-fix/SKILL.md 2>/dev/null | head -c 3)\" = '---'";
const FIRE: &str = r#"echo '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"pdf-fix"}}]}}'"#;

/// girth with a ledger in the fixture, and no way to reach a real agent: the default
/// agent is replaced and `claude` is off the PATH.
fn base(f: &Fixture) -> Command {
    let mut cmd = Command::cargo_bin("girth").unwrap();
    cmd.env("GIRTH_HOME", &f.home)
        .env("GIRTH_TRIAL_AGENT", "false")
        .env("PATH", "/usr/bin:/bin");
    cmd
}

fn trial(f: &Fixture) -> Command {
    let mut cmd = base(f);
    cmd.current_dir(&f.repo);
    cmd.arg("trial").arg(&f.skill);
    cmd
}

#[test]
fn a_skill_that_makes_the_check_pass_is_admitted_and_recorded() {
    let f = fixture();
    // Passes only with the trial's skill (not the committed stub), and only from the
    // prompt on stdin.
    let a = agent(
        &f,
        "a.sh",
        &format!("if {HAS_SKILL}; then {FIRE}; grep -q PDF prompt.txt && echo yes > ok; fi"),
    );
    let out = trial(&f)
        .args(["--task", f.root.join("task.md").to_str().unwrap()])
        .args(["--check", "test -f ok", "--agent", &a])
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    for quiet in ["timed out", "ended in an error", "every run failed"] {
        assert!(!text.contains(quiet), "{quiet}: {text}");
    }
    // Arms alternate, with-skill first.
    let log = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    let runs: Vec<&str> = log.lines().take(2).collect();
    assert_eq!(
        runs,
        [
            "girth trial: task 1 run 1 (with): pass",
            "girth trial: task 1 run 1 (without): fail"
        ],
        "{log}"
    );
    assert!(
        text.contains("task 1: with 3/3 · without 0/3 → won"),
        "{text}"
    );
    assert!(
        text.contains("skill used in 3 of 3 runs · $0.2500 vs $0.2500 per run → admit"),
        "{text}"
    );
    let ledger = fs::read_to_string(f.home.join("ledger.jsonl")).unwrap();
    assert!(ledger.contains(r#""kind":"trial""#), "{ledger}");
    assert!(ledger.contains(r#""verdict":"admit""#), "{ledger}");
    assert!(ledger.contains(r#""with_cost_micros":250000"#), "{ledger}");
    assert!(!ledger.contains("Fix the PDF"), "{ledger}");
    assert!(!ledger.contains("test -f ok"), "{ledger}");
    // The clones are gone, and the real repo was never touched.
    assert_eq!(fs::read_to_string(f.repo.join("ok")).unwrap(), "leaked\n");
    // Clones live in a scratch dir, removed afterwards, never in the repo.
    assert_eq!(fs::read_dir(f.home.join("trials")).unwrap().count(), 0);
    let mut entries: Vec<String> = fs::read_dir(&f.repo)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    assert_eq!(entries, [".claude", ".git", "README", "ok"]);
    assert_eq!(
        fs::read_to_string(f.repo.join(".claude/skills/pdf-fix/SKILL.md")).unwrap(),
        "old\n"
    );
}

#[test]
fn a_skill_that_breaks_the_task_is_rejected() {
    let f = fixture();
    let a = agent(
        &f,
        "a.sh",
        &format!("if {HAS_SKILL}; then {FIRE}; else echo yes > ok; fi"),
    );
    trial(&f)
        .args(["--task", f.root.join("task.md").to_str().unwrap()])
        .args(["--check", "test -f ok", "--agent", &a])
        .assert()
        .success()
        .stdout(predicates::str::contains("with 0/3 · without 3/3 → lost"))
        .stdout(predicates::str::contains("→ reject"));
}

#[test]
fn a_skill_that_never_fires_is_undecided() {
    let f = fixture();
    let a = agent(
        &f,
        "a.sh",
        &format!("if {HAS_SKILL}; then echo yes > ok; fi"),
    );
    trial(&f)
        .args(["--task", f.root.join("task.md").to_str().unwrap()])
        .args(["--check", "test -f ok", "--agent", &a, "--runs", "2"])
        .assert()
        .success()
        .stdout(predicates::str::contains("skill used in 0 of 2 runs"))
        .stdout(predicates::str::contains("→ undecided"));
}

#[test]
fn a_hanging_agent_is_killed_and_fails_and_a_plain_agent_reports_unknowns() {
    let f = fixture();
    let started = std::time::Instant::now();
    trial(&f)
        .args(["--task", f.root.join("task.md").to_str().unwrap()])
        .args(["--check", "test -f ok", "--agent", "touch ok; sleep 30"])
        .args(["--runs", "2", "--timeout", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("with 0/2 · without 0/2 → tied"))
        .stdout(predicates::str::contains("4 agent runs timed out"))
        .stdout(predicates::str::contains("every run failed in both arms"))
        .stdout(predicates::str::contains(
            "skill use unknown (the agent's output didn't say; the verdict assumes it ran) · cost unknown → undecided",
        ));
    assert!(started.elapsed().as_secs() < 20);
}

#[test]
fn one_check_serves_every_task_and_mismatched_counts_are_refused() {
    let f = fixture();
    let task = f.root.join("task.md");
    let task = task.to_str().unwrap();
    trial(&f)
        .args([
            "--task", task, "--task", task, "--check", "true", "--agent", "true",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "task 2: with 3/3 · without 3/3 → tied",
        ));
    trial(&f)
        .args(["--task", task, "--task", task])
        .args(["--check", "true", "--check", "true", "--check", "true"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains(
            "one --check for every --task, or a single one",
        ));
    trial(&f)
        .args(["--task", task, "--check", "true", "--runs", "1"])
        .assert()
        .code(2);
}

#[test]
fn outside_a_git_repo_or_without_a_skill_it_refuses() {
    let f = fixture();
    let task = f.root.join("task.md");
    base(&f)
        .current_dir(&f.root)
        .arg("trial")
        .arg(&f.skill)
        .args(["--task", task.to_str().unwrap(), "--check", "true"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("git repository"));
    trial(&f)
        .args(["--task", "/nonexistent/task.md", "--check", "true"])
        .assert()
        .code(2);
    base(&f)
        .current_dir(&f.repo)
        .args(["trial", f.root.to_str().unwrap()])
        .args(["--task", task.to_str().unwrap(), "--check", "true"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("SKILL.md"));
    assert!(!f.home.join("ledger.jsonl").exists());
}

#[test]
fn agent_errors_and_check_timeouts_count_as_fails_and_say_why() {
    let f = fixture();
    let task = f.root.join("task.md");
    let task = task.to_str().unwrap();
    trial(&f)
        .args(["--task", task, "--check", "true", "--runs", "2"])
        .args(["--agent", "echo 'Invalid API key' >&2; exit 1"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "4 agent runs ended in an error (counted as fails): Invalid API key",
        ))
        .stdout(predicates::str::contains("with 0/2 · without 0/2"));
    trial(&f)
        .args(["--task", task, "--check", "sleep 20", "--runs", "2"])
        .args(["--agent", "true", "--check-timeout", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "4 checks timed out (counted as fails)",
        ));
    // An `is_error` result fails the run even when the agent exits 0.
    let a = agent(
        &f,
        "e.sh",
        r#"echo '{"type":"result","is_error":true,"result":"Not logged in"}'"#,
    );
    trial(&f)
        .args([
            "--task", task, "--check", "true", "--runs", "2", "--agent", &a,
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("4 agent runs ended in an error"));
    // An empty GIRTH_TRIAL_AGENT falls back to the default agent, which isn't on this
    // PATH, so the run fails at once instead of reaching a model.
    trial(&f)
        .env("GIRTH_TRIAL_AGENT", "")
        .args(["--task", task, "--check", "true", "--runs", "2"])
        .assert()
        .success()
        .stdout(predicates::str::contains("claude: not found"));
    // The default agent comes from GIRTH_TRIAL_AGENT here (`false`), never a real one.
    trial(&f)
        .args(["--task", task, "--check", "true", "--runs", "2"])
        .assert()
        .success()
        .stdout(predicates::str::contains("4 agent runs ended in an error"));
}

#[cfg(unix)]
#[test]
fn an_interrupted_trial_stops_its_agent_and_the_next_one_sweeps_its_clone() {
    use std::os::unix::process::CommandExt;
    let f = fixture();
    let marker = format!("girth-trial-marker-{}", std::process::id());
    let bin = assert_cmd::cargo::cargo_bin("girth");
    let mut child = std::process::Command::new(bin)
        .env("GIRTH_HOME", &f.home)
        .env("GIRTH_TRIAL_AGENT", "false")
        .env("PATH", "/usr/bin:/bin")
        .current_dir(&f.repo)
        .arg("trial")
        .arg(&f.skill)
        .args([
            "--task",
            f.root.join("task.md").to_str().unwrap(),
            "--check",
            "true",
        ])
        .args(["--agent", &format!("sleep 30 # {marker}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let running = |m: &str| {
        std::process::Command::new("pgrep")
            .args(["-f", m])
            .output()
            .unwrap()
            .status
            .success()
    };
    let start = std::time::Instant::now();
    while !running(&marker) && start.elapsed().as_secs() < 10 {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(running(&marker), "the agent never started");
    // A live process's scratch dir (pid 1 never exits) must survive the sweep.
    let live = f.home.join("trials/1-0");
    fs::create_dir_all(&live).unwrap();
    // Ctrl-C reaches the whole foreground process group.
    std::process::Command::new("kill")
        .args(["-INT", "--", &format!("-{}", child.id())])
        .status()
        .unwrap();
    child.wait().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!running(&marker), "the agent outlived girth");
    assert_eq!(fs::read_dir(f.home.join("trials")).unwrap().count(), 2);
    trial(&f)
        .args([
            "--task",
            f.root.join("task.md").to_str().unwrap(),
            "--check",
            "true",
        ])
        .args(["--agent", "true", "--runs", "2"])
        .assert()
        .success();
    let left: Vec<_> = fs::read_dir(f.home.join("trials"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(left, [live]);
}
