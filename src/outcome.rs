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

/// Bash calls read per turn; the rest are ignored.
pub const MAX_CALLS: usize = 64;

/// What `command` does: its simple commands, read through wrappers and chains.
pub fn classify(command: &str) -> Kinds {
    let mut kinds = Kinds::default();
    let text = strip_heredocs(command);
    for segment in split_unquoted(&text, &['&', '|', ';', '\n']) {
        let words = normalise(segment);
        let w: Vec<&str> = words.iter().map(String::as_str).collect();
        kinds.test |= is_test(&w);
        kinds.commit |= is_commit(&w);
        kinds.pr |= matches!(w.as_slice(), ["gh", "pr", "create" | "merge", ..]);
    }
    kinds
}

/// `text` split on `seps` outside single and double quotes.
fn split_unquoted<'a>(text: &'a str, seps: &[char]) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let (mut start, mut quote, mut escaped) = (0, None, false);
    for (i, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, c) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('"'), '\\') | (None, '\\') => escaped = true,
            (None, '\'' | '"') => quote = Some(c),
            (None, c) if seps.contains(&c) => {
                parts.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
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
    let segment = segment
        .trim()
        .trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace());
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
            name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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
        || starts_word(&lower, "not ok")
        || lower.contains("some tests failed")
        || lower.contains("could not compile")
        || (t.starts_with("make: ***") && t.contains("Error"));
    let mut passed = lower.contains("test result: ok.")
        || lower.contains("all tests passed")
        || starts_word(t, "PASS")
        || t.starts_with("OK (")
        || t.starts_with("ok ")
        || t.starts_with("ok\t")
        || (t.starts_with("Ran ") && (t.contains(" tests across") || t.contains(" test across")));
    let raw: Vec<&str> = lower.split_whitespace().collect();
    let word = |w: &str| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
    for pair in raw.windows(2) {
        let (a, b) = (word(pair[0]), word(pair[1]));
        // "3 failed": a count, then what it counts. Not "5, Failures:", where
        // the count belongs to the key before it.
        if let Ok(n) = a.parse::<u64>() {
            if !pair[0].ends_with([',', ';', ':']) {
                match b.as_str() {
                    "failed" | "failure" | "failures" | "error" | "errors" | "fail" | "failing" => {
                        failed |= n > 0;
                        passed |= n == 0 && b.starts_with("failure");
                    }
                    "passed" | "pass" | "passing" => passed |= n > 0,
                    _ => {}
                }
            }
        }
        // "Failures: 1", "errors: 0", node's "fail 1": a key, then its count.
        if pair[0].ends_with(':') || a == "fail" {
            if let Ok(n) = b.parse::<u64>() {
                match a.as_str() {
                    "failed" | "failure" | "failures" | "fail" => {
                        failed |= n > 0;
                        passed |= n == 0;
                    }
                    "error" | "errors" => failed |= n > 0,
                    _ => {}
                }
            }
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
            // A filter at the end of the pipe owns the exit status: grep
            // finding nothing in a passing run exits 1.
            let error = call.error && !ends_in_filter(&call.command);
            found.push(verdict(&call.output, error));
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

/// The command's last pipeline ends in a text filter (`| grep`, `| tail`, …).
fn ends_in_filter(command: &str) -> bool {
    let text = strip_heredocs(command)
        .replace("&&", "\n")
        .replace("||", "\n");
    let Some(pipeline) = split_unquoted(&text, &['\n', ';'])
        .into_iter()
        .map(str::trim)
        .rfind(|p| !p.is_empty())
    else {
        return false;
    };
    let stages = split_unquoted(pipeline, &['|']);
    stages.len() > 1
        && normalise(stages[stages.len() - 1])
            .first()
            .is_some_and(|w| {
                matches!(
                    w.as_str(),
                    "grep"
                        | "egrep"
                        | "fgrep"
                        | "rg"
                        | "tail"
                        | "head"
                        | "sed"
                        | "awk"
                        | "cut"
                        | "sort"
                        | "uniq"
                        | "wc"
                        | "tr"
                )
            })
}

/// Git's answer when a commit made nothing.
fn no_commit(output: &str) -> bool {
    [
        "nothing to commit",
        "no changes added to commit",
        "nothing added to commit",
    ]
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
        for command in [
            "git commit --dry-run",
            "git log",
            "git commit-tree abc",
            "git status",
        ] {
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
        assert_eq!(
            verdict("test result: ok. 1 passed", true),
            Outcome::TestFail
        );
        assert_eq!(verdict("", false), Outcome::TestUntold);
        assert_eq!(
            verdict("Compiling girth v0.9.0\nwarning: unused", false),
            Outcome::TestUntold
        );
        assert_eq!(verdict("0 failed", false), Outcome::TestUntold);
    }

    #[test]
    fn only_the_tail_is_read() {
        let early = format!("test result: FAILED. 1 failed\n{}", "x".repeat(TAIL));
        assert_eq!(
            verdict(&format!("{early}\ntest result: ok. 1 passed"), false),
            Outcome::TestPass
        );
        // A tail cut inside a multi-byte character must not panic.
        let wide = format!("é{}test result: ok. 1 passed", "é".repeat(TAIL));
        assert_eq!(verdict(&wide, false), Outcome::TestPass);
    }

    #[test]
    fn heredoc_words_end_where_they_should() {
        assert!(test("cat <<'EOF'\nx\nEOF\ncargo test"));
        assert!(test("cat <<END_OF\ncargo build\nEND_OF\ncargo test"));
        assert!(!test("cat <<END_OF\ncargo test\nEND_OF"));
    }

    #[test]
    fn only_real_assignments_are_stripped() {
        assert!(test("foo=1 cargo test"));
        assert!(test("_X=1 cargo test"));
        for command in ["=x cargo test", "1A=x cargo test", "A-B=x cargo test"] {
            assert!(!test(command), "{command}");
        }
    }

    #[test]
    fn summary_edge_cases() {
        // A word that starts like a marker is not the marker.
        assert_eq!(
            verdict("FAIL_FAST=1\ntest result: ok. 1 passed", false),
            Outcome::TestPass
        );
        assert_eq!(verdict("PASSWORD reset", false), Outcome::TestUntold);
        // "Error" alone is not make's failure line, nor is make's line without it.
        assert_eq!(
            verdict("test result: ok. 3 passed\nhandles TypeError", false),
            Outcome::TestPass
        );
        assert_eq!(
            verdict("make: *** Waiting for unfinished jobs", false),
            Outcome::TestUntold
        );
        assert_eq!(
            verdict("Ran 1 test across 1 file. [3.00ms]", false),
            Outcome::TestPass
        );
        assert_eq!(
            verdict("Ran 4 tests across 2 files.", false),
            Outcome::TestPass
        );
        assert_eq!(verdict("Ran out of disk space", false), Outcome::TestUntold);
        assert_eq!(
            verdict("tests across the board", false),
            Outcome::TestUntold
        );
        assert_eq!(verdict("0 passed", false), Outcome::TestUntold);
    }

    #[test]
    fn the_tail_is_exactly_tail_characters() {
        // One character more would complete "FAIL" at the start of the window.
        let output = format!("FAIL x\n{}", "y".repeat(TAIL - 6));
        assert_eq!(output.chars().count(), TAIL + 1);
        assert_eq!(verdict(&output, false), Outcome::TestUntold);
        let exact = format!("FAIL x\n{}", "y".repeat(TAIL - 7));
        assert_eq!(exact.chars().count(), TAIL);
        assert_eq!(verdict(&exact, false), Outcome::TestFail);
    }

    #[test]
    fn key_value_summaries_are_read_by_key() {
        for output in [
            "Tests run: 5, Failures: 0, Errors: 0, Skipped: 0",
            "OK (5 tests, 7 assertions)",
            "  3 passing (12ms)",
            "00:02 +12: All tests passed!",
        ] {
            assert_eq!(verdict(output, false), Outcome::TestPass, "{output}");
        }
        for output in [
            "Tests run: 5, Failures: 1, Errors: 0",
            "Tests run: 5, Failures: 0, Errors: 2",
            "errors: 1",
            "Assertions: 7, Failures: 1.",
            "ok 1 - adds\nnot ok 2 - subtracts",
            "  3 passing (12ms)\n  1 failing",
            "00:02 +11 -1: Some tests failed.",
        ] {
            assert_eq!(verdict(output, false), Outcome::TestFail, "{output}");
        }
    }

    #[test]
    fn a_filter_at_the_end_of_the_pipe_makes_the_exit_status_meaningless() {
        // grep found nothing and exited 1: the run itself passed.
        let quiet = call("cargo test 2>&1 | grep -E 'FAILED|panicked'", "", true);
        assert_eq!(outcomes(&[quiet]), vec![Outcome::TestUntold]);
        let summary = call(
            "cargo test 2>&1 | grep 'test result'",
            "test result: ok. 3 passed",
            true,
        );
        assert_eq!(outcomes(&[summary]), vec![Outcome::TestPass]);
        let failed = call(
            "cargo test 2>&1 | tail -3",
            "test result: FAILED. 1 failed",
            true,
        );
        assert_eq!(outcomes(&[failed]), vec![Outcome::TestFail]);
        // Without a filter at the end, an error still fails the run.
        assert_eq!(
            outcomes(&[call("cargo test", "", true)]),
            vec![Outcome::TestFail]
        );
        assert_eq!(
            outcomes(&[call("cargo test | tee log && echo done", "", true)]),
            vec![Outcome::TestFail]
        );
    }

    #[test]
    fn separators_inside_quotes_do_not_split() {
        let message = "git commit -m \"fix; cargo test now passes\ncargo test | grep ok\"";
        assert_eq!(
            classify(message),
            Kinds {
                commit: true,
                ..Kinds::default()
            }
        );
        assert!(test("echo 'a | b' && cargo test"));
        assert!(test("echo \"it's\" && cargo test"));
    }

    #[test]
    fn escaped_quotes_do_not_open_a_quote() {
        assert!(test(r"echo \' && cargo test"));
        assert!(test(r#"echo "a \" ; b" && cargo test"#));
    }

    #[test]
    fn only_a_filter_after_a_pipe_owns_the_exit_status() {
        // tee is not a filter: its stage's error stands.
        assert_eq!(
            outcomes(&[call("cargo test 2>&1 | tee log", "", true)]),
            vec![Outcome::TestFail]
        );
        // A grep of its own, not after a pipe: the error stands too.
        assert_eq!(
            outcomes(&[call("cargo test; grep -q ok log", "", true)]),
            vec![Outcome::TestFail]
        );
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
            call(
                "cargo test 2>&1 | tail -3",
                "test result: FAILED. 1 failed",
                false,
            ),
            call("ls", "", false),
            call(
                "cargo test && git commit -m x",
                "test result: ok. 3 passed\n[main abc] x",
                false,
            ),
            call(
                "gh pr create --fill",
                "https://github.com/o/r/pull/1",
                false,
            ),
        ];
        assert_eq!(
            outcomes(&calls),
            vec![
                Outcome::TestFail,
                Outcome::TestPass,
                Outcome::Commit,
                Outcome::Pr
            ]
        );
    }

    #[test]
    fn failed_or_empty_commits_and_backgrounded_calls_do_not_count() {
        let mut bg = call("cargo test", "test result: ok. 1 passed", false);
        bg.background = true;
        let calls = [
            call("git commit -m x", "error: pathspec", true),
            call(
                "git commit -m x | tail -2",
                "nothing to commit, working tree clean",
                false,
            ),
            call(
                "git commit -m x",
                "no changes added to commit (use \"git add\")",
                false,
            ),
            call("gh pr merge 3", "GraphQL: not mergeable", true),
            bg,
        ];
        assert_eq!(outcomes(&calls), Vec::<Outcome>::new());
    }

    #[test]
    fn outcomes_serialise_as_snake_case() {
        let json =
            serde_json::to_string(&[Outcome::TestPass, Outcome::TestUntold, Outcome::Pr]).unwrap();
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
