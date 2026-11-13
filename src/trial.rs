//! Skill trials: run the same task with and without a skill, grade both with a check
//! command, and judge whether the skill earned its place. A task is decided only when
//! chance alone would give its gap at most 1 time in 20 (Fisher's exact test), so a
//! flipped run never sets a verdict and more runs only sharpen it. Cost is reported,
//! never decisive: it varies too much at small n.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// One task's passes in each arm, out of `runs` runs per arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResult {
    pub with_passed: u64,
    pub without_passed: u64,
    pub runs: u64,
}

/// How one task came out for the skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call {
    Won,
    Lost,
    Tied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Admit,
    Reject,
    Undecided,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Admit => "admit",
            Verdict::Reject => "reject",
            Verdict::Undecided => "undecided",
        }
    }
}

/// The most a task's call may be wrong by chance: a gap is decisive only when two
/// identical arms would show one at least that large less than 1 time in 20.
pub const ALPHA: f64 = 0.05;

/// Fisher's one-sided exact test: the chance that, of the `a + b` passes across two arms
/// of `runs` runs each, the first arm gets `a` or more if the arms were the same.
pub fn tail(a: u64, b: u64, runs: u64) -> f64 {
    let total = a + b;
    let hits: u128 = (a..=runs.min(total))
        .filter(|x| total - x <= runs)
        .map(|x| choose(runs, x) * choose(runs, total - x))
        .sum();
    hits as f64 / choose(2 * runs, total) as f64
}

fn choose(n: u64, k: u64) -> u128 {
    (0..k as u128).fold(1, |acc, i| acc * (n as u128 - i) / (i + 1))
}

/// Won or lost when Fisher's exact test puts the gap under `ALPHA`, else tied. In the
/// 2026-11-13 simulation, identical arms got a call at most 4.2% of the time at 2–10
/// runs, where a fixed 2-pass rule called 21% at 3 runs and 50% at 10. Worked exactly over
/// every pass rate (`examples/trial/false_calls.py`), the chance of any call stays at or
/// under 4.22% up to 10 runs and passes 5% at 11, so `--runs` stops at 10.
pub fn judge(t: &TaskResult) -> Call {
    if tail(t.with_passed, t.without_passed, t.runs) <= ALPHA {
        Call::Won
    } else if tail(t.without_passed, t.with_passed, t.runs) <= ALPHA {
        Call::Lost
    } else {
        Call::Tied
    }
}

/// Admit when some task was won and none lost; reject when the reverse; otherwise, or
/// when the skill never fired in a with-skill run (`fired == Some(0)`), undecided.
pub fn verdict(tasks: &[TaskResult], fired: Option<u64>) -> Verdict {
    if fired == Some(0) {
        return Verdict::Undecided;
    }
    let won = tasks.iter().any(|t| judge(t) == Call::Won);
    let lost = tasks.iter().any(|t| judge(t) == Call::Lost);
    match (won, lost) {
        (true, false) => Verdict::Admit,
        (false, true) => Verdict::Reject,
        _ => Verdict::Undecided,
    }
}

/// What an agent's output says about one run: whether the skill fired and what the run
/// cost. `None` when the output isn't Claude Code's JSON.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stream {
    pub fired: Option<bool>,
    pub cost: Option<f64>,
    /// The result said the run errored (`is_error`), such as a login or API failure.
    pub error: bool,
}

/// Reads Claude Code's `stream-json` lines (or one `json` result object).
pub fn read_stream(stdout: &str, skill: &str) -> Stream {
    let mut out = Stream::default();
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            continue;
        };
        let Some(kind) = v.get("type").and_then(|t| t.as_str()) else {
            continue;
        };
        match kind {
            // A stream (not a single result object) says what ran, so firing is known.
            "system" => {
                out.fired.get_or_insert(false);
            }
            "assistant" => {
                let fired = out.fired.get_or_insert(false);
                let blocks = v.pointer("/message/content").and_then(|c| c.as_array());
                for b in blocks.into_iter().flatten() {
                    let named = b
                        .pointer("/input/skill")
                        .and_then(|s| s.as_str())
                        .is_some_and(|s| s == skill || s.rsplit(':').next() == Some(skill));
                    if b.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                        && b.get("name").and_then(|n| n.as_str()) == Some("Skill")
                        && named
                    {
                        *fired = true;
                    }
                }
            }
            "result" => {
                out.error |= v.get("is_error").and_then(|e| e.as_bool()) == Some(true);
                out.cost = v
                    .get("total_cost_usd")
                    .and_then(|c| c.as_f64())
                    .or(out.cost);
            }
            _ => {}
        }
    }
    out
}

