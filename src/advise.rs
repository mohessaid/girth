//! `girth advise`: what is costing you, the Girth lever that fixes it, and why the fix keeps
//! things working. Read from the local ledger; costs are list-price estimates, and shares
//! overlap (a turn that ran two skills counts toward both).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::ledger::{Event, Record, Turn};
use crate::price;

/// A contributor needs at least this share of spend to reach `--brief`.
pub const BRIEF_SHARE: f64 = 0.15;
/// Context per request above this many cached tokens counts as a large conversation.
pub const LARGE_CONTEXT: u64 = 150_000;
/// The `--brief` line is shown at session start; keep it short.
pub const BRIEF_MAX_CHARS: usize = 120;
/// Turns taken more than this many seconds after a session's first turn are late.
pub const LONG_SESSION: u64 = 8 * 3600;

/// Levers that are already on, so their advice is suppressed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct On {
    pub keeper: bool,
    pub compact_idle: bool,
    pub agents_model: bool,
    /// The advice is for Codex turns: name Codex's commands.
    pub codex: bool,
    /// The advice is for a harness with `/clear` and `AGENTS.md` (Copilot CLI, OpenCode).
    pub agents_md: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Contributor {
    /// `skill`, `context`, `sessions`, `rewrites` or `subagents`.
    pub kind: &'static str,
    pub label: String,
    /// Share of spend in the window, 0–1.
    pub share: f64,
    pub cost: f64,
    pub fix: String,
    pub guard: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Advice {
    pub window_s: u64,
    pub spend: f64,
    pub contributors: Vec<Contributor>,
    /// Turns on models with no known prices; their tokens are not in `spend`.
    pub unpriced: u64,
    /// Spend per harness (`claude-code` for turns the mod recorded).
    pub harnesses: BTreeMap<String, f64>,
    /// Premium requests on harnesses that bill by request (Copilot); their `spend`
    /// share is what the tokens would cost at list prices.
    pub premium_requests: u64,
}

/// Dollars for one turn: the harness's own figure when it logged one, else list prices
/// (subagents write the 5-minute cache); 0 for unknown models.
fn turn_cost(t: &Turn) -> f64 {
    if let Some(micros) = t.cost_micros {
        return micros as f64 / 1e6;
    }
    price::rates(&t.model)
        .map(|r| if t.agent { r.five_minute() } else { r })
        .map_or(0.0, |r| {
            price::cost(r, t.input, t.output, t.cache_read, t.cache_write)
        })
}

fn expensive(model: &str) -> bool {
    model.contains("opus") || model.contains("fable")
}

/// Contributors over the `window_s` seconds before `now`, largest share first.
pub fn advise(records: &[Record], now: u64, window_s: u64, on: On) -> Advice {
    let start = now.saturating_sub(window_s);
    let in_window = |r: &&Record| r.ts >= start && r.ts <= now;
    let turns: Vec<(u64, &Turn)> = records
        .iter()
        .filter(in_window)
        .filter_map(|r| match &r.event {
            Event::Turn(t) => Some((r.ts, t)),
            _ => None,
        })
        .collect();
    let spend: f64 = turns.iter().map(|(_, t)| turn_cost(t)).sum();
    let unpriced = turns
        .iter()
        .filter(|(_, t)| t.cost_micros.is_none() && price::rates(&t.model).is_none())
        .count() as u64;
    let mut harnesses: BTreeMap<String, f64> = BTreeMap::new();
    for (_, t) in &turns {
        let h = t.harness.as_deref().unwrap_or("claude-code");
        *harnesses.entry(h.to_string()).or_default() += turn_cost(t);
    }
    let premium_requests = turns.iter().filter_map(|(_, t)| t.premium).sum();
    let mut out = Vec::new();
    if spend <= 0.0 {
        return Advice {
            window_s,
            spend,
            contributors: out,
            unpriced,
            harnesses,
            premium_requests,
        };
    }
    let (clear, memory) = if on.codex {
        ("/new", "AGENTS.md")
    } else if on.agents_md {
        ("/clear", "AGENTS.md")
    } else {
        ("/clear", "CLAUDE.md")
    };
    let push =
        |out: &mut Vec<Contributor>, kind, label: String, cost: f64, fix: String, guard: &str| {
            if cost > 0.0 {
                out.push(Contributor {
                    kind,
                    label,
                    share: cost / spend,
                    cost,
                    fix,
                    guard: guard.into(),
                });
            }
        };

    // Skills. A loaded skill stays in context for the rest of its session, so with a session
    // id a main-loop use charges every later main turn there (even a use from before the
    // window), and a subagent's use its next subagent turn. Without one (older ledgers), a
    // use charges the next turn of the same project. Each turn counts once per skill.
    let mut per_skill: BTreeMap<&str, (f64, BTreeSet<usize>)> = Default::default();
    for r in records {
        let Event::Use {
            harness: _,
            skill,
            project,
            session,
            agent,
        } = &r.event
        else {
            continue;
        };
        if skill == "girth" || skill.starts_with("girth:") || skill.starts_with("girth-") {
            continue;
        }
        let later = turns.iter().enumerate().filter(|(_, (ts, _))| *ts >= r.ts);
        let charged: Vec<usize> = match session {
            Some(s) => later
                .filter(|(_, (_, t))| t.agent == *agent && t.session.as_ref() == Some(s))
                .take(if *agent { 1 } else { usize::MAX })
                .map(|(i, _)| i)
                .collect(),
            None if in_window(&r) => later
                .filter(|(_, (_, t))| &t.project == project)
                .take(1)
                .map(|(i, _)| i)
                .collect(),
            None => Vec::new(),
        };
        let entry = per_skill.entry(skill.as_str()).or_default();
        for i in charged {
            if entry.1.insert(i) {
                entry.0 += turn_cost(turns[i].1);
            }
        }
    }
    for (skill, (cost, _)) in per_skill {
        let name = skill.rsplit(':').next().unwrap_or(skill);
        push(
            &mut out,
            "skill",
            skill.to_string(),
            cost,
            format!("ask girth to distil the slice of {name} this project uses"),
            "girth validate checks it, girth drift flags source changes; the source stays installed",
        );
    }

    // Large conversations: every request re-reads the whole context.
    let large: f64 = turns
        .iter()
        .filter(|(_, t)| t.cache_read / t.requests.unwrap_or(1).max(1) > LARGE_CONTEXT)
        .map(|(_, t)| turn_cost(t))
        .sum();
    push(
        &mut out,
        "context",
        "large conversations".into(),
        large,
        format!("/compact <what to keep> at a natural break, or {clear} when you switch tasks"),
        &format!("the summary keeps what you name; {clear} starts clean for unrelated work"),
    );

    // Long sessions: turns taken more than LONG_SESSION after the session's first turn.
    let mut first: BTreeMap<&str, u64> = BTreeMap::new();
    for r in records {
        if let Event::Turn(Turn {
            session: Some(s), ..
        }) = &r.event
        {
            let f = first.entry(s.as_str()).or_insert(r.ts);
            *f = (*f).min(r.ts);
        }
    }
    let late: f64 = turns
        .iter()
        .filter(|(ts, t)| {
            t.session
                .as_deref()
                .and_then(|s| first.get(s))
                .is_some_and(|f| ts.saturating_sub(*f) > LONG_SESSION)
        })
        .map(|(_, t)| turn_cost(t))
        .sum();
    push(
        &mut out,
        "sessions",
        "turns 8h+ into a session".into(),
        late,
        format!(
            "{clear} when you switch tasks: a fresh session re-reads only what the new task needs"
        ),
        &format!(
            "{memory}, project skills and the ledger carry over; /compact <what to keep> if the work goes on"
        ),
    );

    // Cache rewrites after idle gaps.
    let rewrites: f64 = turns
        .iter()
        .filter_map(|(_, t)| {
            let r = price::rates(&t.model)?;
            let tokens: u64 = t.breaks.iter().map(|b| b.rewritten).sum();
            Some(price::cost(r, 0, 0, 0, tokens))
        })
        .sum();
    let fix = if !on.keeper {
        Some("turn the girth keeper on: it keeps the cache warm while you are away")
    } else if !on.compact_idle {
        Some("turn on idle compaction: {\"compact\":{\"idle\":true}} in ~/.girth/config.json")
    } else {
        None
    };
    if let Some(fix) = fix {
        push(
            &mut out,
            "rewrites",
            "cache rewrites after idle".into(),
            rewrites,
            fix.into(),
            "the keeper only reads the cache; compaction is opt-in and keeps what matters",
        );
    }

    // Subagents on an expensive model.
    if !on.agents_model {
        let agents: f64 = turns
            .iter()
            .filter(|(_, t)| t.agent && expensive(&t.model))
            .map(|(_, t)| turn_cost(t))
            .sum();
        push(
            &mut out,
            "subagents",
            "subagents on the main model".into(),
            agents,
            "{\"agents\":{\"model\":\"sonnet\"}} in ~/.girth/config.json".into(),
            "only subagents switch; code edits stay on the main model",
        );
    }

    out.sort_by(|a, b| b.share.total_cmp(&a.share));
    Advice {
        window_s,
        spend,
        contributors: out,
        unpriced,
        harnesses,
        premium_requests,
    }
}

fn window_label(window_s: u64) -> String {
    match window_s {
        s if s >= 86_400 && s % 86_400 == 0 => format!("{}d", s / 86_400),
        s => format!("{}h", s.div_ceil(3600)),
    }
}

/// The full report: spend, then each contributor with its fix and guard.
pub fn render(advice: &Advice) -> String {
    let when = window_label(advice.window_s);
    if advice.contributors.is_empty() {
        return format!(
            "girth advise ({when}): ${:.2} estimated, nothing stands out\n{}",
            advice.spend,
            notes(advice)
        );
    }
    let mut s = format!(
        "girth advise ({when}): ${:.2} estimated at list prices; shares overlap\n",
        advice.spend
    );
    for c in advice.contributors.iter().filter(|c| c.share >= 0.01) {
        s += &format!(
            "{:>4.0}%  {}  (${:.2})\n       fix: {}\n       keeps working: {}\n",
            c.share * 100.0,
            c.label,
            c.cost,
            c.fix,
            c.guard
        );
    }
    let hidden = advice
        .contributors
        .iter()
        .filter(|c| c.share < 0.01)
        .count();
    if hidden > 0 {
        s += &format!("       ({hidden} more under 1%; --json lists them)\n");
    }
    s + &notes(advice)
}

fn notes(advice: &Advice) -> String {
    let mut s = unpriced_note(advice.unpriced);
    if advice.premium_requests > 0 {
        s += &format!(
            "       ({} Copilot premium requests; Copilot's share is at list prices, not what the subscription bills)\n",
            advice.premium_requests
        );
    }
    s
}

fn unpriced_note(unpriced: u64) -> String {
    match unpriced {
        0 => String::new(),
        1 => "       (1 turn on a model without known prices is not counted)\n".into(),
        n => format!("       ({n} turns on models without known prices are not counted)\n"),
    }
}

/// The one line for session start, or nothing when no contributor clears `BRIEF_SHARE`.
pub fn brief(advice: &Advice) -> Option<String> {
    let top = advice
        .contributors
        .first()
        .filter(|c| c.share >= BRIEF_SHARE)?;
    let pct = (top.share * 100.0).round();
    let when = window_label(advice.window_s);
    let line = match top.kind {
        "skill" => format!(
            "girth: {pct}% of your spend ({when}) ran with {}; I can distil its slice.",
            top.label
        ),
        _ => format!(
            "girth: {pct}% of your spend ({when}) is {}. Ask me to cut it.",
            top.label
        ),
    };
    Some(line.chars().take(BRIEF_MAX_CHARS).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::Break;

    const NOW: u64 = 1_000_000;
    const WEEK: u64 = 7 * 86_400;

    fn turn(model: &str, cache_read: u64, output: u64) -> Turn {
        Turn {
            harness: None,
            premium: None,
            cost_micros: None,
            project: "/p".into(),
            session: None,
            model: model.into(),
            input: 0,
            output,
            cache_read,
            cache_write: 0,
            agent: false,
            aborted: false,
            lens_before: None,
            lens_after: None,
            requests: Some(1),
            mcp: vec![],
            breaks: vec![],
            outcomes: vec![],
        }
    }

    fn rec(ts: u64, t: Turn) -> Record {
        Record {
            v: 1,
            ts,
            event: Event::Turn(t),
        }
    }

    fn used(ts: u64, skill: &str) -> Record {
        Record {
            v: 1,
            ts,
            event: Event::Use {
                harness: None,
                skill: skill.into(),
                project: "/p".into(),
                session: None,
                agent: false,
            },
        }
    }

    const SONNET: &str = "claude-sonnet-5-5";

    fn in_session(session: &str, agent: bool) -> Turn {
        Turn {
            session: Some(session.into()),
            agent,
            ..turn(SONNET, 0, 100_000)
        }
    }

    fn used_in(ts: u64, skill: &str, session: &str, agent: bool) -> Record {
        Record {
            v: 1,
            ts,
            event: Event::Use {
                skill: skill.into(),
                project: "/p".into(),
                session: Some(session.into()),
                agent,
                harness: None,
            },
        }
    }

    fn share(a: &Advice, kind: &str) -> f64 {
        a.contributors
            .iter()
            .find(|c| c.kind == kind)
            .map_or(0.0, |c| c.share)
    }

    #[test]
    fn unpriced_turns_are_counted() {
        let records = [
            rec(10, turn("claude-opus-5-5", 1000, 100)),
            rec(20, turn("gpt-9-unknown", 1000, 100)),
            rec(30, turn("gpt-9-unknown", 1000, 100)),
        ];
        let advice = advise(&records, 100, 1000, On::default());
        assert_eq!(advice.unpriced, 2);
        assert!(render(&advice).contains("2 turns on models without known prices are not counted"));
        let one = advise(&records[..2], 100, 1000, On::default());
        assert!(render(&one).contains("1 turn on a model without known prices is not counted"));
        assert!(
            !render(&advise(&records[..1], 100, 1000, On::default()))
                .contains("without known prices")
        );
    }

    #[test]
    fn spend_is_split_by_harness() {
        let mut codex = turn("gpt-6-astra", 0, 1_000_000);
        codex.harness = Some("codex".into());
        let records = [
            rec(10, turn("claude-opus-5-5", 0, 1_000_000)),
            rec(20, codex),
        ];
        let advice = advise(&records, 100, 1000, On::default());
        assert_eq!(advice.harnesses["claude-code"], 20.0);
        assert_eq!(advice.harnesses["codex"], 50.0);
        assert_eq!(advice.spend, 70.0);
    }

    #[test]
    fn copilot_premium_requests_are_reported_beside_list_price() {
        let mut copilot = turn("claude-sonnet-4-6", 0, 1_000_000);
        copilot.harness = Some("copilot".into());
        copilot.premium = Some(12);
        let mut more = copilot.clone();
        more.premium = Some(3);
        let records = [
            rec(10, copilot),
            rec(20, more),
            rec(30, turn("claude-opus-5-5", 0, 1)),
        ];
        let advice = advise(&records, 100, 1000, On::default());
        assert_eq!(advice.premium_requests, 15);
        let text = render(&advice);
        assert!(text.contains("15 Copilot premium requests"), "{text}");
        assert!(
            text.contains("list prices, not what the subscription bills"),
            "{text}"
        );
        let none = advise(
            &[rec(30, turn("claude-opus-5-5", 0, 1))],
            100,
            1000,
            On::default(),
        );
        assert_eq!(none.premium_requests, 0);
        assert!(!render(&none).contains("premium"));
    }

    #[test]
    fn copilot_and_opencode_advice_names_agents_md() {
        let on = On {
            agents_md: true,
            ..On::default()
        };
        let mut early = turn("claude-sonnet-4-6", 400_000, 0);
        early.session = Some("s".into());
        let late = early.clone();
        let records = [rec(10, early), rec(10 + 9 * 3600, late)];
        let advice = advise(&records, 10 + 9 * 3600, 86_400, on);
        let text: String = advice
            .contributors
            .iter()
            .map(|c| format!("{} {}\n", c.fix, c.guard))
            .collect();
        assert!(
            text.contains("/clear") && text.contains("AGENTS.md"),
            "{text}"
        );
        assert!(!text.contains("CLAUDE.md"), "{text}");
    }

    #[test]
    fn a_logged_cost_is_used_and_is_not_unpriced() {
        let mut opencode = turn("deepseek-v4-pro", 0, 1_000_000);
        opencode.harness = Some("opencode".into());
        opencode.cost_micros = Some(2_500_000);
        let mut listed = turn("claude-opus-5-5", 0, 1_000_000);
        listed.cost_micros = Some(1_000_000); // the log's figure wins over list prices
        let advice = advise(
            &[rec(10, opencode), rec(20, listed)],
            100,
            1000,
            On::default(),
        );
        assert_eq!(advice.harnesses["opencode"], 2.5);
        assert_eq!(advice.spend, 3.5);
        assert_eq!(advice.unpriced, 0);
    }

    #[test]
    fn codex_advice_names_codex_commands() {
        let big = turn("gpt-6-astra", 400_000, 0);
        let on = On {
            codex: true,
            ..On::default()
        };
        let advice = advise(&[rec(10, big)], 100, 1000, on);
        let context = advice
            .contributors
            .iter()
            .find(|c| c.kind == "context")
            .unwrap();
        assert!(
            context.fix.contains("/new") && !context.fix.contains("/clear"),
            "{}",
            context.fix
        );
        let claude = advise(
            &[rec(10, turn("claude-opus-5-5", 400_000, 0))],
            100,
            1000,
            On::default(),
        );
        assert!(claude.contributors[0].fix.contains("/clear"));
    }

    #[test]
    fn empty_ledger_has_no_spend_and_no_brief() {
        let a = advise(&[], NOW, WEEK, On::default());
        assert_eq!(a.spend, 0.0);
        assert!(a.contributors.is_empty());
        assert_eq!(brief(&a), None);
    }

    #[test]
    fn a_skill_is_charged_for_the_turn_it_ran_in() {
        // Two equal turns; playwright ran in the first, so it carries half the spend.
        let records = vec![
            used(NOW - 100, "playwright-cli"),
            rec(NOW - 90, turn(SONNET, 0, 100_000)),
            rec(NOW - 50, turn(SONNET, 0, 100_000)),
        ];
        let a = advise(&records, NOW, WEEK, On::default());
        let c = &a.contributors[0];
        assert_eq!((c.kind, c.label.as_str()), ("skill", "playwright-cli"));
        assert!((c.share - 0.5).abs() < 1e-9, "{c:?}");
        assert!(c.fix.contains("girth"), "{c:?}");
        assert!(c.guard.contains("drift"), "{c:?}");
    }

    #[test]
    fn girth_itself_and_turns_outside_the_window_are_not_charged() {
        let records = vec![
            used(NOW - WEEK - 100, "old-skill"),
            rec(NOW - WEEK - 90, turn(SONNET, 0, 100_000)),
            used(NOW - 100, "girth:girth"),
            rec(NOW - 90, turn(SONNET, 0, 100_000)),
        ];
        let a = advise(&records, NOW, WEEK, On::default());
        assert!(a.contributors.iter().all(|c| c.kind != "skill"), "{a:?}");
        assert!(
            (a.spend - 1.0).abs() < 1e-9,
            "only the in-window turn: {}",
            a.spend
        );
    }

    #[test]
    fn large_contexts_point_at_compact_or_clear() {
        let records = vec![
            rec(NOW - 90, turn(SONNET, 400_000, 0)),
            rec(NOW - 50, turn(SONNET, 10_000, 0)),
        ];
        let a = advise(&records, NOW, WEEK, On::default());
        let c = a.contributors.iter().find(|c| c.kind == "context").unwrap();
        assert!(c.share > 0.9, "{c:?}");
        assert!(
            c.fix.contains("/clear") && c.fix.contains("/compact"),
            "{c:?}"
        );
    }

    #[test]
    fn cache_rewrites_suggest_the_keeper_then_idle_compaction_then_nothing() {
        let mut t = turn(SONNET, 0, 0);
        t.cache_write = 500_000;
        t.breaks = vec![Break {
            lost: 500_000,
            rewritten: 500_000,
            idle_s: 7200,
        }];
        let records = vec![rec(NOW - 90, t)];
        let fix = |on| {
            advise(&records, NOW, WEEK, on)
                .contributors
                .into_iter()
                .find(|c| c.kind == "rewrites")
                .map(|c| c.fix)
        };
        assert!(fix(On::default()).unwrap().contains("keeper"));
        let keeper = On {
            keeper: true,
            ..On::default()
        };
        assert!(fix(keeper).unwrap().contains("idle"));
        let both = On {
            keeper: true,
            compact_idle: true,
            ..On::default()
        };
        assert_eq!(fix(both), None);
    }

    #[test]
    fn subagents_on_an_expensive_model_suggest_a_cheaper_one_unless_set() {
        let mut t = turn("claude-opus-5-5", 0, 100_000);
        t.agent = true;
        let records = vec![
            rec(NOW - 90, t),
            rec(NOW - 50, turn("claude-opus-5-5", 0, 100_000)),
        ];
        let a = advise(&records, NOW, WEEK, On::default());
        let c = a
            .contributors
            .iter()
            .find(|c| c.kind == "subagents")
            .unwrap();
        assert!(c.fix.contains("sonnet"), "{c:?}");
        let set = On {
            agents_model: true,
            ..On::default()
        };
        let a = advise(&records, NOW, WEEK, set);
        assert!(a.contributors.iter().all(|c| c.kind != "subagents"));
    }

    #[test]
    fn contributors_are_sorted_and_brief_names_the_top_one_in_one_short_line() {
        let records = vec![
            used(NOW - 100, "playwright-cli"),
            rec(NOW - 90, turn(SONNET, 0, 300_000)),
            used(NOW - 60, "tiny"),
            rec(NOW - 50, turn(SONNET, 0, 10_000)),
        ];
        let a = advise(&records, NOW, WEEK, On::default());
        assert!(a.contributors.windows(2).all(|w| w[0].share >= w[1].share));
        let line = brief(&a).unwrap();
        assert!(line.starts_with("girth: "), "{line}");
        assert!(
            line.contains("playwright-cli") && line.contains("97%"),
            "{line}"
        );
        assert!(line.chars().count() <= BRIEF_MAX_CHARS, "{line}");
    }

    #[test]
    fn render_hides_contributors_under_one_percent() {
        let records = vec![
            used(NOW - 100, "big"),
            rec(NOW - 90, turn(SONNET, 0, 1_000_000)),
            used(NOW - 60, "crumb"),
            rec(NOW - 50, turn(SONNET, 0, 1_000)),
        ];
        let text = render(&advise(&records, NOW, WEEK, On::default()));
        assert!(text.contains("big") && !text.contains("crumb"), "{text}");
        assert!(text.contains("1 more under 1%"), "{text}");
    }

    #[test]
    fn only_expensive_subagents_count_and_main_turns_never_do() {
        let mut cheap = turn(SONNET, 0, 100_000);
        cheap.agent = true;
        let main_opus = turn("claude-opus-5-5", 0, 100_000);
        let a = advise(
            &[rec(NOW - 90, cheap), rec(NOW - 80, main_opus)],
            NOW,
            WEEK,
            On::default(),
        );
        assert!(
            a.contributors.iter().all(|c| c.kind != "subagents"),
            "{a:?}"
        );
    }

    #[test]
    fn large_context_is_per_request_and_150k_is_not_large() {
        let mut spread = turn(SONNET, 400_000, 0);
        spread.requests = Some(4); // 100K per request
        let edge = turn(SONNET, LARGE_CONTEXT, 0);
        let a = advise(
            &[rec(NOW - 90, spread), rec(NOW - 80, edge)],
            NOW,
            WEEK,
            On::default(),
        );
        assert!(a.contributors.iter().all(|c| c.kind != "context"), "{a:?}");
    }

    #[test]
    fn window_labels_use_days_only_for_whole_days() {
        assert_eq!(window_label(7 * 86_400), "7d");
        assert_eq!(window_label(86_400), "1d");
        assert_eq!(window_label(36 * 3600), "36h");
        assert_eq!(window_label(3600), "1h");
        assert_eq!(window_label(100), "1h");
    }

    #[test]
    fn exactly_one_percent_is_shown_and_no_tail_note_without_a_tail() {
        let records = vec![
            used(NOW - 100, "big"),
            rec(NOW - 90, turn(SONNET, 0, 990_000)),
            used(NOW - 60, "edge"),
            rec(NOW - 50, turn(SONNET, 0, 10_000)),
        ];
        let text = render(&advise(&records, NOW, WEEK, On::default()));
        assert!(text.contains("edge"), "{text}");
        assert!(!text.contains("more under 1%"), "{text}");
    }

    #[test]
    fn brief_is_silent_below_the_threshold() {
        let mut records = vec![
            used(NOW - 100, "rare"),
            rec(NOW - 90, turn(SONNET, 0, 10_000)),
        ];
        records.push(rec(NOW - 50, turn(SONNET, 0, 1_000_000)));
        let a = advise(&records, NOW, WEEK, On::default());
        assert_eq!(brief(&a), None, "{a:?}");
    }

    #[test]
    fn a_skill_stays_charged_for_the_rest_of_its_session_once() {
        // Used twice in s1, before and inside the window: every later s1 turn counts once.
        let records = vec![
            rec(NOW - WEEK - 200, in_session("s1", false)),
            used_in(NOW - WEEK - 100, "playwright-cli", "s1", false),
            rec(NOW - 90, in_session("s1", false)),
            used_in(NOW - 80, "playwright-cli", "s1", false),
            rec(NOW - 70, in_session("s1", false)),
            rec(NOW - 60, in_session("s2", false)),
            rec(NOW - 50, in_session("s1", true)),
        ];
        let a = advise(&records, NOW, WEEK, On::default());
        assert!((share(&a, "skill") - 0.5).abs() < 1e-9, "{a:?}");
    }

    #[test]
    fn a_subagent_skill_is_charged_for_its_next_subagent_turn_only() {
        let records = vec![
            used_in(NOW - 100, "dataviz", "s1", true),
            rec(NOW - 90, in_session("s1", false)),
            rec(
                NOW - 80,
                Turn {
                    output: 300_000,
                    ..in_session("s1", true)
                },
            ),
            rec(NOW - 70, in_session("s1", true)),
            rec(NOW - 60, in_session("s1", false)),
        ];
        // 3 of 6: the subagent turn after the use, not the main turn before it.
        let a = advise(&records, NOW, WEEK, On::default());
        assert!((share(&a, "skill") - 0.5).abs() < 1e-9, "{a:?}");
    }

    #[test]
    fn turns_more_than_8h_into_a_session_point_at_clear() {
        let h8 = 8 * 3600;
        let records = vec![
            rec(NOW - WEEK - 10, in_session("old", false)),
            rec(NOW - 90, in_session("old", false)),
            rec(NOW - 80, in_session("old", false)),
            rec(NOW - h8 - 100, in_session("s", false)),
            rec(NOW - 100, in_session("s", false)),
            rec(NOW - 99, in_session("s", false)),
            rec(NOW - 50, in_session("t", false)),
        ];
        let a = advise(&records, NOW, WEEK, On::default());
        // Late: both "old" turns and the one 8h+1s into "s"; exactly 8h is not late.
        assert!((share(&a, "sessions") - 0.5).abs() < 1e-9, "{a:?}");
        let c = a
            .contributors
            .iter()
            .find(|c| c.kind == "sessions")
            .unwrap();
        assert!(c.fix.contains("/clear"), "{c:?}");
        // Turns without a session id are never part of a long session.
        let legacy = vec![
            rec(NOW - WEEK + 10, turn(SONNET, 0, 100_000)),
            rec(NOW - 10, turn(SONNET, 0, 100_000)),
        ];
        assert_eq!(
            share(&advise(&legacy, NOW, WEEK, On::default()), "sessions"),
            0.0
        );
    }
}
