//! Per-model API prices, for cost estimates from recorded token counts. A vendor's dated
//! change goes in `PERIODS`, so each turn is priced as it was billed (`rates_at`).
//!
//! Sources: the claude-api skill bundled with Claude Code 2.1.300 (pricing cached
//! 2026-10-06) for Claude models (earlier models from its legacy table, with cache reads at
//! 0.1× and 1-hour writes at 2× input), rechecked against docs.claude.com pricing on
//! 2026-11-08 (Sonnet 5.5 cache hits fell to $0.10 on 2026-10-08). Claude Haiku 5.5 bills a
//! request whose prompt is over 100,000 tokens at higher prices; a turn's tokens are
//! summed over its requests, so the table prices every Haiku 5.5 turn at the lower tier
//! (main-loop turns the mod could price from Claude Code's own records skip the table); developers.openai.com/api/docs/pricing (standard
//! tier, short context, read 2026-11-06) for the GPT-6 models Codex runs, with
//! cache writes at the 30-minute rate; aistudio.google.com/docs/pricing (standard tier,
//! prompts up to 200K tokens, text input, read 2026-11-06; 3.6 and 3.8 Flash read
//! 2026-11-08) for the Gemini models Gemini CLI and Antigravity CLI run. `gemini-3.1-flash-lite-preview` takes the `gemini-3.1-flash-lite` row, and
//! `gemini-3-pro-preview`, which the page no longer lists, is left unpriced. Cache writes are priced at the 1-hour rate (2× input), the
//! cache the main conversation writes; this reproduces the cost Claude Code
//! reports. Claude subagents write the 5-minute cache (1.25× input): see `turn_rates`.
//! Fast mode (2× on Fable 5.2 and Opus 5.5) is not in the table: turns the mod prices from
//! Claude Code's own records carry it; parallel subagents and imported turns don't.

/// US dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

/// A model's rates while `from <= ts < until` (unix seconds), over its `rates` row: a
/// vendor's dated price change, before or after it happens.
const PERIODS: [(&str, u64, u64, Rates); 6] = [
    // Anthropic halved Sonnet 5.5's cache hits to $0.10 on 2026-10-08.
    (
        "claude-sonnet-5-5",
        0,
        1_791_417_600,
        Rates {
            input: 2.0,
            output: 10.0,
            cache_read: 0.20,
            cache_write: 4.0,
        },
    ),
    // Google doubles Gemini 3.6 and 3.8 Flash on 2027-01-01.
    (
        "gemini-3.8-flash",
        1_798_761_600,
        u64::MAX,
        google(1.50, 0.15, 7.50),
    ),
    (
        "gemini-3.8-flash-n",
        1_798_761_600,
        u64::MAX,
        google(1.50, 0.15, 7.50),
    ),
    (
        "gemini-3.6-flash",
        1_798_761_600,
        u64::MAX,
        google(1.50, 0.15, 7.50),
    ),
    // And Gemini 4 Carbon and Argon on 2026-12-01.
    (
        "gemini-4-carbon",
        1_796_083_200,
        u64::MAX,
        google(4.0, 0.20, 20.0),
    ),
    (
        "gemini-4-argon",
        1_796_083_200,
        u64::MAX,
        google(4.0, 0.20, 20.0),
    ),
];

/// Today's rates for a model id: see `rates_at`.
pub fn rates(model: &str) -> Option<Rates> {
    rates_at(model, crate::ledger::now())
}

/// Rates for a model id at `ts` (unix seconds), so a turn is priced as it was billed.
/// Matches the exact id, or the id followed by a bracketed suffix (`claude-opus-5-5[1m]`).
pub fn rates_at(model: &str, ts: u64) -> Option<Rates> {
    let id = model.split('[').next().unwrap_or(model);
    PERIODS
        .iter()
        .find(|(name, from, until, _)| *name == id && *from <= ts && ts < *until)
        .map(|p| p.3)
        .or_else(|| listed(id))
}

