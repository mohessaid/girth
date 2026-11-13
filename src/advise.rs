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
/// A cache break this soon after the previous request cannot be an expiry (the shortest
/// prompt-cache lifetime is 5 minutes): the prompt changed.
pub const LIVE_CACHE_SECS: u64 = 300;
/// Turns taken more than this many seconds after a session's first turn are late.
pub const LONG_SESSION: u64 = 8 * 3600;

/// Levers that are already on, so their advice is suppressed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct On {
    pub keeper: bool,
    pub compact_idle: bool,
    pub agents_model: bool,
    /// The harness the advice is for (`None`: Claude Code), so fixes name its own
    /// command for a fresh session and its instructions file.
    pub harness: Option<&'static str>,
}

/// A harness's command for a fresh session and the instructions file it reads, as its
/// own docs and command lists name them (2026-11-06).
fn names(harness: Option<&str>) -> (&'static str, &'static str) {
    match harness {
        Some("codex") => ("/new", "AGENTS.md"),
        Some("copilot" | "opencode") => ("/clear", "AGENTS.md"),
        Some("gemini") => ("/clear", "GEMINI.md"),
        Some("antigravity") => ("/clear", "AGENTS.md"),
        _ => ("/clear", "CLAUDE.md"),
    }
}

/// What a tool's large results are, by its name in each harness (Claude Code, Codex,
/// Copilot, OpenCode).
#[derive(Clone, Copy, PartialEq)]
enum Output {
    Shell,
    File,
    Fetch,
    /// Searches, MCP calls, subagents and the rest: no one fix fits them.
    Other,
}

fn output_kind(tool: &str) -> Output {
    match tool.to_ascii_lowercase().as_str() {
        "exec" | "js" => Output::Shell,
        t if t.contains("bash") || t.contains("shell") => Output::Shell,
        "read" | "view" | "read_file" | "read_many_files" => Output::File,
        t if t.contains("file_contents") => Output::File,
        t if t.contains("fetch") => Output::Fetch,
        _ => Output::Other,
    }
}

