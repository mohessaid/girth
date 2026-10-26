//! `girth advise`: what is costing you, the Girth lever that fixes it, and why the fix keeps
//! things working. Read from the local ledger; costs are list-price estimates, and shares
//! overlap (a turn that ran two skills counts toward both).

use serde::Serialize;

use crate::ledger::{Event, Record, Turn};
use crate::price;

/// A contributor needs at least this share of spend to reach `--brief`.
pub const BRIEF_SHARE: f64 = 0.15;
/// Context per request above this many cached tokens counts as a large conversation.
pub const LARGE_CONTEXT: u64 = 150_000;
/// The `--brief` line is shown at session start; keep it short.
pub const BRIEF_MAX_CHARS: usize = 120;

/// Levers that are already on, so their advice is suppressed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct On {
    pub keeper: bool,
    pub compact_idle: bool,
    pub agents_model: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Contributor {
    /// `skill`, `context`, `rewrites` or `subagents`.
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
}

/// Estimated dollars for one turn (subagents write the 5-minute cache); 0 for unknown models.
fn turn_cost(t: &Turn) -> f64 {
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
    let mut out = Vec::new();
    if spend <= 0.0 {
        return Advice {
            window_s,
            spend,
            contributors: out,
        };
    }
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

    // Skills: a `use` event charges the next turn of the same project (the turn it ran in).
    let mut per_skill: std::collections::BTreeMap<&str, (f64, std::collections::BTreeSet<u64>)> =
        Default::default();
    for r in records.iter().filter(in_window) {
        let Event::Use { skill, project } = &r.event else {
            continue;
        };
        if skill == "girth" || skill.starts_with("girth:") || skill.starts_with("girth-") {
            continue;
        }
        if let Some((ts, t)) = turns
            .iter()
            .find(|(ts, t)| *ts >= r.ts && &t.project == project)
        {
            let entry = per_skill.entry(skill.as_str()).or_default();
            if entry.1.insert(*ts) {
                entry.0 += turn_cost(t);
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
        "/compact <what to keep> at a natural break, or /clear when you switch tasks".into(),
        "the summary keeps what you name; /clear starts clean for unrelated work",
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
            "girth advise ({when}): ${:.2} estimated, nothing stands out\n",
            advice.spend
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
    s
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
            project: "/p".into(),
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
                skill: skill.into(),
                project: "/p".into(),
            },
        }
    }

    const SONNET: &str = "claude-sonnet-5-5";

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
}
