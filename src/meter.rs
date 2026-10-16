//! Meter: real token usage per turn, summarised from `turn` ledger events.

use serde::Serialize;

use crate::ledger::{Event, Record};
use crate::outcome::Outcome;
use crate::price;

/// Bytes per token for the Lens estimate (a conservative English-text ratio).
pub const BYTES_PER_TOKEN: u64 = 4;

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Summary {
    pub turns: u64,
    /// Requests across those turns (a turn without a count is one request).
    pub requests: u64,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// Estimated dollars over turns whose model has known rates.
    pub cost: f64,
    /// Turns whose model has no known rates (left out of `cost`).
    pub unpriced_turns: u64,
    /// Turns where Lens had trimmed the listing.
    pub lens_turns: u64,
    /// At least this many listing tokens were not sent (every request carries the listing).
    pub lens_tokens_saved: u64,
    /// The saving priced at cache-read rates: the lowest it could be.
    pub lens_cost_saved: f64,
    /// Outcomes of the turns' Bash calls.
    pub commits: u64,
    pub prs: u64,
    pub tests_passed: u64,
    pub tests_failed: u64,
    pub tests_untold: u64,
    /// The cost up to and including the last commit's turn, per commit; 0 without one.
    pub per_commit: f64,
    /// The cost of the turns after the last commit (all of them without one).
    pub since_commit: f64,
}

/// Summarise `turn` events, optionally for one project.
pub fn summarise(records: &[Record], project: Option<&str>) -> Summary {
    let mut s = Summary::default();
    let turns = records.iter().filter_map(|r| match &r.event {
        Event::Turn(t) if project.is_none_or(|p| p == t.project) => Some(t),
        _ => None,
    });
    for t in turns {
        s.turns += 1;
        let requests = t.requests.unwrap_or(1).max(1);
        s.requests += requests;
        s.input += t.input;
        s.output += t.output;
        s.cache_read += t.cache_read;
        s.cache_write += t.cache_write;
        let rates = price::rates(&t.model).map(|r| if t.agent { r.five_minute() } else { r });
        let cost = match rates {
            Some(r) => price::cost(r, t.input, t.output, t.cache_read, t.cache_write),
            None => {
                s.unpriced_turns += 1;
                0.0
            }
        };
        s.cost += cost;
        s.since_commit += cost;
        let mut committed = false;
        for outcome in &t.outcomes {
            match outcome {
                Outcome::TestPass => s.tests_passed += 1,
                Outcome::TestFail => s.tests_failed += 1,
                Outcome::TestUntold => s.tests_untold += 1,
                Outcome::Commit => {
                    s.commits += 1;
                    committed = true;
                }
                Outcome::Pr => s.prs += 1,
            }
        }
        if committed {
            s.since_commit = 0.0;
        }
        if let (Some(before), Some(after)) = (t.lens_before, t.lens_after) {
            s.lens_turns += 1;
            let saved = before.saturating_sub(after) / BYTES_PER_TOKEN * requests;
            s.lens_tokens_saved += saved;
            if let Some(r) = rates {
                s.lens_cost_saved += saved as f64 * r.cache_read / 1e6;
            }
        }
    }
    if s.commits > 0 {
        s.per_commit = (s.cost - s.since_commit) / s.commits as f64;
    }
    s
}

