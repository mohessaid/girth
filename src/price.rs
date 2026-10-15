//! Per-model API prices, for cost estimates from recorded token counts.
//!
//! Source: the claude-api skill bundled with Claude Code 2.1.300 (pricing cached
//! 2026-10-06). Cache writes are priced at the 1-hour rate (2× input), the
//! cache Claude Code writes; this reproduces the cost Claude Code reports.

/// US dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

/// Rates for a model id. Matches the exact id, or the id followed by a
/// bracketed suffix (`claude-opus-5-5[1m]`).
pub fn rates(model: &str) -> Option<Rates> {
    const TABLE: [(&str, Rates); 4] = [
        (
            "claude-opus-5-5",
            Rates {
                input: 4.0,
                output: 20.0,
                cache_read: 0.20,
                cache_write: 8.0,
            },
        ),
        (
            "claude-sonnet-5-5",
            Rates {
                input: 2.0,
                output: 10.0,
                cache_read: 0.20,
                cache_write: 4.0,
            },
        ),
        (
            "claude-haiku-5-5",
            Rates {
                input: 0.10,
                output: 0.50,
                cache_read: 0.01,
                cache_write: 0.20,
            },
        ),
        (
            "claude-fable-5-1",
            Rates {
                input: 10.0,
                output: 50.0,
                cache_read: 0.25,
                cache_write: 20.0,
            },
        ),
    ];
    let id = model.split('[').next().unwrap_or(model);
    TABLE.iter().find(|(name, _)| *name == id).map(|(_, r)| *r)
}

/// Estimated dollars for one usage record.
pub fn cost(rates: Rates, input: u64, output: u64, cache_read: u64, cache_write: u64) -> f64 {
    (input as f64 * rates.input
        + output as f64 * rates.output
        + cache_read as f64 * rates.cache_read
        + cache_write as f64 * rates.cache_write)
        / 1e6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_models_have_rates_and_unknown_ones_do_not() {
        let opus = rates("claude-opus-5-5").unwrap();
        assert_eq!(
            (opus.input, opus.output, opus.cache_read, opus.cache_write),
            (4.0, 20.0, 0.20, 8.0)
        );
        assert_eq!(rates("claude-opus-5-5[1m]"), Some(opus));
        assert_eq!(rates("claude-sonnet-5-5").unwrap().cache_read, 0.20);
        assert_eq!(rates("claude-haiku-5-5").unwrap().input, 0.10);
        assert_eq!(rates("claude-fable-5-1").unwrap().cache_read, 0.25);
        assert_eq!(rates("gpt-6"), None);
        assert_eq!(rates(""), None);
        // A longer id must not be matched by a shorter one's prefix.
        assert_eq!(rates("claude-opus-5"), None);
    }

    #[test]
    fn cost_sums_each_kind_at_its_rate() {
        let r = rates("claude-opus-5-5").unwrap();
        // The probe's "say hi" request: 2 in, 7 out, 8,605 read, 26,631 written.
        let c = cost(r, 2, 7, 8_605, 26_631);
        let want = (2.0 * 4.0 + 7.0 * 20.0 + 8_605.0 * 0.20 + 26_631.0 * 8.0) / 1e6;
        assert!((c - want).abs() < 1e-12, "{c} vs {want}");
        assert_eq!(cost(r, 0, 0, 0, 0), 0.0);
    }

    #[test]
    fn matches_the_cost_claude_code_reports_for_one_hour_cache_writes() {
        // A logged Haiku 5.5 run (examples/cache/probe): Claude Code reported
        // costUSD 0.01462233 for 2 in, 4 out, 11,093 read, 72,546 written (1-hour).
        let c = cost(rates("claude-haiku-5-5").unwrap(), 2, 4, 11_093, 72_546);
        assert!((c - 0.014_622_33).abs() < 1e-8, "{c}");
        // Every model writes at twice its input rate.
        for id in [
            "claude-opus-5-5",
            "claude-sonnet-5-5",
            "claude-haiku-5-5",
            "claude-fable-5-1",
        ] {
            let r = rates(id).unwrap();
            assert_eq!(r.cache_write, 2.0 * r.input, "{id}");
        }
    }
}
