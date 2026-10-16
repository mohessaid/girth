//! Outcomes: what a turn's Bash calls achieved (a test run's verdict, a
//! commit, a pull request), read from each command and the end of its output.
//! Only the labels are kept; command text and output never reach the ledger.

use serde::{Deserialize, Serialize};

/// One thing a Bash call achieved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    TestPass,
    TestFail,
    /// A test run whose output carried no runner summary.
    TestUntold,
    Commit,
    Pr,
}

/// What a command line does, as far as outcomes go.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Kinds {
    pub test: bool,
    pub commit: bool,
    pub pr: bool,
}

/// One Bash call, as the mod hands it over.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Call {
    #[serde(default)]
    pub command: String,
    /// The end of the output, as the model read it.
    #[serde(default)]
    pub output: String,
    /// The tool reported an error (for Bash, a non-zero exit of the last command).
    #[serde(default)]
    pub error: bool,
    /// Backgrounded or interrupted: the output is not the run's.
    #[serde(default)]
    pub background: bool,
}

/// Characters of output the verdict reads, from the end.
pub const TAIL: usize = 4_000;

/// What `command` does: its simple commands, read through wrappers and chains.
pub fn classify(command: &str) -> Kinds {
    let mut kinds = Kinds::default();
    let text = strip_heredocs(command);
    for segment in text.split(['&', '|', ';', '\n']) {
        let words = normalise(segment);
        let w: Vec<&str> = words.iter().map(String::as_str).collect();
        kinds.test |= is_test(&w);
        kinds.commit |= is_commit(&w);
        kinds.pr |= matches!(w.as_slice(), ["gh", "pr", "create" | "merge", ..]);
    }
    kinds
}

/// `command` without heredoc bodies: their lines are data, not commands.
fn strip_heredocs(command: &str) -> String {
    let mut out = String::new();
    let mut until: Option<String> = None;
    for line in command.lines() {
        if let Some(word) = &until {
            if line.trim() == word {
                until = None;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        until = heredoc_word(line);
    }
    out
}

fn heredoc_word(line: &str) -> Option<String> {
    let at = line.find("<<")?;
    let rest = &line[at + 2..];
    if rest.starts_with('<') {
        return None;
    }
    let rest = rest.trim_start_matches('-').trim_start();
    let word: String = rest
        .trim_start_matches(['\'', '"'])
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!word.is_empty()).then_some(word)
}

/// A simple command's words, with wrappers (`rtk`, `sudo`, `timeout 60`,
/// `NAME=value`, …) and the program's directory stripped. `cd` yields nothing.
fn normalise(segment: &str) -> Vec<String> {
    let segment = segment.trim().trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace());
    let mut words: Vec<&str> = segment.split_whitespace().collect();
    loop {
        let skip = match words.as_slice() {
            ["rtk", "proxy", ..] | ["timeout", _, ..] | ["uv", "run", ..] => 2,
            ["nice", "-n", _, ..] => 3,
            ["rtk" | "sudo" | "time" | "env", ..] => 1,
            ["mise", "exec", rest @ ..] => match rest.iter().position(|w| *w == "--") {
                Some(i) => i + 3,
                None => break,
            },
            [first, ..] if is_assignment(first) => 1,
            _ => break,
        };
        words.drain(..skip.min(words.len()));
    }
    let mut words: Vec<String> = words.into_iter().map(str::to_string).collect();
    if let Some(first) = words.first_mut() {
        if first == "cd" {
            return Vec::new();
        }
        if let Some((_, program)) = first.rsplit_once('/') {
            *first = program.to_string();
        }
    }
    words
}

fn is_assignment(word: &str) -> bool {
    match word.split_once('=') {
        Some((name, _)) => {
            !name.is_empty()
                && name.starts_with(|c: char| c.is_ascii_uppercase() || c == '_')
                && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        }
        None => false,
    }
}