/// The table's rates for an id with no bracketed suffix.
fn listed(id: &str) -> Option<Rates> {
    const TABLE: [(&str, Rates); 27] = [
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
                cache_read: 0.10,
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
        // Fable 5.2 and Mythos 5.2 (2026-11-09; docs.claude.com pricing read 2026-11-13).
        (
            "claude-fable-5-2",
            Rates {
                input: 8.0,
                output: 40.0,
                cache_read: 0.20,
                cache_write: 16.0,
            },
        ),
        (
            "claude-mythos-5-2",
            Rates {
                input: 8.0,
                output: 40.0,
                cache_read: 0.20,
                cache_write: 16.0,
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
        // Until 2026-12-31; Google doubles all three on 2027-01-01. Antigravity CLI logs
        // 3.8 Flash as `gemini-3.8-flash-n`.
        ("gemini-3.8-flash", google(0.75, 0.075, 3.75)),
        ("gemini-3.8-flash-n", google(0.75, 0.075, 3.75)),
        ("gemini-3.6-flash", google(0.75, 0.075, 3.75)),
        // Gemini 4 (ai.google.dev pricing read 2026-11-13): Carbon and Argon through
        // 2026-11-30 (see PERIODS for December on), Hydrogen.
        ("gemini-4-carbon", google(2.0, 0.10, 10.0)),
        ("gemini-4-argon", google(2.0, 0.10, 10.0)),
        ("gemini-4-hydrogen", google(1.50, 0.075, 7.50)),
        ("gpt-6-astra", openai(10.0, 1.0, 12.5, 50.0)),
        ("gpt-6.1-sol", openai(2.0, 0.10, 2.5, 10.0)),
        ("gpt-6-sol", openai(2.0, 0.20, 2.5, 10.0)),
        ("gpt-6.1-terra", openai(1.0, 0.05, 1.25, 5.0)),
        ("gpt-6-luna", openai(0.10, 0.01, 0.125, 0.50)),
    ];
    TABLE.iter().find(|(name, _)| *name == id).map(|(_, r)| *r)
}

/// Rates for one turn of `model`: a Claude subagent's turn writes Anthropic's
/// 5-minute cache (see `Rates::five_minute`). Other vendors charge a subagent's
/// cache writes the same as the main loop's.
pub fn turn_rates(model: &str, agent: bool) -> Option<Rates> {
    turn_rates_at(model, agent, crate::ledger::now())
}

/// `turn_rates` at `ts` (unix seconds): the rates of the turn's own day.
pub fn turn_rates_at(model: &str, agent: bool, ts: u64) -> Option<Rates> {
    let r = rates_at(model, ts)?;
    Some(if agent && model.starts_with("claude-") {
        r.five_minute()
    } else {
        r
    })
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

/// Models billed by prompt length: over `threshold` input tokens per request (cache reads
/// and writes included), the whole request costs `factor` times every rate. Each request
/// is priced on its own (platform.claude.com pricing, "Long context pricing", 2026-11-13).
const LONG_PROMPT: [(&str, u64, f64); 1] = [("claude-haiku-5-5", 100_000, 5.0)];

/// `rates` for a request whose prompt (input, cache reads and cache writes) is `prompt`
/// tokens: the long-prompt price when `model` has one and the prompt is over its line.
pub fn long_prompt(rates: Rates, model: &str, prompt: u64) -> Rates {
    let id = model.split('[').next().unwrap_or(model);
    match LONG_PROMPT.iter().find(|(m, _, _)| *m == id) {
        Some((_, threshold, f)) if prompt > *threshold => Rates {
            input: rates.input * f,
            output: rates.output * f,
            cache_read: rates.cache_read * f,
            cache_write: rates.cache_write * f,
        },
        _ => rates,
    }
}

/// A recorded turn's rates on its own day, at its long-prompt price when it has one.
pub fn rates_for_turn(t: &crate::ledger::Turn, ts: u64) -> Option<Rates> {
    turn_rates_at(&t.model, t.agent, ts).map(|r| long_prompt(r, &t.model, t.prompt()))
}

/// The average prompt of `requests` requests (at least one) that together read `input`,
/// `cache_read` and `cache_write` tokens. Usage is kept per turn, not per request.
pub fn prompt_per_request(requests: u64, input: u64, cache_read: u64, cache_write: u64) -> u64 {
    (input + cache_read + cache_write) / requests.max(1)
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
    fn haiku_5_5_prompts_over_100k_tokens_pay_five_times_every_rate() {
        let base = rates("claude-haiku-5-5").unwrap();
        // The page's rows: $0.50 / $0.625 (5 min) / $1 (1 h) / $0.05 / $2.50.
        let long = long_prompt(base, "claude-haiku-5-5", 100_001);
        assert_eq!((long.input, long.output), (0.5, 2.5));
        assert!((long.cache_read - 0.05).abs() < 1e-12);
        assert!((long.cache_write - 1.0).abs() < 1e-12);
        assert!((long.five_minute().cache_write - 0.625).abs() < 1e-12);
        // At the line, and for other models, nothing changes.
        assert_eq!(long_prompt(base, "claude-haiku-5-5", 100_000), base);
        assert_eq!(long_prompt(base, "claude-haiku-5-5[1m]", 100_001), long);
        let opus = rates("claude-opus-5-5").unwrap();
        assert_eq!(long_prompt(opus, "claude-opus-5-5", 900_000), opus);
        // The probe's request: 148,715 tokens written at $1.00 per million.
        let c = cost(
            long_prompt(base, "claude-haiku-5-5", 148_717),
            2,
            0,
            0,
            148_715,
        );
        assert!((c - 0.148716).abs() < 1e-6, "{c}");
    }

    #[test]
    fn the_prompt_per_request_is_the_average_input_with_cache() {
        assert_eq!(prompt_per_request(4, 2, 300_000, 100_002), 100_001);
        assert_eq!(prompt_per_request(0, 1, 2, 3), 6);
    }

    #[test]
    fn a_turn_is_priced_at_the_rates_of_its_own_day() {
        const OCT_8: u64 = 1_791_417_600; // 2026-10-08T00:00:00Z
        const JAN_1: u64 = 1_798_761_600; // 2027-01-01T00:00:00Z
        // Anthropic halved Sonnet 5.5's cache reads on 2026-10-08.
        let before = rates_at("claude-sonnet-5-5", OCT_8 - 1).unwrap();
        let after = rates_at("claude-sonnet-5-5[1m]", OCT_8).unwrap();
        assert_eq!((before.cache_read, after.cache_read), (0.20, 0.10));
        assert_eq!(
            (before.input, before.output, before.cache_write),
            (2.0, 10.0, 4.0)
        );
        // Google doubles Gemini 3.6 and 3.8 Flash on 2027-01-01.
        for id in ["gemini-3.8-flash", "gemini-3.8-flash-n", "gemini-3.6-flash"] {
            let now = rates_at(id, JAN_1 - 1).unwrap();
            let later = rates_at(id, JAN_1).unwrap();
            assert_eq!(
                (now.input, now.output, now.cache_read),
                (0.75, 3.75, 0.075),
                "{id}"
            );
            assert_eq!(
                (later.input, later.output, later.cache_read),
                (1.50, 7.50, 0.15),
                "{id}"
            );
            assert_eq!(later.cache_write, later.input, "{id}");
        }
        // Google doubles Gemini 4 Carbon and Argon on 2026-12-01; Hydrogen has one price.
        const DEC_1: u64 = 1_796_083_200;
        for id in ["gemini-4-carbon", "gemini-4-argon"] {
            let now = rates_at(id, DEC_1 - 1).unwrap();
            let later = rates_at(id, DEC_1).unwrap();
            assert_eq!(
                (now.input, now.output, now.cache_read, now.cache_write),
                (2.0, 10.0, 0.10, 2.0),
                "{id}"
            );
            assert_eq!(
                (
                    later.input,
                    later.output,
                    later.cache_read,
                    later.cache_write
                ),
                (4.0, 20.0, 0.20, 4.0),
                "{id}"
            );
        }
        let h = rates_at("gemini-4-hydrogen", DEC_1).unwrap();
        assert_eq!((h.input, h.output, h.cache_read), (1.50, 7.50, 0.075));
        // Models without a dated change keep one price; unknown ones have none.
        assert_eq!(
            rates_at("claude-opus-5-5", 0),
            rates_at("claude-opus-5-5", JAN_1)
        );
        assert_eq!(rates_at("gpt-6", OCT_8), None);
        // `rates` is today's price; a subagent turn still writes the 5-minute cache.
        assert_eq!(
            rates("claude-sonnet-5-5"),
            rates_at("claude-sonnet-5-5", crate::ledger::now())
        );
        assert_eq!(
            turn_rates_at("claude-sonnet-5-5", true, OCT_8 - 1),
            Some(before.five_minute())
        );
        assert_eq!(
            turn_rates_at("gemini-3.6-flash", true, JAN_1),
            rates_at("gemini-3.6-flash", JAN_1)
        );
    }

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
    fn only_claude_subagents_pay_the_five_minute_write_rate() {
        let opus = rates("claude-opus-5-5").unwrap();
        assert_eq!(turn_rates("claude-opus-5-5", false), Some(opus));
        assert_eq!(
            turn_rates("claude-opus-5-5[1m]", true),
            Some(opus.five_minute())
        );
        // Other vendors' writes cost the same in a subagent.
        for id in ["gpt-5.4", "deepseek-v4-pro", "gemini-2.5-pro"] {
            assert_eq!(turn_rates(id, true), rates(id), "{id}");
        }
        assert_eq!(turn_rates("unknown", true), None);
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
            // Antigravity CLI's models; `-n` is how agy 1.5 logs 3.8 Flash.
            ("gemini-3.8-flash", (0.75, 3.75, 0.075)),
            ("gemini-3.8-flash-n", (0.75, 3.75, 0.075)),
            ("gemini-3.6-flash", (0.75, 3.75, 0.075)),
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
        // Anthropic cut Sonnet 5.5's cache hits to 5% of input (2026-10-08).
        let sonnet = rates("claude-sonnet-5-5").unwrap();
        assert_eq!(
            (
                sonnet.input,
                sonnet.output,
                sonnet.cache_read,
                sonnet.cache_write
            ),
            (2.0, 10.0, 0.10, 4.0)
        );
        assert_eq!(sonnet.five_minute().cache_write, 2.5);
        assert_eq!(rates("claude-haiku-5-5").unwrap().input, 0.10);
        assert_eq!(rates("claude-fable-5-1").unwrap().cache_read, 0.25);
        // Fable 5.2 and Mythos 5.2 (2026-11-09): $8/$40, 1-hour writes $16, hits $0.20.
        for id in [
            "claude-fable-5-2",
            "claude-fable-5-2[1m]",
            "claude-mythos-5-2",
        ] {
            let r = rates(id).unwrap_or_else(|| panic!("{id}"));
            assert_eq!(
                (r.input, r.output, r.cache_read, r.cache_write),
                (8.0, 40.0, 0.20, 16.0),
                "{id}"
            );
            assert_eq!(r.five_minute().cache_write, 10.0, "{id}");
        }
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