/// The `meter:` lines for `girth stats`; empty when no turn was recorded.
pub fn render(summary: &Summary) -> String {
    use crate::measure::thousands;
    if summary.turns == 0 {
        return String::new();
    }
    let mut text = format!(
        "meter: {} turns · input {} · output {} · cache read {} · cache write {} · ≈ ${:.2} (estimate)",
        summary.turns,
        thousands(summary.input),
        thousands(summary.output),
        thousands(summary.cache_read),
        thousands(summary.cache_write),
        summary.cost,
    );
    if summary.unpriced_turns > 0 {
        text.push_str(&format!(" ({} turns unpriced)", summary.unpriced_turns));
    }
    if summary.lens_turns > 0 {
        text.push_str(&format!(
            "\nmeter: Lens trimmed the skill listing in {} turns: ≥ {} tokens not sent (≥ ${:.4} at cache-read rates)",
            summary.lens_turns,
            thousands(summary.lens_tokens_saved),
            summary.lens_cost_saved,
        ));
    }
    let mut parts = Vec::new();
    if summary.commits > 0 {
        parts.push(format!(
            "{} commits · ≈ ${:.2} per commit",
            summary.commits, summary.per_commit
        ));
        if summary.since_commit > 0.0 {
            parts.push(format!("≈ ${:.2} since the last commit", summary.since_commit));
        }
    }
    if summary.prs > 0 {
        parts.push(format!("{} PRs", summary.prs));
    }
    let runs: Vec<String> = [
        (summary.tests_passed, "passed"),
        (summary.tests_failed, "failed"),
        (summary.tests_untold, "no verdict"),
    ]
    .iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| format!("{n} {what}"))
    .collect();
    if !runs.is_empty() {
        parts.push(format!("test runs: {}", runs.join(", ")));
    }
    if !parts.is_empty() {
        text.push_str(&format!("\nmeter: outcomes: {}", parts.join(" · ")));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::Turn;

    fn turn(project: &str, model: &str, input: u64, lens: Option<(u64, u64)>) -> Record {
        Record {
            v: 1,
            ts: 1,
            event: Event::Turn(Turn {
                project: project.into(),
                model: model.into(),
                input,
                output: 10,
                cache_read: 1_000,
                cache_write: 100,
                agent: false,
                aborted: false,
                lens_before: lens.map(|l| l.0),
                lens_after: lens.map(|l| l.1),
                requests: None,
                mcp: vec![],
                breaks: vec![],
                outcomes: vec![],
            }),
        }
    }

    #[test]
    fn sums_tokens_prices_known_models_and_counts_lens_savings() {
        let records = vec![
            turn("/a", "claude-opus-5-5", 5, Some((30_000, 2_000))),
            turn("/a", "claude-opus-5-5", 7, Some((30_000, 2_000))),
            turn("/a", "mystery-model", 1, None),
            turn("/b", "claude-sonnet-5-5", 3, None),
            Record {
                v: 1,
                ts: 1,
                event: Event::Seen {
                    names: vec!["x".into()],
                },
            },
        ];
        let all = summarise(&records, None);
        assert_eq!((all.turns, all.input, all.output), (4, 16, 40));
        assert_eq!((all.cache_read, all.cache_write), (4_000, 400));
        assert_eq!(all.unpriced_turns, 1);
        assert_eq!(all.lens_turns, 2);
        assert_eq!(all.lens_tokens_saved, 2 * 28_000 / BYTES_PER_TOKEN);
        let opus = price::rates("claude-opus-5-5").unwrap();
        let sonnet = price::rates("claude-sonnet-5-5").unwrap();
        let want = 2.0 * 0.0
            + price::cost(opus, 5, 10, 1_000, 100)
            + price::cost(opus, 7, 10, 1_000, 100)
            + price::cost(sonnet, 3, 10, 1_000, 100);
        assert!((all.cost - want).abs() < 1e-12);
        let saved = 14_000.0 * opus.cache_read / 1e6;
        assert!(
            (all.lens_cost_saved - saved).abs() < 1e-12,
            "{}",
            all.lens_cost_saved
        );

        let b = summarise(&records, Some("/b"));
        assert_eq!((b.turns, b.lens_turns, b.unpriced_turns), (1, 0, 0));
    }

    #[test]
    fn a_subagent_turn_pays_the_five_minute_write_rate() {
        let mut record = turn("/a", "claude-opus-5-5", 0, None);
        if let Event::Turn(t) = &mut record.event {
            t.agent = true;
            (t.output, t.cache_read, t.cache_write) = (0, 0, 1_000_000);
        }
        let s = summarise(&[record], None);
        assert!((s.cost - 5.0).abs() < 1e-9, "{}", s.cost);
    }

    #[test]
    fn lens_savings_count_every_request_in_a_turn() {
        let mut record = turn("/a", "claude-opus-5-5", 1, Some((30_000, 2_000)));
        if let Event::Turn(t) = &mut record.event {
            t.requests = Some(4);
        }
        let s = summarise(&[record], None);
        assert_eq!(s.lens_tokens_saved, 4 * 28_000 / BYTES_PER_TOKEN);
        assert_eq!(s.requests, 4);
    }

    #[test]
    fn a_rewrite_that_grew_the_listing_saves_nothing() {
        let s = summarise(&[turn("/a", "claude-opus-5-5", 1, Some((100, 300)))], None);
        assert_eq!((s.lens_turns, s.lens_tokens_saved), (1, 0));
    }

    #[test]
    fn render_shows_totals_and_lens_lines_only_when_there_is_data() {
        assert_eq!(render(&Summary::default()), "");
        let s = summarise(
            &[turn("/a", "claude-opus-5-5", 5, Some((30_000, 2_000)))],
            None,
        );
        let text = render(&s);
        assert!(
            text.starts_with(
                "meter: 1 turns · input 5 · output 10 · cache read 1,000 · cache write 100 · ≈ $"
            ),
            "{text}"
        );
        assert!(!text.contains("unpriced"), "{text}");
        assert!(text.contains("\nmeter: Lens trimmed the skill listing in 1 turns: ≥ 7,000 tokens not sent (≥ $0.0014 at cache-read rates)"), "{text}");
        let plain = render(&summarise(&[turn("/a", "mystery", 5, None)], None));
        assert!(
            !plain.contains("Lens") && plain.contains("(1 turns unpriced)"),
            "{plain}"
        );
    }

    fn with(mut record: Record, outcomes: &[Outcome]) -> Record {
        if let Event::Turn(t) = &mut record.event {
            t.outcomes = outcomes.to_vec();
        }
        record
    }

    #[test]
    fn outcomes_are_counted_and_priced_per_commit() {
        use Outcome::*;
        let opus = price::rates("claude-opus-5-5").unwrap();
        let one = price::cost(opus, 1, 10, 1_000, 100);
        let records = vec![
            with(turn("/a", "claude-opus-5-5", 1, None), &[TestFail, TestUntold]),
            with(turn("/a", "claude-opus-5-5", 1, None), &[TestPass, Commit]),
            with(turn("/b", "claude-opus-5-5", 1, None), &[Commit, Pr]),
            with(turn("/a", "claude-opus-5-5", 1, None), &[]),
            with(turn("/a", "claude-opus-5-5", 1, None), &[TestPass]),
        ];
        let a = summarise(&records, Some("/a"));
        assert_eq!((a.commits, a.prs), (1, 0));
        assert_eq!((a.tests_passed, a.tests_failed, a.tests_untold), (2, 1, 1));
        // The two turns after /a's last commit.
        assert!((a.since_commit - 2.0 * one).abs() < 1e-12, "{}", a.since_commit);
        assert!((a.per_commit - 2.0 * one).abs() < 1e-12, "{}", a.per_commit);
        let all = summarise(&records, None);
        assert_eq!((all.commits, all.prs), (2, 1));
        // Everything up to the last commit (turn 3), over two commits.
        assert!((all.per_commit - 1.5 * one).abs() < 1e-12, "{}", all.per_commit);
        assert!((all.since_commit - 2.0 * one).abs() < 1e-12);
    }

    #[test]
    fn without_a_commit_everything_is_since_and_nothing_is_per_commit() {
        let s = summarise(&[with(turn("/a", "claude-opus-5-5", 1, None), &[Outcome::TestPass])], None);
        assert_eq!(s.commits, 0);
        assert_eq!(s.per_commit, 0.0);
        assert!((s.since_commit - s.cost).abs() < 1e-12);
    }

    #[test]
    fn render_adds_an_outcomes_line_with_only_the_parts_it_has() {
        use Outcome::*;
        let plain = render(&summarise(&[turn("/a", "claude-opus-5-5", 5, None)], None));
        assert!(!plain.contains("outcomes"), "{plain}");
        let s = Summary {
            turns: 3,
            commits: 12,
            per_commit: 3.4,
            since_commit: 1.2,
            tests_passed: 20,
            tests_failed: 8,
            tests_untold: 2,
            ..Summary::default()
        };
        assert!(render(&s).ends_with(
            "\nmeter: outcomes: 12 commits · ≈ $3.40 per commit · ≈ $1.20 since the last commit · test runs: 20 passed, 8 failed, 2 no verdict"
        ), "{}", render(&s));
        let tests_only = render(&summarise(&[with(turn("/a", "claude-opus-5-5", 1, None), &[TestFail])], None));
        assert!(tests_only.ends_with("\nmeter: outcomes: test runs: 1 failed"), "{tests_only}");
        let commit_only = render(&summarise(&[with(turn("/a", "claude-opus-5-5", 1, None), &[Commit, Pr])], None));
        assert!(commit_only.contains("\nmeter: outcomes: 1 commits · ≈ $"), "{commit_only}");
        assert!(commit_only.ends_with(" per commit · 1 PRs"), "{commit_only}");
    }
}
