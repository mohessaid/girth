//! Hermes Agent spend as ledger turns, from its own SQLite state (`~/.hermes/state.db`,
//! or `$HERMES_HOME/state.db`), read through `sqlite3 -readonly -json`.
//!
//! Hermes keeps running totals, not per-turn records:
//! - **`session_model_usage`:** one row per session, model, billing route and task
//!   (`''` for the main loop, otherwise an auxiliary task such as `compression`). Its
//!   CLI path adds every API call's delta there.
//! - **`sessions`:** the row a gateway session (Telegram and the like) keeps its totals
//!   on. It is read only for sessions with no `session_model_usage` row, since it would
//!   double-count the rest.
//!
//! Each import appends what grew since the last one, as one turn per row, and keeps
//! the totals it imported in `import.json`. A total that shrank (a reset row) starts over.
//!
//! **What the columns mean,** from Hermes's `agent/usage_pricing.py` (v0.19.0):
//! - `input_tokens` excludes cache reads and writes.
//! - `output_tokens` is the provider's output count, which already includes reasoning.
//!
//! The cost is Hermes's `actual_cost_usd` when above zero, else its `estimated_cost_usd`.
//! A row whose session has a parent (a delegated run), or with a task, is side work
//! (`agent`). The project is the session's git root, else its working folder.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::ledger::{Event, Turn};

const HARNESS: &str = "hermes";

/// Per-model rows, then the session rows of sessions that have none.
pub const QUERY: &str = "SELECT 'm' AS src, u.session_id AS session, u.model AS model, \
u.task AS task, u.billing_provider || '|' || u.billing_base_url || '|' || u.billing_mode AS route, \
u.api_call_count AS calls, u.input_tokens AS input, u.output_tokens AS output, \
u.cache_read_tokens AS cache_read, u.cache_write_tokens AS cache_write, \
CASE WHEN COALESCE(u.actual_cost_usd, 0) > 0 THEN u.actual_cost_usd ELSE COALESCE(u.estimated_cost_usd, 0) END AS cost, \
COALESCE(u.last_seen, u.first_seen, s.started_at) AS ts, \
COALESCE(s.git_repo_root, s.cwd, '') AS project, s.parent_session_id AS parent \
FROM session_model_usage u JOIN sessions s ON s.id = u.session_id \
UNION ALL SELECT 's', s.id, COALESCE(s.model, ''), '', '', COALESCE(s.api_call_count, 0), \
COALESCE(s.input_tokens, 0), COALESCE(s.output_tokens, 0), COALESCE(s.cache_read_tokens, 0), \
COALESCE(s.cache_write_tokens, 0), \
CASE WHEN COALESCE(s.actual_cost_usd, 0) > 0 THEN s.actual_cost_usd ELSE COALESCE(s.estimated_cost_usd, 0) END, \
COALESCE(s.ended_at, s.started_at), COALESCE(s.git_repo_root, s.cwd, ''), s.parent_session_id \
FROM sessions s WHERE NOT EXISTS (SELECT 1 FROM session_model_usage u WHERE u.session_id = s.id)";

/// One row of [`QUERY`]: running totals.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Row {
    pub src: String,
    pub session: String,
    pub model: String,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub route: String,
    #[serde(default)]
    pub calls: u64,
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default)]
    pub cache_read: u64,
    #[serde(default)]
    pub cache_write: u64,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub ts: f64,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub parent: Option<String>,
}

impl Row {
    fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.src, self.session, self.model, self.task, self.route
        )
    }
}

const FIELDS: [&str; 6] = [
    "calls",
    "input",
    "output",
    "cache_read",
    "cache_write",
    "micros",
];