fn is_test(w: &[&str]) -> bool {
    match w {
        ["cargo", rest @ ..] => {
            let rest = match rest {
                [toolchain, rest @ ..] if toolchain.starts_with('+') => rest,
                _ => rest,
            };
            matches!(rest, ["test" | "nextest" | "llvm-cov", ..])
        }
        ["make", "check" | "test", ..] => true,
        ["npm" | "pnpm" | "yarn" | "bun", "test", ..] => true,
        ["npm" | "pnpm" | "yarn" | "bun", "run", "test", ..] => true,
        ["npx", "vitest" | "jest", ..] | ["npx", "playwright", "test", ..] => true,
        ["vitest" | "jest" | "pytest" | "rspec" | "phpunit", ..] => true,
        [python, "-m", "pytest", ..] => python.starts_with("python"),
        ["go" | "deno" | "flutter" | "dart" | "mix", "test", ..] => true,
        ["claude", "plugin", "test", ..] => true,
        _ => false,
    }
}

fn is_commit(w: &[&str]) -> bool {
    let Some((&"git", mut rest)) = w.split_first() else {
        return false;
    };
    while let ["-C" | "-c", _, tail @ ..] = rest {
        rest = tail;
    }
    matches!(rest, ["commit", ..]) && !rest.contains(&"--dry-run")
}

/// A test run's verdict from the runner's summary near the end of its output.
/// Agents pipe test output through `tail` or `grep`, so the exit status is
/// usually the filter's: the summary decides, and an error only adds a failure.
pub fn verdict(output: &str, error: bool) -> Outcome {
    let start = output
        .char_indices()
        .rev()
        .nth(TAIL - 1)
        .map_or(0, |(i, _)| i);
    let tail = &output[start..];
    let (mut passed, mut failed) = (false, error);
    for line in tail.lines() {
        let (p, f) = summary(line);
        passed |= p;
        failed |= f;
    }
    if failed {
        Outcome::TestFail
    } else if passed {
        Outcome::TestPass
    } else {
        Outcome::TestUntold
    }
}

/// (passing, failing) markers on one line of output.
fn summary(line: &str) -> (bool, bool) {
    let t = line.trim_start();
    let lower = t.to_lowercase();
    let mut failed = lower.contains("test result: failed")
        || starts_word(t, "FAIL")
        || starts_word(t, "FAILED")
        || lower.contains("could not compile")
        || (t.starts_with("make: ***") && t.contains("Error"));
    let mut passed = lower.contains("test result: ok.")
        || starts_word(t, "PASS")
        || t.starts_with("ok ")
        || t.starts_with("ok\t")
        || (t.starts_with("Ran ") && (t.contains(" tests across") || t.contains(" test across")));
    let words: Vec<&str> = lower
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .collect();
    for pair in words.windows(2) {
        match (pair[0].parse::<u64>().ok(), pair[1], pair[0], pair[1].parse::<u64>().ok()) {
            (Some(n), "failed" | "failure" | "failures" | "error" | "errors" | "fail", _, _) => {
                failed |= n > 0;
                passed |= n == 0 && pair[1].starts_with("failure");
            }
            (Some(n), "passed" | "pass", _, _) => passed |= n > 0,
            (_, _, "fail", Some(n)) => {
                failed |= n > 0;
                passed |= n == 0;
            }
            _ => {}
        }
    }
    (passed, failed)
}

/// `line` starts with `word` followed by a non-word character or its end.
fn starts_word(line: &str, word: &str) -> bool {
    line.strip_prefix(word)
        .is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
}

/// The outcomes of a turn's Bash calls, in call order.
pub fn outcomes(calls: &[Call]) -> Vec<Outcome> {
    let mut found = Vec::new();
    for call in calls.iter().filter(|c| !c.background) {
        let kinds = classify(&call.command);
        if kinds.test {
            found.push(verdict(&call.output, call.error));
        }
        if kinds.commit && !call.error && !no_commit(&call.output) {
            found.push(Outcome::Commit);
        }
        if kinds.pr && !call.error {
            found.push(Outcome::Pr);
        }
    }
    found
}

