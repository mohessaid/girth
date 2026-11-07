//! OpenCode sessions as ledger turns, read through `opencode db` from OpenCode's own
//! SQLite database (`~/.local/share/opencode/opencode.db`), so Girth needs no SQLite code.
//!
//! A turn is one user prompt: the assistant messages whose `parentID` is that prompt.
//! Each message is one request. OpenCode's `tokens.input` excludes cache reads and
//! writes, and `tokens.output` excludes reasoning, which is billed as output.
//! OpenCode prices each message (`cost`, in dollars); a turn keeps that figure when every
//! request has one above zero, and otherwise falls back to list prices.
//!
//! A turn is done when a later prompt exists in its session, when its last request
//! finished for a reason other than `tool-calls`, or when a request logged an `error`.
//! A turn cut off by a later prompt or an error is recorded as aborted. A session left
//! mid-tool-call keeps its last turn open until it moves on. `opencode db` runs with
//! no timeout; its stdin is closed, so it can't wait on a prompt, but a slow database
//! migration would hold up `girth import`.
//!
//! A tool result (`part.state.output`) of `LARGE_OUTPUT_CHARS` or more goes with its
//! prompt's turn as `tool_output`, by tool name. OpenCode caps what it sends the model
//! and stores that capped text ("Output capped at 50 KB"). `context` is the last
//! request's whole input: uncached, cache read and cache write.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::ledger::{Event, LARGE_OUTPUT_CHARS, Turn};

const HARNESS: &str = "opencode";

/// Messages, `skill` tool calls, and every tool result's name and length (`n`, in
/// characters), oldest first. `t` is milliseconds since the epoch.
pub const QUERY: &str = "SELECT 'm' AS kind, m.id AS id, m.session_id AS session, \
s.parent_id AS parent, s.directory AS dir, m.time_created AS t, m.data AS data, NULL AS n \
FROM message m JOIN session s ON s.id = m.session_id \
UNION ALL SELECT 's', p.id, p.session_id, NULL, NULL, p.time_created, \
json_extract(p.data, '$.state.input.name'), NULL FROM part p \
WHERE json_extract(p.data, '$.tool') = 'skill' \
UNION ALL SELECT 't', p.id, p.session_id, NULL, NULL, p.time_created, \
json_extract(p.data, '$.tool'), length(json_extract(p.data, '$.state.output')) FROM part p \
WHERE json_extract(p.data, '$.type') = 'tool' ORDER BY t";

#[derive(Deserialize)]
struct Row {
    kind: String,
    id: String,
    session: String,
    parent: Option<String>,
    dir: Option<String>,
    t: u64,
    data: Option<String>,
    #[serde(default)]
    n: Option<u64>,
}

#[derive(Default)]
struct Prompt {
    id: String,
    t: u64,
    requests: Vec<Value>,
    skills: Vec<(u64, String)>,
    /// Estimated tokens of large tool results, by tool.
    outputs: BTreeMap<String, u64>,
}