/// The agent `girth trial` runs when `--agent` isn't given: Claude Code, headless, with
/// user-level skills, plugins and MCP servers hidden so the skill is the only difference.
pub const DEFAULT_AGENT: &str = "claude -p --output-format stream-json --verbose --setting-sources project,local --permission-mode acceptEdits --allowedTools Bash,Skill";

pub struct Options {
    pub skill_dir: PathBuf,
    /// (prompt, check command) per task.
    pub tasks: Vec<(String, String)>,
    pub runs: u64,
    pub agent: String,
    pub timeout: Duration,
    pub check_timeout: Duration,
    /// Where scratch clones go (`$GIRTH_HOME/trials`); stale ones there are swept.
    pub scratch_root: PathBuf,
}

/// What a trial found, before it becomes a ledger event.
#[derive(Debug, PartialEq)]
pub struct Report {
    pub skill: String,
    pub tasks: Vec<TaskResult>,
    pub fired: Option<u64>,
    pub with_cost: Option<f64>,
    pub without_cost: Option<f64>,
    /// Agent runs killed at the timeout, checks killed at theirs, and agent runs that
    /// ended in an error (non-zero exit or an `is_error` result). All count as fails.
    pub timed_out: u64,
    pub check_timed_out: u64,
    pub errored: u64,
    /// The first line an erroring agent printed to stderr, to say why.
    pub error_line: Option<String>,
    pub verdict: Verdict,
}

impl Report {
    pub fn render(&self) -> String {
        let mut lines: Vec<String> = self
            .tasks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let call = match judge(t) {
                    Call::Won => "won",
                    Call::Lost => "lost",
                    Call::Tied => "tied",
                };
                format!(
                    "task {}: with {}/{} · without {}/{} → {call}",
                    i + 1,
                    t.with_passed,
                    t.runs,
                    t.without_passed,
                    t.runs
                )
            })
            .collect();
        if self.timed_out > 0 {
            lines.push(format!(
                "{} agent runs timed out (counted as fails)",
                self.timed_out
            ));
        }
        if self.check_timed_out > 0 {
            lines.push(format!(
                "{} checks timed out (counted as fails)",
                self.check_timed_out
            ));
        }
        if self.errored > 0 {
            let why = self
                .error_line
                .as_deref()
                .map(|l| format!(": {l}"))
                .unwrap_or_default();
            lines.push(format!(
                "{} agent runs ended in an error (counted as fails){why}",
                self.errored
            ));
        }
        if !self.tasks.is_empty()
            && self
                .tasks
                .iter()
                .all(|t| t.with_passed + t.without_passed == 0)
        {
            lines.push("every run failed in both arms: check the task, the check command and the agent before blaming the skill".into());
        }
        if self.tasks.iter().any(|t| t.runs < 3) {
            lines.push("at 2 runs per arm no task can be decided (the widest gap, 2/2 vs 0/2, happens 1 time in 6 by chance): use this to see whether the skill fires".into());
        }
        let with_runs: u64 = self.tasks.iter().map(|t| t.runs).sum();
        let used = match self.fired {
            Some(n) => format!("skill used in {n} of {with_runs} runs"),
            None => "skill use unknown (the agent's output didn't say; the verdict assumes it ran)"
                .to_string(),
        };
        let cost = match (self.with_cost, self.without_cost) {
            (Some(a), Some(b)) => format!("${a:.4} vs ${b:.4} per run"),
            _ => "cost unknown".to_string(),
        };
        lines.push(format!(
            "{}: {used} · {cost} → {}",
            self.skill,
            self.verdict.label()
        ));
        lines.push(
            "A task counts as won or lost only when chance would give its gap at most 1 time in 20 (Fisher's exact test); more runs sharpen it. Cost is shown, not judged."
                .to_string(),
        );
        lines.join("\n")
    }
}