/// The fix and guard for large tool output: one part per kind of tool with at least a
/// fifth of the large tokens, largest first; other tools count toward the total only.
/// When no known kind qualifies, a general fix. RTK filters Claude Code's Bash tool only.
fn output_fix(turns: &[(u64, &Turn)], harness: Option<&str>) -> (String, String) {
    let mut kinds: Vec<(Output, u64)> = Vec::new();
    for (tool, n) in turns.iter().flat_map(|(_, t)| &t.tool_output) {
        let k = output_kind(tool);
        match kinds.iter_mut().find(|(o, _)| *o == k) {
            Some(e) => e.1 += n,
            None => kinds.push((k, *n)),
        }
    }
    kinds.sort_by_key(|k| std::cmp::Reverse(k.1));
    let total: u64 = kinds.iter().map(|k| k.1).sum();
    let claude = matches!(harness, None | Some("claude-code"));
    let (mut fix, mut guard) = (Vec::new(), Vec::new());
    for (k, n) in kinds {
        if n * 5 < total {
            break;
        }
        let (f, g) = match (k, claude) {
            (Output::Shell, true) => (
                "end long commands with | tail -n 40 or | head, or use a shell-output filter such as RTK",
                "tail keeps the summary and errors at the end; a filter keeps errors and summaries",
            ),
            (Output::Shell, false) => (
                "end long commands with | tail -n 40 or | head",
                "tail keeps the summary and errors at the end",
            ),
            (Output::File, _) => (
                "read large files by line range",
                "a range keeps the part being worked on",
            ),
            (Output::Fetch, _) => (
                "fetch large pages in a subagent, so only its summary stays in the conversation",
                "the subagent still reads the whole page",
            ),
            (Output::Other, _) => continue,
        };
        fix.push(f);
        guard.push(g);
    }
    if fix.is_empty() {
        fix.push("ask tools for less: narrower searches and patterns, smaller ranges and limits");
        guard.push("a narrower query still finds what the task needs");
    }
    (fix.join("; "), guard.join("; "))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Contributor {
    /// `agent_summary`, `mcp_schemas`, `skill`, `context`, `sessions`, `prompt_changes`,
    /// `rewrites`, `subagents` or `tool_output`.
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
fn turn_cost(ts: u64, t: &Turn) -> f64 {
    if let Some(micros) = t.cost_micros {
        return micros as f64 / 1e6;
    }
    price::rates_for_turn(t, ts).map_or(0.0, |r| {
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
    // Claude Code's requests outside turns (progress summaries, suggestions, titles).
    let sides: Vec<(u64, &crate::ledger::Side)> = records
        .iter()
        .filter(in_window)
        .filter_map(|r| match &r.event {
            Event::Side(s) => Some((r.ts, s)),
            _ => None,
        })
        .collect();
    let side_spend: f64 = sides
        .iter()
        .map(|(ts, s)| crate::meter::side_cost(s, *ts))
        .sum();
    let spend: f64 = turns.iter().map(|(ts, t)| turn_cost(*ts, t)).sum::<f64>() + side_spend;
    let unpriced = turns
        .iter()
        .filter(|(_, t)| t.cost_micros.is_none() && price::rates(&t.model).is_none())
        .count() as u64;
    let mut harnesses: BTreeMap<String, f64> = BTreeMap::new();
    for (ts, t) in &turns {
        let h = t.harness.as_deref().unwrap_or("claude-code");
        *harnesses.entry(h.to_string()).or_default() += turn_cost(*ts, t);
    }
    if side_spend > 0.0 {
        *harnesses.entry("claude-code".to_string()).or_default() += side_spend;
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
    let (clear, memory) = names(on.harness);
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

    // Background agents' progress summaries: Claude Code re-sends each running agent's
    // transcript about every 30 s, uncached, to draw its progress line.
    push(
        &mut out,
        "agent_summary",
        "background-agent progress summaries".into(),
        sides
            .iter()
            .filter(|(_, s)| s.source == "agent_summary")
            .map(|(ts, s)| crate::meter::side_cost(s, *ts))
            .sum(),
        "set CLAUDE_CODE_DISABLE_AGENT_PROGRESS_SUMMARIES=1 in your shell or settings env (checked on Claude Code 2.1.319)".into(),
        "background agents run the same; only their progress line goes",
    );

    // MCP tool schemas every main-loop request carried: Claude Code sends them all when
    // tool search is off, and a server's schemas when the session searched for its tools.
    let schemas = crate::schema::mcp_costs(records, |r| in_window(&r));
    let unused: Vec<&str> = schemas
        .iter()
        .filter(|r| r.called == 0)
        .map(|r| r.server.as_str())
        .collect();
    let mut fix = "keep tool search on (leave ENABLE_TOOL_SEARCH unset), so a tool's schema is sent only once it is needed".to_string();
    if !unused.is_empty() {
        fix.push_str(&format!(
            "; disable servers you don't use in /mcp (never called: {})",
            unused.join(", ")
        ));
    }
    push(
        &mut out,
        "mcp_schemas",
        "MCP tool schemas on every request".into(),
        schemas.iter().map(|r| r.cost).sum(),
        fix,
        "every tool stays available: tool search finds and loads it when a task calls for it",
    );

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
                entry.0 += turn_cost(turns[i].0, turns[i].1);
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
        .map(|(ts, t)| turn_cost(*ts, t))
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
        .map(|(ts, t)| turn_cost(*ts, t))
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

    // Cache rewrites: after a gap the cache may have expired (the keeper's case); within
    // LIVE_CACHE_SECS it could not have, so the prompt changed in front of the cached part.
    let rewritten = |live: bool| -> f64 {
        turns
            .iter()
            .filter_map(|(ts, t)| {
                let r = price::long_prompt(price::rates_at(&t.model, *ts)?, &t.model, t.prompt());
                let tokens: u64 = t
                    .breaks
                    .iter()
                    .filter(|b| (b.idle_s < LIVE_CACHE_SECS) == live)
                    .map(|b| b.rewritten)
                    .sum();
                Some(price::cost(r, 0, 0, 0, tokens))
            })
            .sum()
    };
    let (fix, guard) = if schemas.is_empty() {
        (
            "keep the start of the prompt fixed within a session: switching models, editing CLAUDE.md or settings, or connecting MCP servers rewrites everything after it",
            "make those changes between sessions; nothing else changes",
        )
    } else {
        (
            "keep tool search on (leave ENABLE_TOOL_SEARCH unset): with it off, an MCP server whose tools arrive after the first request changes the tools at the start of the prompt",
            "every tool stays available: tool search finds and loads it when a task calls for it",
        )
    };
    push(
        &mut out,
        "prompt_changes",
        "cache rewrites while the cache was live".into(),
        rewritten(true),
        fix.into(),
        guard,
    );
    let rewrites = rewritten(false);
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
            .map(|(ts, t)| turn_cost(*ts, t))
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

    // Large tool output: written to the cache once, then re-read by every later request
    // until the conversation shrinks. Half its own turn's requests carry it (where in the
    // turn it came isn't recorded); on the main loop, so does every later main turn of
    // its session up to the first whose context is smaller than the one before.
    let large_output: f64 = turns
        .iter()
        .enumerate()
        .filter_map(|(i, (ts, t))| {
            let tokens: u64 = t.tool_output.values().sum();
            if tokens == 0 {
                return None;
            }
            let r = price::rates_for_turn(t, *ts)?;
            let mut carried = t.requests.unwrap_or(1) as f64 / 2.0;
            if let (Some(s), false) = (&t.session, t.agent) {
                let mut before = t.context;
                let rest = turns[i + 1..]
                    .iter()
                    .filter(|(_, l)| !l.agent && l.session.as_ref() == Some(s));
                for (_, later) in rest {
                    if let (Some(b), Some(c)) = (before, later.context) {
                        if c < b {
                            break;
                        }
                    }
                    before = later.context.or(before);
                    carried += later.requests.unwrap_or(1) as f64;
                }
            }
            let read = tokens as f64 * carried * r.cache_read / 1e6;
            Some(price::cost(r, 0, 0, 0, tokens) + read)
        })
        .sum();
    let (fix, guard) = output_fix(&turns, on.harness);
    push(
        &mut out,
        "tool_output",
        "large tool output".into(),
        large_output,
        fix,
        &guard,
    );

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
            agent_id: None,
            agent_type: None,
            harness: None,
            premium: None,
            cost_micros: None,
            context: None,
            tool_output: Default::default(),
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

    /// A model with one price for all time, so test timestamps don't pick a dated rate.
    const SONNET: &str = "claude-haiku-5-5";

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

    fn bash_out(session: Option<&str>, requests: u64, tokens: u64, context: u64) -> Turn {
        Turn {
            session: session.map(str::to_string),
            requests: Some(requests),
            context: Some(context),
            tool_output: [("Bash".to_string(), tokens)]
                .into_iter()
                .filter(|(_, n)| *n > 0)
                .collect(),
            ..turn(SONNET, 0, 100_000)
        }
    }

    fn tool_output(a: &Advice) -> Option<&Contributor> {
        a.contributors.iter().find(|c| c.kind == "tool_output")
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn large_bash_output_is_charged_until_the_conversation_shrinks() {
        let s = Some("s");
        let records = [
            // 10,000 tokens of large output; half of its 4 requests carry it.
            rec(10, bash_out(s, 4, 10_000, 50_000)),
            // A subagent's turn carries its own context, not the main loop's.
            rec(
                15,
                Turn {
                    agent: true,
                    ..bash_out(s, 9, 0, 0)
                },
            ),
            // The same size is no shrink.
            rec(20, bash_out(s, 3, 0, 50_000)),
            // Smaller than the turn before: compacted or cleared, so the output is gone.
            rec(30, bash_out(s, 5, 0, 20_000)),
            rec(40, bash_out(s, 6, 0, 90_000)),
            rec(50, bash_out(Some("other"), 7, 0, 90_000)),
        ];
        let advice = advise(&records, 100, 1000, On::default());
        let c = tool_output(&advice).expect("a large-output contributor");
        let r = price::rates(SONNET).unwrap();
        let want = price::cost(r, 0, 0, 10_000 * (2 + 3), 10_000);
        assert!(close(c.cost, want), "{} vs {want}", c.cost);
        assert_eq!(c.label, "large tool output");
        assert!(
            c.fix.contains("| tail") && c.fix.contains("filter"),
            "{}",
            c.fix
        );
        assert!(c.guard.contains("errors"), "{}", c.guard);
    }

    fn side(ts: u64, source: &str, micros: u64) -> Record {
        Record {
            v: 1,
            ts,
            event: Event::Side(crate::ledger::Side {
                project: "/p".into(),
                session: None,
                source: source.into(),
                model: "claude-opus-5-5".into(),
                requests: 3,
                input: 1_000,
                output: 10,
                cache_read: 0,
                cache_write: 0,
                cost_micros: Some(micros),
            }),
        }
    }

    #[test]
    fn side_queries_count_in_spend_and_progress_summaries_get_their_fix() {
        let records = [
            rec(10, turn(SONNET, 0, 100_000)),
            side(20, "agent_summary", 250_000),
            side(30, "agent_summary", 250_000),
            side(40, "prompt_suggestion", 100_000),
            // Outside the window: not counted.
            side(5_000, "agent_summary", 9_000_000),
        ];
        let advice = advise(&records, 100, 1000, On::default());
        let turn_cost = price::cost(price::rates(SONNET).unwrap(), 0, 100_000, 0, 0);
        assert!(close(advice.spend, turn_cost + 0.6), "{}", advice.spend);
        assert!(close(advice.harnesses["claude-code"], advice.spend));
        let c = advice
            .contributors
            .iter()
            .find(|c| c.kind == "agent_summary")
            .expect("a progress-summary contributor");
        assert!(close(c.cost, 0.5), "{}", c.cost);
        assert_eq!(c.label, "background-agent progress summaries");
        assert!(
            c.fix
                .contains("CLAUDE_CODE_DISABLE_AGENT_PROGRESS_SUMMARIES=1"),
            "{}",
            c.fix
        );
        assert!(c.guard.contains("progress"), "{}", c.guard);
        // Other side sources have no verified fix: spend only.
        assert_eq!(
            advice
                .contributors
                .iter()
                .filter(|c| c.kind == "agent_summary")
                .count(),
            1
        );
        assert!(
            advice
                .contributors
                .iter()
                .all(|c| !c.label.contains("prompt_suggestion"))
        );
        // Side queries alone are spend too.
        let alone = advise(&[side(20, "compact", 300_000)], 100, 1000, On::default());
        assert!(close(alone.spend, 0.3), "{}", alone.spend);
    }

    #[test]
    fn spend_prices_each_turn_at_the_rates_of_its_day() {
        const OCT_8: u64 = 1_791_417_600;
        let sonnet = "claude-sonnet-5-5";
        let records = [
            rec(OCT_8 - 60, turn(sonnet, 1_000_000, 0)),
            rec(OCT_8 + 60, turn(sonnet, 1_000_000, 0)),
        ];
        let advice = advise(&records, OCT_8 + 100, 1000, On::default());
        // $0.20 then $0.10 per million cache reads.
        assert!(close(advice.spend, 0.30), "{}", advice.spend);
    }

    #[test]
    fn each_harness_names_its_own_fresh_session_command_and_instructions_file() {
        assert_eq!(names(Some("antigravity")), ("/clear", "AGENTS.md"));
        assert_eq!(names(Some("gemini")), ("/clear", "GEMINI.md"));
        assert_eq!(names(Some("codex")), ("/new", "AGENTS.md"));
        assert_eq!(names(None), ("/clear", "CLAUDE.md"));
    }

    #[test]
    fn other_harnesses_trim_large_output_without_rtk() {
        let records = [rec(10, bash_out(None, 2, 8_000, 50_000))];
        let on = On {
            harness: Some("codex"),
            ..On::default()
        };
        let c = advise(&records, 100, 1000, on);
        let c = tool_output(&c).unwrap();
        // RTK hooks Claude Code's Bash tool; tail works in every shell.
        assert!(
            c.fix.contains("| tail -n 40") && !c.fix.contains("RTK"),
            "{}",
            c.fix
        );
        assert!(
            c.guard.contains("errors") && !c.guard.contains("filter"),
            "{}",
            c.guard
        );
    }

    fn outputs(tools: &[(&str, u64)]) -> Turn {
        Turn {
            tool_output: tools.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            ..bash_out(None, 2, 0, 50_000)
        }
    }

    #[test]
    fn haiku_turns_over_100k_tokens_per_request_pay_the_long_prompt_price() {
        let mut t = turn("claude-haiku-5-5", 0, 190);
        (t.input, t.cache_write, t.requests) = (4, 327_745, Some(2));
        t.breaks = vec![Break {
            lost: 150_000,
            rewritten: 150_000,
            idle_s: 4_000,
        }];
        t.tool_output = [("Bash".to_string(), 10_000)].into_iter().collect();
        let advice = advise(&[rec(10, t)], 100, 1000, On::default());
        assert!((advice.spend - 0.328222).abs() < 1e-6, "{}", advice.spend);
        let get = |k: &str| {
            advice
                .contributors
                .iter()
                .find(|c| c.kind == k)
                .unwrap()
                .cost
        };
        assert!((get("rewrites") - 150_000.0 * 1.0 / 1e6).abs() < 1e-12);
        // 10,000 written at $1.00, then read by one of the turn's two requests at $0.05.
        assert!((get("tool_output") - (0.01 + 10_000.0 * 0.05 / 1e6)).abs() < 1e-12);
    }

    #[test]
    fn a_kind_with_exactly_a_fifth_of_the_large_tokens_is_named() {
        let advice = advise(
            &[rec(10, outputs(&[("webfetch", 8_000), ("read", 2_000)]))],
            100,
            1000,
            On::default(),
        );
        let fix = &tool_output(&advice).unwrap().fix;
        assert!(fix.contains("read large files by line range"), "{fix}");
    }

    #[test]
    fn other_harnesses_alone_put_no_claude_code_spend_in_the_split() {
        let mut codex = turn("claude-opus-5-5", 0, 1_000_000);
        codex.harness = Some("codex".into());
        let advice = advise(&[rec(10, codex)], 100, 1000, On::default());
        assert_eq!(advice.harnesses.keys().collect::<Vec<_>>(), ["codex"]);
    }

    #[test]
    fn the_fix_names_what_the_large_results_came_from() {
        let fix = |tools: &[(&str, u64)], harness| {
            let on = On {
                harness,
                ..On::default()
            };
            let c = advise(&[rec(10, outputs(tools))], 100, 1000, on);
            let c = tool_output(&c).unwrap().clone();
            (c.fix, c.guard)
        };
        // Mostly fetched pages, then file reads (`read` and `view` together pass a
        // fifth, neither alone does); bash under a fifth is left out.
        let (f, g) = fix(
            &[
                ("webfetch", 7_000),
                ("read", 1_100),
                ("view", 1_100),
                ("bash", 800),
            ],
            Some("opencode"),
        );
        assert!(f.starts_with("fetch large pages in a subagent"), "{f}");
        assert!(f.contains("read large files by line range"), "{f}");
        assert!(!f.contains("tail"), "{f}");
        assert!(g.contains("subagent") && g.contains("range"), "{g}");
        // Copilot's names: `view` is a file read, `web_fetch` a fetch, `read_bash` a shell.
        let (f, _) = fix(&[("view", 5_000), ("read_bash", 5_000)], Some("copilot"));
        assert!(
            f.contains("line range") && f.contains("| tail -n 40"),
            "{f}"
        );
        let (f, _) = fix(&[("web_fetch", 9_000)], Some("copilot"));
        assert!(f.starts_with("fetch large pages"), "{f}");
        // Claude Code's names; RTK is offered with the shell fix there only.
        let (f, _) = fix(
            &[("Bash", 5_000), ("Read", 5_000), ("WebFetch", 5_000)],
            None,
        );
        assert!(
            f.contains("RTK") && f.contains("line range") && f.contains("subagent"),
            "{f}"
        );
        // File readers by other names; `read_bash` reads a shell's output.
        let (f, _) = fix(&[("read_file", 9_000)], Some("gemini"));
        assert_eq!(f, "read large files by line range");
        let (f, _) = fix(
            &[("github-mcp-server-get_file_contents", 9_000)],
            Some("copilot"),
        );
        assert_eq!(f, "read large files by line range");
        let (f, _) = fix(&[("read_bash", 9_000)], Some("copilot"));
        assert!(f.contains("| tail -n 40"), "{f}");
        // Codex's shell runs under `exec` and `js`.
        let (f, _) = fix(&[("exec", 5_000), ("js", 5_000)], Some("codex"));
        assert!(f.contains("| tail -n 40") && !f.contains("RTK"), "{f}");
        // Tools of no known kind count toward the total but get no fix of their own.
        let (f, _) = fix(
            &[("run_shell_command", 6_000), ("resolve-library-id", 4_000)],
            Some("gemini"),
        );
        assert_eq!(f, "end long commands with | tail -n 40 or | head");
        let (f, _) = fix(&[("bash", 1_000), ("grep", 9_000)], Some("codex"));
        assert!(f.starts_with("ask tools for less"), "{f}");
        let (f, g) = fix(&[("mystery", 9_000)], Some("codex"));
        assert!(
            f.starts_with("ask tools for less") && !f.contains("RTK"),
            "{f}"
        );
        assert!(!g.is_empty(), "{g}");
    }

    #[test]
    fn large_output_without_a_session_is_charged_within_its_turn() {
        let records = [
            rec(10, bash_out(None, 3, 8_000, 50_000)),
            rec(20, bash_out(None, 9, 0, 60_000)),
        ];
        let c = advise(&records, 100, 1000, On::default());
        let r = price::rates(SONNET).unwrap();
        let want = price::cost(r, 0, 0, 12_000, 8_000); // 8,000 × 1.5 requests
        assert!(close(tool_output(&c).unwrap().cost, want));
    }

    #[test]
    fn a_subagent_s_large_output_stays_in_its_turn() {
        let agent = |tokens| Turn {
            agent: true,
            context: None,
            ..bash_out(Some("s"), 4, tokens, 0)
        };
        let records = [
            rec(10, agent(6_000)),
            rec(20, agent(0)),
            rec(30, bash_out(Some("s"), 5, 0, 1)),
        ];
        let c = advise(&records, 100, 1000, On::default());
        let r = price::rates(SONNET).unwrap().five_minute();
        let want = price::cost(r, 0, 0, 12_000, 6_000);
        assert!(close(tool_output(&c).unwrap().cost, want));
    }

    #[test]
    fn a_turn_without_its_context_does_not_end_the_carry() {
        let s = Some("s");
        let unknown = Turn {
            context: None,
            ..bash_out(s, 2, 0, 0)
        };
        let records = [
            rec(10, bash_out(s, 2, 4_000, 50_000)),
            rec(20, unknown),
            rec(30, bash_out(s, 2, 0, 40_000)),
            rec(40, bash_out(s, 2, 0, 30_000)),
        ];
        let c = advise(&records, 100, 1000, On::default());
        let r = price::rates(SONNET).unwrap();
        // Its own turn (1), the unknown one (2), then 40,000 < 50,000 stops it.
        let want = price::cost(r, 0, 0, 4_000 * 3, 4_000);
        assert!(close(tool_output(&c).unwrap().cost, want));
    }

    #[test]
    fn no_large_output_means_no_such_contributor() {
        let records = [rec(10, bash_out(Some("s"), 4, 0, 50_000))];
        assert!(tool_output(&advise(&records, 100, 1000, On::default())).is_none());
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
    fn each_harness_gets_its_own_names() {
        assert_eq!(names(None), ("/clear", "CLAUDE.md"));
        assert_eq!(names(Some("claude-code")), ("/clear", "CLAUDE.md"));
        assert_eq!(names(Some("codex")), ("/new", "AGENTS.md"));
        assert_eq!(names(Some("opencode")), ("/clear", "AGENTS.md"));
        assert_eq!(names(Some("gemini")), ("/clear", "GEMINI.md"));
    }

    #[test]
    fn copilot_and_opencode_advice_names_agents_md() {
        let on = On {
            harness: Some("copilot"),
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
            harness: Some("codex"),
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
        let one = price::cost(price::rates(SONNET).unwrap(), 0, 100_000, 0, 0);
        assert!(
            (a.spend - one).abs() < 1e-9,
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
    fn rewrites_while_the_cache_was_live_are_prompt_changes_not_idle() {
        let mut t = turn(SONNET, 0, 0);
        // Under Haiku 5.5's 100,000-token line: base prices.
        t.cache_write = 50_000;
        let brk = |idle_s, rewritten| Break {
            lost: rewritten,
            rewritten,
            idle_s,
        };
        // Two seconds apart: the cache cannot have expired. Five minutes: it may have.
        t.breaks = vec![brk(2, 200_000), brk(LIVE_CACHE_SECS, 100_000)];
        t.session = Some("s".into());
        let mut records = vec![rec(NOW - 90, t)];
        let get = |records: &[Record], kind: &str| {
            advise(records, NOW, WEEK, On::default())
                .contributors
                .into_iter()
                .find(|c| c.kind == kind)
        };
        let r = price::rates(SONNET).unwrap();
        let changed = get(&records, "prompt_changes").unwrap();
        assert!((changed.cost - price::cost(r, 0, 0, 0, 200_000)).abs() < 1e-12);
        let idle = get(&records, "rewrites").unwrap().cost;
        assert!((idle - price::cost(r, 0, 0, 0, 100_000)).abs() < 1e-12);
        assert!(
            changed.fix.contains("keep the start of the prompt"),
            "{}",
            changed.fix
        );
        // With MCP schemas sent in the window, the fix is tool search.
        records.push(Record {
            v: 1,
            ts: NOW - 95,
            event: Event::Schema(crate::ledger::Schema {
                project: "/p".into(),
                session: Some("s".into()),
                servers: vec![crate::ledger::ServerSchema {
                    server: "github".into(),
                    tools: 1,
                    sent: 1_000,
                    deferred: 0,
                }],
            }),
        });
        let changed = get(&records, "prompt_changes").unwrap();
        assert!(
            changed.fix.starts_with("keep tool search on"),
            "{}",
            changed.fix
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
