//! Skill trials: run the same task with and without a skill, grade both with a check
//! command, and judge whether the skill earned its place. A task is decided only by a
//! difference of two or more passes, so a single flipped run never sets a verdict (the
//! 2026-11-13 pilot). Cost is reported, never decisive: it varies too much at n = 3.

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

/// Won by two or more passes, lost by two or more, else tied.
pub fn judge(t: &TaskResult) -> Call {
    let d = t.with_passed as i64 - t.without_passed as i64;
    if d >= 2 {
        Call::Won
    } else if d <= -2 {
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

/// How long a check may run.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(600);

pub struct Options {
    pub skill_dir: PathBuf,
    /// (prompt, check command) per task.
    pub tasks: Vec<(String, String)>,
    pub runs: u64,
    pub agent: String,
    pub timeout: Duration,
}

/// What a trial found, before it becomes a ledger event.
#[derive(Debug, PartialEq)]
pub struct Report {
    pub skill: String,
    pub tasks: Vec<TaskResult>,
    pub fired: Option<u64>,
    pub with_cost: Option<f64>,
    pub without_cost: Option<f64>,
    pub timed_out: u64,
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
                "{} runs timed out (counted as fails)",
                self.timed_out
            ));
        }
        let with_runs: u64 = self.tasks.iter().map(|t| t.runs).sum();
        let used = match self.fired {
            Some(n) => format!("skill used in {n} of {with_runs} runs"),
            None => "skill use unknown".to_string(),
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
            "A task counts as won or lost only by a difference of 2 or more passes; cost is shown, not judged."
                .to_string(),
        );
        lines.join("\n")
    }
}

/// Runs every task `runs` times in each arm, alternating arms, each run in a fresh clone
/// of `project` at HEAD. `log` hears one line per run.
pub fn run(project: &Path, opts: &Options, mut log: impl FnMut(&str)) -> Result<Report, String> {
    let skill = opts
        .skill_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("the skill folder has no name")?
        .to_string();
    let scratch = scratch_dir()?;
    let result = (|| {
        let mut tasks = Vec::new();
        let (mut fired, mut fired_known) = (0u64, true);
        let (mut with_costs, mut without_costs) = (Vec::new(), Vec::new());
        let mut timed_out = 0;
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
                let target = work.join(".claude/skills").join(&skill);
                if target.exists() {
                    std::fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
                }
                if with {
                    copy_dir(&opts.skill_dir, &target)?;
                }
                let out = scratch.join(format!("t{i}-r{n}.out"));
                let agent = shell(
                    &opts.agent,
                    &work,
                    Some(prompt),
                    &out,
                    &scratch,
                    opts.timeout,
                )?;
                let stream =
                    read_stream(&std::fs::read_to_string(&out).unwrap_or_default(), &skill);
                // A run that timed out fails, whatever it left behind.
                let passed = agent != Exit::TimedOut
                    && shell(
                        check,
                        &work,
                        None,
                        &scratch.join("check.out"),
                        &scratch,
                        CHECK_TIMEOUT,
                    )? == Exit::Ok;
                timed_out += u64::from(agent == Exit::TimedOut);
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

fn scratch_dir() -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("girth-trial-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
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
    /// Exit 0 (for a check: pass).
    Ok,
    /// Ran to the end with another status (an agent's own status isn't judged).
    Failed,
    TimedOut,
}

/// Runs `cmd` with `sh -c` in `dir`, `stdin` from a file, stdout to `out`, in its own
/// process group so a timeout kills everything it started.
fn shell(
    cmd: &str,
    dir: &Path,
    stdin: Option<&str>,
    out: &Path,
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
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .env("GIRTH_HOME", scratch.join("home"))
        .env("GIRTH_TRIAL", "1")
        .stdin(input)
        .stdout(file)
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command.spawn().map_err(|e| format!("sh: {e}"))?;
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
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .stderr(Stdio::null())
                .status();
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Exit::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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
    fn a_task_needs_two_passes_of_difference() {
        assert_eq!(judge(&t(3, 0)), Call::Won);
        assert_eq!(judge(&t(2, 0)), Call::Won);
        assert_eq!(judge(&t(0, 1)), Call::Tied);
        assert_eq!(judge(&t(2, 1)), Call::Tied);
        assert_eq!(judge(&t(0, 2)), Call::Lost);
        assert_eq!(judge(&t(1, 3)), Call::Lost);
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
                cost: Some(0.00221888)
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
                cost: Some(0.5)
            }
        );
        assert_eq!(
            read_stream("done\n{not json\n[1,2]", "x"),
            Stream::default()
        );
    }
}