/// Git's answer when a commit made nothing.
fn no_commit(output: &str) -> bool {
    ["nothing to commit", "no changes added to commit", "nothing added to commit"]
        .iter()
        .any(|m| output.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn test(command: &str) -> bool {
        classify(command).test
    }

    #[test]
    fn test_runners_are_recognised() {
        for command in [
            "cargo test",
            "cargo test --lib -- outcome",
            "cargo +nightly test",
            "cargo nextest run",
            "cargo llvm-cov --summary-only",
            "make check",
            "make test",
            "npm test",
            "npm run test -- --watch=false",
            "pnpm test",
            "yarn test",
            "bun test",
            "npx vitest run",
            "npx jest src",
            "npx playwright test",
            "vitest run",
            "jest",
            "pytest -q tests/",
            "python -m pytest",
            "python3 -m pytest -x",
            "go test ./...",
            "deno test",
            "flutter test",
            "dart test",
            "mix test test/app_test.exs",
            "rspec spec",
            "phpunit",
            "claude plugin test .",
        ] {
            assert!(test(command), "{command}");
        }
    }

    #[test]
    fn other_commands_are_not_tests() {
        for command in [
            "cargo build",
            "make dist",
            "npm install",
            "grep -rn pytest src",
            "echo cargo test",
            "cat tests/cli.rs",
            "make testing-docs",
            "gotest",
        ] {
            assert_eq!(classify(command), Kinds::default(), "{command}");
        }
    }

    #[test]
    fn wrappers_and_chains_are_seen_through() {
        for command in [
            "rtk cargo test",
            "rtk proxy cargo test 2>&1 | tail -20",
            "cd /repo && cargo test",
            "cd /repo; make check",
            "/usr/bin/make check",
            "sudo pytest",
            "time go test ./...",
            "env RUST_LOG=debug cargo test",
            "RUST_BACKTRACE=1 PATH=/usr/bin:$PATH cargo test",
            "timeout 120 mix test",
            "nice -n 10 cargo test",
            "mise exec elixir@1.18 -- mix test",
            "uv run pytest",
            "(cd app && npm test)",
            "cargo fmt && cargo clippy && cargo test",
        ] {
            assert!(test(command), "{command}");
        }
    }

    #[test]
    fn heredoc_bodies_are_not_commands() {
        let command = "python3 - <<'EOF'\nimport os\npytest = 1\ncargo test\nEOF";
        assert_eq!(classify(command), Kinds::default());
        let command = "cat > notes.md <<EOF\ngit commit -m x\nEOF\ncargo test";
        assert_eq!(
            classify(command),
            Kinds {
                test: true,
                ..Kinds::default()
            }
        );
    }

    #[test]
    fn commits_and_pull_requests_are_recognised() {
        for command in [
            "git commit -m 'fix'",
            "rtk git commit -qm \"x\"",
            "/usr/bin/git commit --amend --no-edit",
            "git -C /repo commit -m x",
            "git add -A && git commit -m x && git push",
        ] {
            assert!(classify(command).commit, "{command}");
        }
        for command in ["git commit --dry-run", "git log", "git commit-tree abc", "git status"] {
            assert!(!classify(command).commit, "{command}");
        }
        for command in ["gh pr create --fill", "gh pr merge 12 --squash"] {
            assert!(classify(command).pr, "{command}");
        }
        for command in ["gh pr view 12", "gh pr list"] {
            assert!(!classify(command).pr, "{command}");
        }
    }

    #[test]
    fn failing_summaries_fail() {
        for output in [
            "test result: FAILED. 3 passed; 1 failed; 0 ignored",
            "FAIL src/app.test.ts\n  ● adds",
            "FAILED tests/test_app.py::test_add - assert 1 == 2",
            "=== 2 failed, 10 passed in 0.31s ===",
            "Tests:       1 failed, 9 passed, 10 total",
            "12 tests, 1 failure",
            "4 properties, 30 tests, 2 failures, 1 excluded",
            "ℹ tests 5\nℹ pass 4\nℹ fail 1",
            " 3 pass\n 1 fail\nRan 4 tests across 1 file. [12.00ms]",
            "error[E0425]: cannot find value `x`\nerror: could not compile `girth` (lib test)",
            "make: *** [Makefile:12: check] Error 101",
            "--- FAIL: TestAdd (0.00s)\nFAIL\tgithub.com/x/y\t0.01s",
            "1 error in 0.20s",
        ] {
            assert_eq!(verdict(output, false), Outcome::TestFail, "{output}");
        }
    }

    #[test]
    fn passing_summaries_pass() {
        for output in [
            "test result: ok. 42 passed; 0 failed; 0 ignored",
            "=== 12 passed in 0.31s ===",
            "Tests:       10 passed, 10 total",
            "ok  \tgithub.com/x/y\t0.01s",
            "PASS src/app.test.ts",
            "Finished in 0.1 seconds\n12 tests, 0 failures",
            "ℹ tests 5\nℹ pass 5\nℹ fail 0",
            " 4 pass\n 0 fail\nRan 4 tests across 1 file. [12.00ms]",
            "Test Files  3 passed (3)\n     Tests  21 passed (21)",
        ] {
            assert_eq!(verdict(output, false), Outcome::TestPass, "{output}");
        }
    }

    #[test]
    fn an_error_fails_and_no_summary_is_untold() {
        assert_eq!(verdict("test result: ok. 1 passed", true), Outcome::TestFail);
        assert_eq!(verdict("", false), Outcome::TestUntold);
        assert_eq!(verdict("Compiling girth v0.9.0\nwarning: unused", false), Outcome::TestUntold);
        assert_eq!(verdict("0 failed", false), Outcome::TestUntold);
    }

    #[test]
    fn only_the_tail_is_read() {
        let early = format!("test result: FAILED. 1 failed\n{}", "x".repeat(TAIL));
        assert_eq!(verdict(&format!("{early}\ntest result: ok. 1 passed"), false), Outcome::TestPass);
        // A tail cut inside a multi-byte character must not panic.
        let wide = format!("é{}test result: ok. 1 passed", "é".repeat(TAIL));
        assert_eq!(verdict(&wide, false), Outcome::TestPass);
    }

    fn call(command: &str, output: &str, error: bool) -> Call {
        Call {
            command: command.into(),
            output: output.into(),
            error,
            background: false,
        }
    }

    #[test]
    fn calls_become_outcomes_in_order() {
        let calls = [
            call("cargo test 2>&1 | tail -3", "test result: FAILED. 1 failed", false),
            call("ls", "", false),
            call("cargo test && git commit -m x", "test result: ok. 3 passed\n[main abc] x", false),
            call("gh pr create --fill", "https://github.com/o/r/pull/1", false),
        ];
        assert_eq!(
            outcomes(&calls),
            vec![Outcome::TestFail, Outcome::TestPass, Outcome::Commit, Outcome::Pr]
        );
    }

    #[test]
    fn failed_or_empty_commits_and_backgrounded_calls_do_not_count() {
        let mut bg = call("cargo test", "test result: ok. 1 passed", false);
        bg.background = true;
        let calls = [
            call("git commit -m x", "error: pathspec", true),
            call("git commit -m x | tail -2", "nothing to commit, working tree clean", false),
            call("git commit -m x", "no changes added to commit (use \"git add\")", false),
            call("gh pr merge 3", "GraphQL: not mergeable", true),
            bg,
        ];
        assert_eq!(outcomes(&calls), Vec::<Outcome>::new());
    }

    #[test]
    fn outcomes_serialise_as_snake_case() {
        let json = serde_json::to_string(&[Outcome::TestPass, Outcome::TestUntold, Outcome::Pr]).unwrap();
        assert_eq!(json, r#"["test_pass","test_untold","pr"]"#);
    }

    proptest! {
        #[test]
        fn never_panics(command in ".{0,200}", output in ".{0,300}", error: bool) {
            let _ = classify(&command);
            let _ = verdict(&output, error);
            let _ = outcomes(&[Call { command, output, error, background: false }]);
        }

        #[test]
        fn a_piped_failing_run_fails(n in 1u32..500, filter in prop::sample::select(vec!["tail -5", "grep -E 'FAIL|ok'", "head -50"])) {
            let calls = [call(&format!("cargo test 2>&1 | {filter}"), &format!("test result: FAILED. 0 passed; {n} failed"), false)];
            prop_assert_eq!(outcomes(&calls), vec![Outcome::TestFail]);
        }
    }
}