/// Ledger events for the turns in `rows` (the JSON `QUERY` returns) that are done and
/// newer than each session's cursor, oldest first, and the cursors to keep: the time of
/// each session's last imported prompt, in milliseconds.
pub fn parse(
    rows: &str,
    cursors: &BTreeMap<String, usize>,
) -> (Vec<(u64, Event)>, BTreeMap<String, usize>) {
    let rows: Vec<Row> = serde_json::from_str(rows).unwrap_or_default();
    // Per session, in first-seen order: subagent parent, folder, prompts.
    let mut sessions: Vec<(String, bool, String, Vec<Prompt>)> = Vec::new();
    for row in rows {
        let i = match sessions.iter().position(|s| s.0 == row.session) {
            Some(i) => i,
            None => {
                let dir = row.dir.clone().unwrap_or_default();
                sessions.push((row.session.clone(), row.parent.is_some(), dir, vec![]));
                sessions.len() - 1
            }
        };
        let prompts = &mut sessions[i].3;
        let data = row.data.unwrap_or_default();
        if row.kind == "t" {
            let chars = row.n.unwrap_or(0) as usize;
            if let (Some(p), true) = (prompts.last_mut(), chars >= LARGE_OUTPUT_CHARS) {
                *p.outputs.entry(data).or_default() += chars.div_ceil(4) as u64;
            }
            continue;
        }
        if row.kind == "s" {
            if let Some(p) = prompts.last_mut() {
                p.skills.push((row.t, data));
            }
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        match msg["role"].as_str() {
            Some("user") => prompts.push(Prompt {
                id: row.id,
                t: row.t,
                ..Prompt::default()
            }),
            Some("assistant") => {
                let parent = msg["parentID"].as_str().unwrap_or_default();
                if let Some(p) = prompts.iter_mut().rev().find(|p| p.id == parent) {
                    p.requests.push(msg);
                }
            }
            _ => {}
        }
    }
    let mut events = Vec::new();
    let mut kept = BTreeMap::new();
    for (session, agent, dir, prompts) in sessions {
        let cursor = cursors.get(&session).copied().unwrap_or(0);
        for (i, p) in prompts.iter().enumerate() {
            if (p.t as usize) <= cursor {
                continue;
            }
            let last = p.requests.last();
            let error = p.requests.iter().any(|r| !r["error"].is_null());
            let finished = last.is_some_and(|r| {
                r["finish"].as_str().is_some_and(|f| f != "tool-calls")
                    && r["time"]["completed"].is_u64()
            });
            let later = i + 1 < prompts.len();
            if !(later || finished || error) {
                break;
            }
            kept.insert(session.clone(), p.t as usize);
            let Some(last) = last else {
                continue;
            };
            let mut t = turn(&session, agent, &dir, &p.requests);
            t.aborted = error || !finished;
            t.tool_output = p.outputs.clone();
            let ms = last["time"]["completed"].as_u64().unwrap_or(p.t);
            for (at, skill) in &p.skills {
                events.push((*at, use_event(skill, &t.project, &session, agent)));
            }
            events.push((ms, Event::Turn(t)));
        }
    }
    events.sort_by_key(|(ms, _)| *ms);
    let events = events.into_iter().map(|(ms, e)| (ms / 1000, e)).collect();
    (events, kept)
}

fn turn(session: &str, agent: bool, dir: &str, requests: &[Value]) -> Turn {
    let n = |v: &Value| v.as_u64().unwrap_or(0);
    let tokens = |k: &str| requests.iter().map(|r| n(&r["tokens"][k])).sum::<u64>();
    let cache = |k: &str| {
        requests
            .iter()
            .map(|r| n(&r["tokens"]["cache"][k]))
            .sum::<u64>()
    };
    let costs: Vec<f64> = requests.iter().filter_map(|r| r["cost"].as_f64()).collect();
    let priced = costs.len() == requests.len() && costs.iter().all(|c| *c > 0.0);
    let last = requests.last().cloned().unwrap_or_default();
    Turn {
        project: last["path"]["cwd"].as_str().unwrap_or(dir).to_string(),
        session: Some(session.to_string()),
        model: last["modelID"].as_str().unwrap_or_default().to_string(),
        input: tokens("input"),
        output: tokens("output") + tokens("reasoning"),
        cache_read: cache("read"),
        cache_write: cache("write"),
        agent,
        aborted: false,
        lens_before: None,
        lens_after: None,
        requests: Some(requests.len() as u64),
        mcp: vec![],
        breaks: vec![],
        outcomes: vec![],
        harness: Some(HARNESS.into()),
        premium: None,
        cost_micros: priced.then(|| costs.iter().map(|c| (c * 1e6).round() as u64).sum()),
        context: Some(
            n(&last["tokens"]["input"])
                + n(&last["tokens"]["cache"]["read"])
                + n(&last["tokens"]["cache"]["write"]),
        ),
        tool_output: Default::default(),
    }
}

fn use_event(skill: &str, project: &str, session: &str, agent: bool) -> Event {
    Event::Use {
        skill: skill.to_string(),
        project: project.to_string(),
        session: Some(session.to_string()),
        agent,
        harness: Some(HARNESS.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: &str = include_str!("../../tests/fixtures/opencode/rows.json");
    const BASE: u64 = 1_782_727_240_000;

    fn turns(rows: &str, cursors: &BTreeMap<String, usize>) -> Vec<(u64, Turn)> {
        parse(rows, cursors)
            .0
            .into_iter()
            .filter_map(|(ts, e)| match e {
                Event::Turn(t) => Some((ts, t)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn each_done_prompt_is_one_turn() {
        let got = turns(ROWS, &BTreeMap::new());
        let rows: Vec<_> = got
            .iter()
            .map(|(ts, t)| {
                (
                    *ts,
                    t.session.as_deref().unwrap(),
                    t.requests.unwrap(),
                    (t.input, t.output, t.cache_read, t.cache_write),
                    t.cost_micros,
                    t.agent,
                    t.aborted,
                )
            })
            .collect();
        let s = |ms: u64| (BASE + ms) / 1000;
        assert_eq!(
            rows,
            [
                // A subagent's session.
                (s(2200), "sesB", 1, (300, 30, 0, 0), Some(3000), true, false),
                // Two requests; reasoning counts as output.
                (
                    s(2500),
                    "sesA",
                    2,
                    (120, 20, 250, 30),
                    Some(3000),
                    false,
                    false
                ),
                // Cut off by the next prompt mid-tool-call; a logged cost of 0 is not $0.
                (s(3500), "sesA", 1, (40, 4, 0, 0), None, false, true),
                // Aborted with an error.
                (s(4100), "sesA", 1, (10, 1, 0, 0), Some(500), false, true),
            ]
        );
        let t = &got[1].1;
        assert_eq!(
            (t.model.as_str(), t.project.as_str(), t.harness.as_deref()),
            ("deepseek-v4-pro", "/work/o", Some("opencode"))
        );
    }

    #[test]
    fn skills_come_with_their_turn() {
        let uses: Vec<_> = parse(ROWS, &BTreeMap::new())
            .0
            .into_iter()
            .filter_map(|(ts, e)| match e {
                Event::Use { skill, session, .. } => Some((ts, skill, session)),
                _ => None,
            })
            .collect();
        // later-skill ran in the open turn, so it waits.
        assert_eq!(
            uses,
            [(
                (BASE + 1600) / 1000,
                "career-ops".to_string(),
                Some("sesA".to_string())
            )]
        );
    }

    #[test]
    fn cursors_skip_imported_prompts_and_hold_back_the_open_one() {
        let (_, cursors) = parse(ROWS, &BTreeMap::new());
        assert_eq!(cursors["sesA"], (BASE + 4000) as usize);
        assert_eq!(cursors["sesB"], (BASE + 2000) as usize);
        assert!(parse(ROWS, &cursors).0.is_empty());
        let mut older = cursors.clone();
        older.insert("sesA".into(), (BASE + 3000) as usize);
        let got = turns(ROWS, &older);
        assert_eq!(got.len(), 1);
        assert!(got[0].1.aborted && got[0].1.cost_micros == Some(500));
    }

    #[test]
    fn a_prompt_without_a_reply_moves_the_cursor_but_is_no_turn() {
        let rows = r#"[
          {"kind":"m","id":"u1","session":"s","parent":null,"dir":"/d","t":1000,"data":"{\"role\":\"user\"}"},
          {"kind":"m","id":"u2","session":"s","parent":null,"dir":"/d","t":2000,"data":"{\"role\":\"user\"}"}
        ]"#;
        let (events, cursors) = parse(rows, &BTreeMap::new());
        assert!(events.is_empty());
        assert_eq!(cursors["s"], 1000);
    }

    #[test]
    fn large_tool_output_and_context_go_with_the_prompt() {
        let rows = r#"[
          {"kind":"m","id":"u1","session":"s","parent":null,"dir":"/d","t":1000,"data":"{\"role\":\"user\"}"},
          {"kind":"t","id":"p1","session":"s","parent":null,"dir":null,"t":1100,"data":"read","n":8000},
          {"kind":"t","id":"p2","session":"s","parent":null,"dir":null,"t":1200,"data":"bash","n":7999},
          {"kind":"t","id":"p3","session":"s","parent":null,"dir":null,"t":1300,"data":"read","n":9001},
          {"kind":"m","id":"a1","session":"s","parent":null,"dir":"/d","t":1400,"data":"{\"role\":\"assistant\",\"parentID\":\"u1\",\"finish\":\"tool-calls\",\"tokens\":{\"input\":5,\"cache\":{\"read\":100,\"write\":0}}}"},
          {"kind":"m","id":"a2","session":"s","parent":null,"dir":"/d","t":1500,"data":"{\"role\":\"assistant\",\"parentID\":\"u1\",\"finish\":\"stop\",\"time\":{\"completed\":1600},\"tokens\":{\"input\":7,\"cache\":{\"read\":200,\"write\":30}}}"}
        ]"#;
        let got = turns(rows, &BTreeMap::new());
        assert_eq!(got.len(), 1);
        let t = &got[0].1;
        // 8000 / 4 + ceil(9001 / 4), both from `read`; 7999 characters is not large.
        assert_eq!(t.tool_output, [("read".to_string(), 2000 + 2251)].into());
        // The last request's whole input: uncached, read and written.
        assert_eq!(t.context, Some(7 + 200 + 30));
    }

    #[test]
    fn unreadable_rows_give_nothing() {
        assert_eq!(parse("", &BTreeMap::new()).0.len(), 0);
        assert_eq!(parse("not json", &BTreeMap::new()).1.len(), 0);
        let bad =
            r#"[{"kind":"m","id":"u","session":"s","parent":null,"dir":null,"t":1,"data":"{"}]"#;
        assert!(parse(bad, &BTreeMap::new()).0.is_empty());
    }
}
