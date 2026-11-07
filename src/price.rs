//! Per-model API prices, for cost estimates from recorded token counts.
//!
//! Sources: the claude-api skill bundled with Claude Code 2.1.300 (pricing cached
//! 2026-10-06) for Claude models (earlier models from its legacy table, with cache reads at
//! 0.1× and 1-hour writes at 2× input); developers.openai.com/api/docs/pricing (standard
//! tier, short context, read 2026-11-06) for the GPT-6 models Codex runs, with
//! cache writes at the 30-minute rate; aistudio.google.com/docs/pricing (standard tier,
//! prompts up to 200K tokens, text input, read 2026-11-06) for the Gemini models Gemini
//! CLI runs. `gemini-3.1-flash-lite-preview` takes the `gemini-3.1-flash-lite` row, and
//! `gemini-3-pro-preview`, which the page no longer lists, is left unpriced. Cache writes are priced at the 1-hour rate (2× input), the
//! cache the main conversation writes; this reproduces the cost Claude Code
//! reports. Subagents write the 5-minute cache (1.25× input): see `five_minute`.

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
    const TABLE: [(&str, Rates); 19] = [
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
        // Earlier models GitHub Copilot CLI sessions ran (Copilot logs them as `claude-sonnet-4.6`).
        ("claude-sonnet-4-6", anthropic(3.0, 0.30, 15.0)),
        ("claude-opus-4-6", anthropic(5.0, 0.50, 25.0)),
        ("claude-haiku-4-5", anthropic(1.0, 0.10, 5.0)),
        // Other models Copilot sessions ran. OpenAI lists no GPT-5 cache-write price.
        ("gpt-5.4", openai(2.50, 0.25, 2.50, 15.0)),
        ("gpt-5.3-codex", openai(1.75, 0.175, 1.75, 14.0)),
        // DeepSeek bills by the hour: these are its off-peak rates, which cover weekends
        // and most weekday hours. Peak hours (weekdays 01:00–04:00 and 06:00–10:00 UTC)
        // bill twice these, so a figure here may be low.
        ("deepseek-v4-pro", openai(0.66, 0.022, 0.66, 1.98)),
        // Gemini CLI's models (standard tier, prompts up to 200K tokens, text input).
        ("gemini-3-flash-preview", google(0.50, 0.05, 3.0)),
        ("gemini-3.1-flash-lite-preview", google(0.25, 0.025, 1.50)),
        ("gemini-2.5-pro", google(1.25, 0.125, 10.0)),
        ("gemini-2.5-flash", google(0.30, 0.03, 2.50)),
        ("gpt-6-astra", openai(10.0, 1.0, 12.5, 50.0)),
        ("gpt-6.1-sol", openai(2.0, 0.10, 2.5, 10.0)),
        ("gpt-6-sol", openai(2.0, 0.20, 2.5, 10.0)),
        ("gpt-6.1-terra", openai(1.0, 0.05, 1.25, 5.0)),
        ("gpt-6-luna", openai(0.10, 0.01, 0.125, 0.50)),
    ];
    let id = model.split('[').next().unwrap_or(model);
    TABLE.iter().find(|(name, _)| *name == id).map(|(_, r)| *r)
}

/// Anthropic's earlier models: input, cache read (0.1× input), output; 1-hour writes at 2× input.
const fn anthropic(input: f64, cache_read: f64, output: f64) -> Rates {
    Rates {
        input,
        output,
        cache_read,
        cache_write: input * 2.0,
    }
}

/// Google's table order: input, context caching, output. Implicit cache writes cost
/// nothing extra, so a write is priced as plain input.
const fn google(input: f64, cache_read: f64, output: f64) -> Rates {
    Rates {
        input,
        output,
        cache_read,
        cache_write: input,
    }
}

/// OpenAI's table order: input, cached input, cache write, output.
const fn openai(input: f64, cache_read: f64, cache_write: f64, output: f64) -> Rates {
    Rates {
        input,
        output,
        cache_read,
        cache_write,
    }
}

impl Rates {
    /// The same rates with cache writes at the 5-minute price (1.25× input),
    /// the cache a subagent's requests write.
    pub fn five_minute(self) -> Rates {
        Rates {
            cache_write: self.input * 1.25,
            ..self
        }
    }
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
    fn openai_models_are_priced() {
        assert_eq!(
            rates("gpt-6-astra"),
            Some(Rates {
                input: 10.0,
                output: 50.0,
                cache_read: 1.0,
                cache_write: 12.5,
            })
        );
        assert_eq!(rates("gpt-6.1-terra").unwrap().cache_read, 0.05);
        assert_eq!(rates("gpt-6-sol").unwrap().cache_read, 0.20);
        assert_eq!(rates("gpt-6-luna").unwrap().cache_write, 0.125);
        assert_eq!(rates("gpt-6.1-sol").unwrap().output, 10.0);
    }

    #[test]
    fn other_models_copilot_runs_are_priced() {
        let rows = [
            // OpenAI lists no cache-write price for GPT-5, so a write is plain input.
            ("gpt-5.4", (2.50, 15.0, 0.25, 2.50)),
            ("gpt-5.3-codex", (1.75, 14.0, 0.175, 1.75)),
            // DeepSeek's off-peak rates; peak hours bill twice these.
            ("deepseek-v4-pro", (0.66, 1.98, 0.022, 0.66)),
        ];
        for (id, want) in rows {
            let r = rates(id).unwrap();
            assert_eq!(
                (r.input, r.output, r.cache_read, r.cache_write),
                want,
                "{id}"
            );
        }
    }

    #[test]
    fn earlier_claude_models_copilot_runs_are_priced() {
        let rows = [
            ("claude-sonnet-4-6", (3.0, 15.0, 0.30, 6.0)),
            ("claude-opus-4-6", (5.0, 25.0, 0.50, 10.0)),
            ("claude-haiku-4-5", (1.0, 5.0, 0.10, 2.0)),
        ];
        for (id, want) in rows {
            let r = rates(id).unwrap();
            assert_eq!(
                (r.input, r.output, r.cache_read, r.cache_write),
                want,
                "{id}"
            );
        }
    }

    #[test]
    fn gemini_models_gemini_cli_runs_are_priced() {
        let rows = [
            ("gemini-3-flash-preview", (0.50, 3.0, 0.05)),
            ("gemini-3.1-flash-lite-preview", (0.25, 1.50, 0.025)),
            ("gemini-2.5-pro", (1.25, 10.0, 0.125)),
            ("gemini-2.5-flash", (0.30, 2.50, 0.03)),
        ];
        for (id, want) in rows {
            let r = rates(id).unwrap();
            assert_eq!((r.input, r.output, r.cache_read), want, "{id}");
            // Gemini charges nothing extra to write its implicit cache.
            assert_eq!(r.cache_write, r.input, "{id}");
        }
        // Not on Google's page (2026-11-06), so not guessed.
        assert_eq!(rates("gemini-3-pro-preview"), None);
    }

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
