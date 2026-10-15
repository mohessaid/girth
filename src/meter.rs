//! Meter: real token usage per turn, summarised from `turn` ledger events.

use serde::Serialize;

use crate::ledger::{Event, Record};
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
        let rates = price::rates(&t.model);
        match rates {
            Some(r) => s.cost += price::cost(r, t.input, t.output, t.cache_read, t.cache_write),
            None => s.unpriced_turns += 1,
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
        assert!(text.contains("\nmeter: Lens trimmed the skill listing in 1 turns: ≥ 7,000 tokens not sent (≥ $0.0014 at cache-read rates)"), "{text}");
        let plain = render(&summarise(&[turn("/a", "mystery", 5, None)], None));
        assert!(
            !plain.contains("Lens") && plain.contains("(1 turns unpriced)"),
            "{plain}"
        );
    }
}
