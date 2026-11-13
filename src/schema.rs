//! What MCP tool schemas cost: each server's schema tokens that every main-loop request
//! carries (from `schema` ledger events), times the requests of its session's turns. A
//! session writes them to the cache once, then reads them (a cache break writes them
//! again). The tokens are Claude Code's own estimate, so the figure is one too. They are
//! already part of the turns' cost; this names the share.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::ledger::{Event, Record, Schema};
use crate::price;

/// One server's schemas across the turns that carried them.
#[derive(Debug, Default, Serialize, PartialEq)]
pub struct McpCost {
    /// The tool-prefix id (`mcp__<server>__…`).
    pub server: String,
    /// The most schema tokens a request carried for it.
    pub tokens: u64,
    /// Main-loop requests and turns that carried them.
    pub requests: u64,
    pub turns: u64,
    /// Turns (main loop or subagent) of those sessions that called one of its tools.
    pub called: u64,
    /// Dollars for one cache write per session and cache reads after. Cache breaks add
    /// writes, and Claude Code's estimate runs about 13% over the real count: an estimate.
    pub cost: f64,
}

/// Servers whose schemas went out with requests, costliest first. `keep` picks the turns
/// that count (a project, a window); a schema recorded before them still applies.
pub fn mcp_costs(records: &[Record], keep: impl Fn(&Record) -> bool) -> Vec<McpCost> {
    let mut schemas: BTreeMap<&str, Vec<(u64, &Schema)>> = BTreeMap::new();
    for r in records {
        if let Event::Schema(s) = &r.event {
            if let Some(id) = &s.session {
                schemas.entry(id).or_default().push((r.ts, s));
            }
        }
    }
    let mut out: BTreeMap<&str, McpCost> = BTreeMap::new();
    let mut written: std::collections::BTreeSet<(&str, &str)> = Default::default();
    for r in records.iter().filter(|r| keep(r)) {
        let Event::Turn(t) = &r.event else { continue };
        let Some((session, list)) = t
            .session
            .as_deref()
            .filter(|_| t.harness.is_none())
            .and_then(|s| Some((s, schemas.get(s)?)))
        else {
            continue;
        };
        // The schema in effect at the turn: the latest at or before it, else the
        // session's first (the mod records it after the first turn ends).
        let schema = list
            .iter()
            .rev()
            .find(|(ts, _)| *ts <= r.ts)
            .unwrap_or(&list[0])
            .1;
        for server in &schema.servers {
            let called = t.mcp.contains(&server.server);
            if server.sent == 0 || (t.agent && !called) {
                continue;
            }
            let row = out.entry(&server.server).or_insert_with(|| McpCost {
                server: server.server.clone(),
                ..Default::default()
            });
            row.called += u64::from(called);
            if t.agent {
                continue;
            }
            let requests = t.requests.unwrap_or(1).max(1);
            row.tokens = row.tokens.max(server.sent);
            row.requests += requests;
            row.turns += 1;
            let rates = price::turn_rates_at(&t.model, false, r.ts)
                .map(|rates| price::long_prompt(rates, &t.model, t.prompt()));
            if let Some(rates) = rates {
                let reads = if written.insert((&server.server, session)) {
                    row.cost += server.sent as f64 * rates.cache_write / 1e6;
                    requests - 1
                } else {
                    requests
                };
                row.cost += (server.sent * reads) as f64 * rates.cache_read / 1e6;
            }
        }
    }
    let mut rows: Vec<McpCost> = out.into_values().filter(|r| r.turns > 0).collect();
    rows.sort_by(|a, b| {
        b.cost
            .total_cmp(&a.cost)
            .then_with(|| a.server.cmp(&b.server))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{ServerSchema, Turn};

    fn schema(ts: u64, session: &str, servers: &[(&str, u64, u64)]) -> Record {
        Record {
            v: 1,
            ts,
            event: Event::Schema(Schema {
                project: "/p".into(),
                session: Some(session.into()),
                servers: servers
                    .iter()
                    .map(|(s, sent, deferred)| ServerSchema {
                        server: (*s).into(),
                        tools: 3,
                        sent: *sent,
                        deferred: *deferred,
                    })
                    .collect(),
            }),
        }
    }

    fn turn(ts: u64, session: &str, requests: u64, agent: bool, mcp: &[&str]) -> Record {
        Record {
            v: 1,
            ts,
            event: Event::Turn(Turn {
                agent_id: None,
                agent_type: None,
                harness: None,
                premium: None,
                cost_micros: None,
                context: None,
                tool_output: Default::default(),
                project: "/p".into(),
                session: Some(session.into()),
                // $0.10 per million cache-read tokens.
                model: "claude-haiku-5-5".into(),
                input: 1,
                output: 1,
                cache_read: 1,
                cache_write: 1,
                agent,
                aborted: false,
                lens_before: None,
                lens_after: None,
                requests: Some(requests),
                mcp: mcp.iter().map(|s| (*s).into()).collect(),
                breaks: vec![],
                outcomes: vec![],
            }),
        }
    }

    fn rate() -> f64 {
        price::rates("claude-haiku-5-5").unwrap().cache_read
    }

    /// One main-loop cache write and `reads` cache reads of `tokens`.
    fn cost(tokens: u64, reads: u64) -> f64 {
        let r = price::turn_rates("claude-haiku-5-5", false).unwrap();
        (tokens as f64 * r.cache_write + (tokens * reads) as f64 * r.cache_read) / 1e6
    }

    #[test]
    fn sent_schemas_cost_tokens_times_requests_at_cache_read_rates() {
        let records = vec![
            turn(10, "s", 4, false, &[]),
            // Recorded after the first turn ended: it applies to that turn too.
            schema(11, "s", &[("github", 20_000, 0), ("figma", 0, 35_000)]),
            turn(20, "s", 6, false, &["github"]),
        ];
        let rows = mcp_costs(&records, |_| true);
        assert_eq!(
            rows.len(),
            1,
            "a deferred-only server costs nothing: {rows:?}"
        );
        let g = &rows[0];
        assert_eq!(
            (g.server.as_str(), g.tokens, g.requests, g.turns, g.called),
            ("github", 20_000, 10, 2, 1)
        );
        assert!(
            // Written once, then read by the other 9 requests.
            (g.cost - cost(20_000, 9)).abs() < 1e-12,
            "{}",
            g.cost
        );
    }

    #[test]
    fn schemas_in_a_prompt_over_100k_tokens_pay_haiku_s_long_prompt_price() {
        let mut t = turn(2, "s", 1, false, &[]);
        if let Event::Turn(t) = &mut t.event {
            t.cache_write = 148_715;
        }
        let records = vec![schema(1, "s", &[("github", 20_000, 0)]), t];
        let rows = mcp_costs(&records, |_| true);
        // Written once at $1.00 per million, not $0.20.
        assert!(
            (rows[0].cost - 20_000.0 * 1.0 / 1e6).abs() < 1e-12,
            "{}",
            rows[0].cost
        );
    }

    #[test]
    fn a_later_schema_replaces_the_earlier_one_from_its_time_on() {
        let records = vec![
            schema(1, "s", &[("github", 1_000, 0)]),
            turn(2, "s", 1, false, &[]),
            schema(3, "s", &[("github", 5_000, 0), ("linear", 2_000, 0)]),
            turn(4, "s", 2, false, &[]),
        ];
        let rows = mcp_costs(&records, |_| true);
        let get = |s: &str| rows.iter().find(|r| r.server == s).unwrap();
        assert_eq!((get("github").tokens, get("github").requests), (5_000, 3));
        // 1,000 written; then 5,000 × 2 read, since the session already wrote
        // github once (a floor).
        let want = cost(1_000, 0) + 10_000.0 * rate() / 1e6;
        assert!((get("github").cost - want).abs() < 1e-12);
        assert_eq!(get("linear").requests, 2);
        // Costliest first: linear's one write costs more than github's reads.
        assert_eq!(rows[0].server, "linear");
    }

    #[test]
    fn other_sessions_imported_turns_and_unkept_turns_do_not_count() {
        let mut imported = turn(5, "s", 9, false, &[]);
        if let Event::Turn(t) = &mut imported.event {
            t.harness = Some("codex".into());
        }
        let records = vec![
            schema(1, "s", &[("github", 1_000, 0)]),
            turn(2, "other", 7, false, &[]),
            imported,
            turn(6, "s", 3, false, &[]),
            turn(100, "s", 50, false, &[]),
        ];
        let rows = mcp_costs(&records, |r| r.ts < 100);
        assert_eq!((rows[0].requests, rows[0].turns), (3, 1));
    }

    #[test]
    fn subagent_turns_count_only_as_calls() {
        let records = vec![
            schema(1, "s", &[("github", 1_000, 0)]),
            turn(2, "s", 2, false, &[]),
            turn(3, "s", 8, true, &["github"]),
            turn(4, "s", 8, true, &[]),
        ];
        let rows = mcp_costs(&records, |_| true);
        assert_eq!((rows[0].requests, rows[0].turns, rows[0].called), (2, 1, 1));
    }

    #[test]
    fn a_server_only_subagents_called_without_main_turns_is_left_out() {
        let records = vec![
            schema(1, "s", &[("github", 1_000, 0)]),
            turn(3, "s", 8, true, &["github"]),
        ];
        assert!(mcp_costs(&records, |_| true).is_empty());
    }
}