/// Turns for what grew in each row since the totals in `seen`, which are updated.
pub fn parse(rows: &[Row], seen: &mut BTreeMap<String, usize>) -> Vec<(u64, Event)> {
    let mut out = Vec::new();
    for row in rows {
        let key = row.key();
        let now = [
            row.calls,
            row.input,
            row.output,
            row.cache_read,
            row.cache_write,
            (row.cost.max(0.0) * 1e6).round() as u64,
        ];
        let before: Vec<u64> = FIELDS
            .iter()
            .map(|f| seen.get(&format!("{key}#{f}")).copied().unwrap_or(0) as u64)
            .collect();
        // A total that went down is a reset row: count it from zero.
        let reset = now.iter().zip(&before).any(|(n, b)| n < b);
        let delta: Vec<u64> = now
            .iter()
            .zip(&before)
            .map(|(n, b)| if reset { *n } else { n - b })
            .collect();
        for (f, n) in FIELDS.iter().zip(now) {
            seen.insert(format!("{key}#{f}"), n as usize);
        }
        if delta[1..5].iter().all(|d| *d == 0) {
            continue;
        }
        let turn = Turn {
            agent_id: None,
            agent_type: None,
            project: row.project.clone(),
            session: Some(row.session.clone()),
            model: row.model.clone(),
            input: delta[1],
            output: delta[2],
            cache_read: delta[3],
            cache_write: delta[4],
            agent: row.parent.is_some() || !row.task.is_empty(),
            aborted: false,
            lens_before: None,
            lens_after: None,
            requests: (delta[0] > 0).then_some(delta[0]),
            mcp: vec![],
            breaks: vec![],
            outcomes: vec![],
            harness: Some(HARNESS.into()),
            premium: None,
            cost_micros: (delta[5] > 0).then_some(delta[5]),
            context: None,
            tool_output: Default::default(),
        };
        out.push((row.ts.max(0.0) as u64, Event::Turn(turn)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(calls: u64, input: u64, cost: f64) -> Row {
        Row {
            src: "m".into(),
            session: "s".into(),
            model: "claude-sonnet-5-5".into(),
            task: String::new(),
            route: "anthropic||api".into(),
            calls,
            input,
            output: 7,
            cache_read: 0,
            cache_write: 0,
            cost,
            ts: 1794600000.9,
            project: "/p".into(),
            parent: None,
        }
    }

    fn turn(e: &(u64, Event)) -> &Turn {
        match &e.1 {
            Event::Turn(t) => t,
            _ => panic!("not a turn"),
        }
    }

    #[test]
    fn only_growth_is_a_turn_and_a_reset_row_starts_over() {
        let mut seen = BTreeMap::new();
        let first = parse(&[row(2, 100, 0.5)], &mut seen);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].0, 1794600000);
        assert_eq!(
            (turn(&first[0]).input, turn(&first[0]).requests),
            (100, Some(2))
        );
        assert_eq!(turn(&first[0]).cost_micros, Some(500_000));
        assert!(parse(&[row(2, 100, 0.5)], &mut seen).is_empty());
        let grew = parse(&[row(5, 160, 0.75)], &mut seen);
        assert_eq!(
            (
                turn(&grew[0]).input,
                turn(&grew[0]).output,
                turn(&grew[0]).requests,
                turn(&grew[0]).cost_micros
            ),
            (60, 0, Some(3), Some(250_000))
        );
        // Totals smaller than imported: the row was reset, so all of it is new.
        let reset = parse(&[row(1, 40, 0.0)], &mut seen);
        assert_eq!(
            (turn(&reset[0]).input, turn(&reset[0]).cost_micros),
            (40, None)
        );
    }

    #[test]
    fn calls_or_cost_without_tokens_add_no_turn_and_rows_are_kept_apart() {
        let mut seen = BTreeMap::new();
        let mut r = row(1, 0, 0.0);
        r.output = 0;
        assert!(parse(&[r], &mut seen).is_empty());
        let mut other = row(1, 10, 0.0);
        other.task = "title".into();
        let both = parse(&[row(1, 10, 0.0), other], &mut seen);
        assert_eq!(both.len(), 2);
        // The calls were counted on the earlier, token-less import: none are new here.
        assert_eq!(turn(&both[0]).requests, None);
        assert!(!turn(&both[0]).agent && turn(&both[1]).agent);
        let mut child = row(1, 10, 0.0);
        child.session = "c".into();
        child.parent = Some("s".into());
        assert!(turn(&parse(&[child], &mut seen)[0]).agent);
    }
}