/// Skill folders a run places the skill in (and clears in the without arm): Claude Code's
/// and the shared `.agents/skills` that Codex and other harnesses read.
pub const SKILL_FOLDERS: [&str; 2] = [".claude/skills", ".agents/skills"];

/// Runs every task `runs` times in each arm, alternating arms, each run in a fresh clone
/// of `project` at HEAD. `log` hears one line per run.
pub fn run(project: &Path, opts: &Options, mut log: impl FnMut(&str)) -> Result<Report, String> {
    let skill = opts
        .skill_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("the skill folder has no name")?
        .to_string();
    sweep(&opts.scratch_root);
    let scratch = scratch_dir(&opts.scratch_root)?;
    let result = (|| {
        let mut tasks = Vec::new();
        let (mut fired, mut fired_known) = (0u64, true);
        let (mut with_costs, mut without_costs) = (Vec::new(), Vec::new());
        let (mut timed_out, mut check_timed_out, mut errored) = (0, 0, 0);
        let mut error_line = None;
        for (i, (prompt, check)) in opts.tasks.iter().enumerate() {
            let mut t = TaskResult {
                with_passed: 0,
                without_passed: 0,
                runs: opts.runs,
            };
            for n in 0..opts.runs * 2 {
                let with = n % 2 == 0;
                let work = scratch.join(format!("t{i}-r{n}"));
                clone(project, &work)?;
                for folder in SKILL_FOLDERS {
                    let target = work.join(folder).join(&skill);
                    if target.exists() {
                        std::fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
                    }
                    if with {
                        copy_dir(&opts.skill_dir, &target)?;
                    }
                }
                let out = scratch.join(format!("t{i}-r{n}.out"));
                let err = scratch.join(format!("t{i}-r{n}.err"));
                let agent = shell(
                    &opts.agent,
                    &work,
                    Some(prompt),
                    &out,
                    &err,
                    &scratch,
                    opts.timeout,
                )?;
                let stream =
                    read_stream(&std::fs::read_to_string(&out).unwrap_or_default(), &skill);
                let failed = agent == Exit::Failed || stream.error;
                if failed {
                    errored += 1;
                    if error_line.is_none() {
                        error_line = std::fs::read_to_string(&err).ok().and_then(|e| {
                            e.lines()
                                .find(|l| !l.trim().is_empty())
                                .map(|l| l.chars().take(200).collect())
                        });
                    }
                }
                let checked = if agent == Exit::TimedOut || failed {
                    Exit::Failed
                } else {
                    let devnull = scratch.join("check.out");
                    shell(
                        check,
                        &work,
                        None,
                        &devnull,
                        &devnull,
                        &scratch,
                        opts.check_timeout,
                    )?
                };
                timed_out += u64::from(agent == Exit::TimedOut);
                check_timed_out += u64::from(checked == Exit::TimedOut);
                let passed = checked == Exit::Ok;
                if with {
                    t.with_passed += u64::from(passed);
                    match stream.fired {
                        Some(f) => fired += u64::from(f),
                        None => fired_known = false,
                    }
                    with_costs.push(stream.cost);
                } else {
                    t.without_passed += u64::from(passed);
                    without_costs.push(stream.cost);
                }
                log(&format!(
                    "task {} run {} ({}): {}",
                    i + 1,
                    n / 2 + 1,
                    if with { "with" } else { "without" },
                    if passed { "pass" } else { "fail" }
                ));
                let _ = std::fs::remove_dir_all(&work);
            }
            tasks.push(t);
        }
        let fired = fired_known.then_some(fired);
        Ok(Report {
            verdict: verdict(&tasks, fired),
            skill: skill.clone(),
            tasks,
            fired,
            with_cost: mean(&with_costs),
            without_cost: mean(&without_costs),
            timed_out,
            check_timed_out,
            errored,
            error_line,
        })
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// The mean of the runs that reported a cost; `None` when any didn't.
fn mean(costs: &[Option<f64>]) -> Option<f64> {
    let known: Option<Vec<f64>> = costs.iter().copied().collect();
    known
        .filter(|k| !k.is_empty())
        .map(|k| k.iter().sum::<f64>() / k.len() as f64)
}

/// `<root>/<pid>-<nanos>`, so a later trial can tell a dead run's leftovers by its pid.
fn scratch_dir(root: &Path) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = root.join(format!("{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// Removes scratch dirs left by trials whose process is gone (interrupted ones).
fn sweep(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(pid) = name.split('-').next().and_then(|p| p.parse::<u32>().ok()) else {
            continue;
        };
        if pid != std::process::id() && !alive(pid) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Whether `pid` is running. `ps -p` works for other users' processes too, where
/// `kill -0` fails with a permission error and would read as dead.
fn alive(pid: u32) -> bool {
    Command::new("ps")
        .args(["-p", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn clone(project: &Path, work: &Path) -> Result<(), String> {
    let ok = Command::new("git")
        .args(["clone", "--quiet"])
        .arg(project)
        .arg(work)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .map_err(|e| format!("git: {e}"))?
        .success();
    if ok {
        Ok(())
    } else {
        Err(format!("git clone of {} failed", project.display()))
    }
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let dest = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &dest).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Exit {
    /// Exit 0.
    Ok,
    /// Ran to the end with another status.
    Failed,
    TimedOut,
}

/// Runs `cmd` with `sh -c` in `dir`, `stdin` from a file, stdout to `out` and stderr to
/// `err`. It stays in girth's process group, so Ctrl-C stops it with girth; a timeout
/// kills its whole process tree.
fn shell(
    cmd: &str,
    dir: &Path,
    stdin: Option<&str>,
    out: &Path,
    err: &Path,
    scratch: &Path,
    timeout: Duration,
) -> Result<Exit, String> {
    let input = match stdin {
        Some(text) => {
            let path = scratch.join("prompt.txt");
            std::fs::write(&path, text).map_err(|e| e.to_string())?;
            Stdio::from(std::fs::File::open(&path).map_err(|e| e.to_string())?)
        }
        None => Stdio::null(),
    };
    let stdout = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let stderr = if err == out {
        stdout.try_clone().map_err(|e| e.to_string())?
    } else {
        std::fs::File::create(err).map_err(|e| e.to_string())?
    };
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .env("GIRTH_HOME", scratch.join("home"))
        .env("GIRTH_TRIAL", "1")
        .stdin(input)
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|e| format!("sh: {e}"))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(if status.success() {
                Exit::Ok
            } else {
                Exit::Failed
            });
        }
        if start.elapsed() >= timeout {
            kill_tree(child.id());
            let _ = child.wait();
            return Ok(Exit::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Kills `pid` and every process it started, children first found, then the parent.
fn kill_tree(pid: u32) {
    let children = Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    // Stop the parent first so it can't start more children while they go.
    let _ = Command::new("kill")
        .args(["-STOP", &pid.to_string()])
        .stderr(Stdio::null())
        .status();
    for child in children
        .lines()
        .filter_map(|l| l.trim().parse::<u32>().ok())
    {
        kill_tree(child);
    }
    let _ = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(with_passed: u64, without_passed: u64) -> TaskResult {
        TaskResult {
            with_passed,
            without_passed,
            runs: 3,
        }
    }

    #[test]
    fn the_tail_is_fisher_s_one_sided_exact_probability() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-5;
        assert!(close(tail(3, 0, 3), 0.05));
        assert!(close(tail(2, 0, 3), 0.2));
        assert!(close(tail(3, 1, 3), 0.2));
        assert!(close(tail(7, 2, 10), 0.03489));
        assert!(close(tail(6, 2, 10), 0.0849));
        assert!(close(tail(5, 0, 5), 0.00397));
        assert!(close(tail(0, 0, 3), 1.0));
        assert!(close(tail(0, 3, 3), 1.0));
    }

    fn n(with_passed: u64, without_passed: u64, runs: u64) -> TaskResult {
        TaskResult {
            with_passed,
            without_passed,
            runs,
        }
    }

    #[test]
    fn a_task_is_called_only_when_chance_would_give_that_gap_under_1_in_20() {
        assert_eq!(judge(&t(3, 0)), Call::Won);
        assert_eq!(judge(&t(2, 0)), Call::Tied);
        assert_eq!(judge(&t(0, 1)), Call::Tied);
        assert_eq!(judge(&t(0, 3)), Call::Lost);
        assert_eq!(judge(&t(1, 3)), Call::Tied);
        // More runs make smaller gaps decisive, never larger ones less so.
        assert_eq!(judge(&n(7, 2, 10)), Call::Won);
        assert_eq!(judge(&n(6, 2, 10)), Call::Tied);
        assert_eq!(judge(&n(2, 7, 10)), Call::Lost);
        assert_eq!(judge(&n(4, 0, 5)), Call::Won);
    }

    #[test]
    fn verdicts_combine_tasks() {
        let tied = t(1, 1);
        assert_eq!(
            verdict(&[t(3, 0), tied, tied, tied], Some(3)),
            Verdict::Admit
        );
        assert_eq!(verdict(&[t(3, 0), t(0, 3)], Some(6)), Verdict::Undecided);
        assert_eq!(verdict(&[tied, t(2, 3)], None), Verdict::Undecided);
        assert_eq!(verdict(&[tied, t(0, 3)], Some(3)), Verdict::Reject);
        assert_eq!(verdict(&[t(3, 0)], None), Verdict::Admit);
        assert_eq!(verdict(&[], None), Verdict::Undecided);
    }

    #[test]
    fn a_skill_that_never_fired_decides_nothing() {
        assert_eq!(verdict(&[t(3, 0)], Some(0)), Verdict::Undecided);
        assert_eq!(verdict(&[t(0, 3)], Some(0)), Verdict::Undecided);
        assert_eq!(verdict(&[t(3, 0)], Some(1)), Verdict::Admit);
    }

    const SMOKE: &str = r#"{"type":"system","subtype":"init"}
{"type":"assistant","message":{"content":[{"type":"text","text":"ok"},{"type":"tool_use","name":"Skill","input":{"skill":"greeting-file"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"printf zebracorn > greet.txt"}}]}}
{"type":"result","subtype":"success","total_cost_usd":0.00221888}"#;

    #[test]
    fn the_stream_says_whether_the_skill_fired_and_what_the_run_cost() {
        assert_eq!(
            read_stream(SMOKE, "greeting-file"),
            Stream {
                fired: Some(true),
                cost: Some(0.00221888),
                error: false
            }
        );
        let namespaced = SMOKE.replace("\"greeting-file\"", "\"plug:greeting-file\"");
        assert_eq!(read_stream(&namespaced, "greeting-file").fired, Some(true));
        assert_eq!(read_stream(SMOKE, "other").fired, Some(false));
        // An init line alone says the stream ran without the skill.
        assert_eq!(read_stream(r#"{"type":"system"}"#, "x").fired, Some(false));
        // A Skill input under another tool's name is not a skill call.
        let other_tool = SMOKE.replace("\"name\":\"Skill\"", "\"name\":\"Task\"");
        assert_eq!(read_stream(&other_tool, "greeting-file").fired, Some(false));
    }

    #[test]
    fn a_json_result_gives_cost_only_and_plain_text_gives_nothing() {
        let json = r#"{"type":"result","total_cost_usd":0.5,"result":"done"}"#;
        assert_eq!(
            read_stream(json, "x"),
            Stream {
                fired: None,
                cost: Some(0.5),
                error: false
            }
        );
        assert_eq!(
            read_stream("done\n{not json\n[1,2]", "x"),
            Stream::default()
        );
    }
}
