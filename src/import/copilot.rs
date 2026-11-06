//! GitHub Copilot CLI sessions (`~/.copilot/session-state/<id>/events.jsonl`) as ledger turns.
//!
//! Each `session.shutdown` carries per-model totals (`modelMetrics`): requests, premium
//! requests, and tokens. Girth records one turn per model per shutdown, at the shutdown's
//! time. Copilot's `inputTokens` includes cache reads and writes (unlike Anthropic's
//! `input_tokens`); the ledger's `input` is the uncached rest.
//!
//! A resumed session shuts down again, and Copilot versions differ in what that second
//! shutdown holds: a fresh count for the resumed stretch, or the session's running totals.
//! Each shutdown is read against the previous one:
//! 1. a model that went down or disappeared means a fresh count;
//! 2. a model whose requests and tokens are all unchanged means running totals (a fresh
//!    count cannot repeat the previous one to the token);
//! 3. otherwise, the reading whose requests per model best match the assistant messages
//!    tagged with that model since the last shutdown; or, with no tagged messages, the
//!    reading whose request total best matches the turns started. A tie reads as a fresh
//!    count, as most shutdowns on the author's machine are.
//!
//! On the author's 80 non-empty shutdowns, 7 fall to the turn-count tiebreak.
//! Per-request `session.usage_record` events (Copilot 1.0.105+) are not read yet.

use std::collections::BTreeMap;

use serde_json::Value;

use super::codex::unix_seconds;
use crate::ledger::{Event, Turn};

const HARNESS: &str = "copilot";

#[derive(Clone, Copy, Default, PartialEq)]
struct Usage {
    requests: u64,
    premium: u64,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
}

impl Usage {
    fn read(v: &Value) -> Usage {
        let n = |x: &Value| x.as_u64().unwrap_or(0);
        let u = &v["usage"];
        Usage {
            requests: n(&v["requests"]["count"]),
            premium: n(&v["requests"]["cost"]),
            input: n(&u["inputTokens"]),
            output: n(&u["outputTokens"]),
            cache_read: n(&u["cacheReadTokens"]),
            cache_write: n(&u["cacheWriteTokens"]),
        }
    }

    fn fields(&self) -> [u64; 6] {
        [
            self.requests,
            self.premium,
            self.input,
            self.output,
            self.cache_read,
            self.cache_write,
        ]
    }

    fn minus(&self, before: &Usage) -> Usage {
        Usage {
            requests: self.requests.saturating_sub(before.requests),
            premium: self.premium.saturating_sub(before.premium),
            input: self.input.saturating_sub(before.input),
            output: self.output.saturating_sub(before.output),
            cache_read: self.cache_read.saturating_sub(before.cache_read),
            cache_write: self.cache_write.saturating_sub(before.cache_write),
        }
    }
}

type Metrics = BTreeMap<String, Usage>;

/// Whether `cur` holds running totals that include `prev` (see the module doc).
fn carries_over(prev: &Metrics, cur: &Metrics, tagged: &BTreeMap<String, u64>, turns: u64) -> bool {
    if prev.is_empty() {
        return false;
    }
    let went_down = prev.iter().any(|(m, p)| {
        cur.get(m)
            .is_none_or(|c| c.fields().iter().zip(p.fields()).any(|(c, p)| *c < p))
    });
    if went_down {
        return false;
    }
    if cur.iter().any(|(m, c)| prev.get(m) == Some(c)) {
        return true;
    }
    let delta = |m: &String, c: &Usage| prev.get(m).map_or(*c, |p| c.minus(p)).requests;
    let off = |want: &dyn Fn(&String) -> u64, f: &dyn Fn(&String, &Usage) -> u64| -> u64 {
        cur.iter().map(|(m, c)| f(m, c).abs_diff(want(m))).sum()
    };
    if tagged.is_empty() {
        let fresh: u64 = cur.values().map(|c| c.requests).sum();
        let running: u64 = cur.iter().map(|(m, c)| delta(m, c)).sum();
        running.abs_diff(turns) < fresh.abs_diff(turns)
    } else {
        let want = |m: &String| tagged.get(m).copied().unwrap_or(0);
        off(&want, &|m, c| delta(m, c)) < off(&want, &|_, c| c.requests)
    }
}

/// Copilot names Claude models with a dot (`claude-sonnet-4.6`); Anthropic's ids use a dash.
fn model_id(name: &str) -> String {
    if name.starts_with("claude-") {
        name.replace('.', "-")
    } else {
        name.to_string()
    }
}

/// Ledger events for the shutdowns in `text` after the first `done`, in file order, and
/// how many shutdowns `text` holds. Skills used in a stretch are recorded with its
/// shutdown, so a stretch still open is left whole for the next run.
pub fn parse(text: &str, done: usize) -> (Vec<(u64, Event)>, usize) {
    let (mut session, mut cwd) = (None::<String>, String::new());
    let mut prev = Metrics::new();
    let mut tagged = BTreeMap::<String, u64>::new();
    let (mut turns, mut shutdowns) = (0u64, 0usize);
    let mut uses = Vec::new();
    let mut events = Vec::new();
    for line in text.lines() {
        let Ok(e) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let d = &e["data"];
        let ts = e["timestamp"].as_str().and_then(unix_seconds).unwrap_or(0);
        match e["type"].as_str().unwrap_or_default() {
            kind @ ("session.start" | "session.resume") => {
                if kind == "session.start" {
                    session = d["sessionId"].as_str().map(str::to_string);
                }
                if let Some(c) = d["context"]["cwd"].as_str() {
                    cwd = c.to_string();
                }
            }
            "assistant.turn_start" => turns += 1,
            "assistant.message" => {
                if let Some(m) = d["model"].as_str() {
                    *tagged.entry(m.to_string()).or_default() += 1;
                }
            }
            "skill.invoked" => {
                if let Some(skill) = d["name"].as_str() {
                    uses.push((
                        ts,
                        Event::Use {
                            skill: skill.to_string(),
                            project: cwd.clone(),
                            session: session.clone(),
                            agent: false,
                            harness: Some(HARNESS.into()),
                        },
                    ));
                }
            }
            "session.shutdown" => {
                shutdowns += 1;
                let cur: Metrics = d["modelMetrics"]
                    .as_object()
                    .map(|o| o.iter().map(|(m, v)| (m.clone(), Usage::read(v))).collect())
                    .unwrap_or_default();
                let new = shutdowns > done;
                if new {
                    events.append(&mut uses);
                }
                uses.clear();
                if !cur.is_empty() {
                    let running = carries_over(&prev, &cur, &tagged, turns);
                    for (m, c) in cur.iter().filter(|_| new) {
                        let u = match prev.get(m) {
                            Some(p) if running => c.minus(p),
                            _ => *c,
                        };
                        if u.requests > 0 {
                            events.push((ts, Event::Turn(turn(m, &u, &cwd, &session))));
                        }
                    }
                    prev = cur;
                }
                tagged.clear();
                turns = 0;
            }
            _ => {}
        }
    }
    (events, shutdowns)
}

fn turn(model: &str, u: &Usage, cwd: &str, session: &Option<String>) -> Turn {
    Turn {
        project: cwd.to_string(),
        session: session.clone(),
        model: model_id(model),
        input: u.input.saturating_sub(u.cache_read + u.cache_write),
        output: u.output,
        cache_read: u.cache_read,
        cache_write: u.cache_write,
        agent: false,
        aborted: false,
        lens_before: None,
        lens_after: None,
        requests: Some(u.requests),
        mcp: vec![],
        breaks: vec![],
        outcomes: vec![],
        harness: Some(HARNESS.into()),
        premium: Some(u.premium),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str =
        include_str!("../../tests/fixtures/copilot/session-state/s1/events.jsonl");

    fn turns(text: &str, done: usize) -> Vec<(u64, Turn)> {
        parse(text, done)
            .0
            .into_iter()
            .filter_map(|(ts, e)| match e {
                Event::Turn(t) => Some((ts, t)),
                _ => None,
            })
            .collect()
    }

    fn summary(t: &Turn) -> (&str, u64, u64, u64, u64, u64, &str) {
        (
            t.model.as_str(),
            t.requests.unwrap(),
            t.premium.unwrap(),
            t.input,
            t.cache_read,
            t.output,
            t.project.as_str(),
        )
    }

    #[test]
    fn each_shutdown_is_one_turn_per_model() {
        let got = turns(FIXTURE, 0);
        let rows: Vec<_> = got.iter().map(|(_, t)| summary(t)).collect();
        assert_eq!(
            rows,
            [
                ("claude-sonnet-4-6", 2, 1, 300, 600, 10, "/work/a"),
                // A fresh count: Sonnet went up, and 3 Sonnet messages back it.
                ("claude-sonnet-4-6", 3, 2, 900, 2000, 30, "/work/b"),
                ("gpt-5.4", 1, 1, 500, 0, 5, "/work/b"),
                // Running totals: Sonnet and gpt are unchanged, so only deepseek is new.
                ("deepseek-v4-pro", 1, 0, 300, 100, 4, "/work/b"),
                // A fresh count again: earlier models disappeared.
                ("claude-opus-4-6", 1, 3, 600, 0, 8, "/work/c"),
            ]
        );
        let (ts, first) = &got[0];
        assert_eq!(*ts, unix_seconds("2026-05-07T15:00:00.000Z").unwrap());
        assert_eq!(
            (
                first.session.as_deref(),
                first.cache_write,
                first.harness.as_deref()
            ),
            (Some("s1"), 100, Some("copilot"))
        );
        assert!(!first.aborted && !first.agent);
    }

    #[test]
    fn counts_every_shutdown_including_empty_ones() {
        assert_eq!(parse(FIXTURE, 0).1, 5);
    }

    #[test]
    fn shutdowns_already_imported_are_skipped() {
        let rows: Vec<_> = turns(FIXTURE, 3)
            .iter()
            .map(|(_, t)| t.model.clone())
            .collect();
        assert_eq!(rows, ["claude-opus-4-6"]);
        assert!(parse(FIXTURE, 5).0.is_empty());
    }

    #[test]
    fn skills_are_recorded_once_their_stretch_shuts_down() {
        let uses: Vec<_> = parse(FIXTURE, 0)
            .0
            .into_iter()
            .filter_map(|(_, e)| match e {
                Event::Use {
                    skill,
                    project,
                    harness,
                    ..
                } => Some((skill, project, harness)),
                _ => None,
            })
            .collect();
        // girth-hyperframes ran in the open stretch, so it waits for the next run.
        assert_eq!(
            uses,
            [("girth".into(), "/work/a".into(), Some("copilot".into()))]
        );
        let later = parse(FIXTURE, 1).0;
        assert!(later.iter().all(|(_, e)| !matches!(e, Event::Use { .. })));
    }

    fn shutdown(count: u64, input: u64) -> String {
        format!(
            r#"{{"type":"session.shutdown","timestamp":"2026-05-01T00:00:00.000Z","data":{{"modelMetrics":{{"claude-haiku-4.5":{{"requests":{{"count":{count},"cost":0}},"usage":{{"inputTokens":{input},"outputTokens":1,"cacheReadTokens":0,"cacheWriteTokens":0}}}}}}}}}}"#
        )
    }

    const TURN: &str =
        r#"{"type":"assistant.turn_start","timestamp":"2026-05-01T00:00:00.000Z","data":{}}"#;

    #[test]
    fn without_tagged_messages_turns_started_break_the_tie() {
        // 5 requests, then 2 turns and a shutdown reading 7: running totals (2 new).
        let running = [shutdown(5, 50), TURN.into(), TURN.into(), shutdown(7, 70)].join("\n");
        let got = turns(&running, 1);
        assert_eq!((got[0].1.requests, got[0].1.input), (Some(2), 20));
        // 6 turns and a shutdown reading 7: a fresh count.
        let fresh = [
            vec![shutdown(5, 50)],
            vec![TURN.to_string(); 6],
            vec![shutdown(7, 70)],
        ]
        .concat()
        .join("\n");
        assert_eq!(turns(&fresh, 1)[0].1.requests, Some(7));
    }

    fn tagged(model: &str) -> String {
        format!(
            r#"{{"type":"assistant.message","timestamp":"2026-05-01T00:00:00.000Z","data":{{"model":"{model}"}}}}"#
        )
    }

    fn lines(parts: &[(&str, usize)]) -> String {
        parts
            .iter()
            .flat_map(|(l, n)| std::iter::repeat_n(l.to_string(), *n))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_model_whose_count_went_down_is_a_fresh_count() {
        // No turns logged: the tiebreak alone would read 2 under 5 as running totals.
        let text = [shutdown(5, 50), shutdown(2, 20)].join("\n");
        assert_eq!(turns(&text, 1)[0].1.requests, Some(2));
    }

    #[test]
    fn tagged_messages_outrank_turns_started() {
        // 7 turns say fresh (7); 2 tagged Haiku messages say running totals (2 new).
        let (first, last) = (shutdown(5, 50), shutdown(7, 70));
        let haiku = tagged("claude-haiku-4.5");
        let text = lines(&[(&first, 1), (TURN, 7), (&haiku, 2), (&last, 1)]);
        assert_eq!(turns(&text, 1)[0].1.requests, Some(2));
    }

    #[test]
    fn a_tie_reads_as_a_fresh_count() {
        // 4 then 7: fresh says 7, running says 3; 5 turns or 5 tags sit exactly between.
        let (first, last) = (shutdown(4, 40), shutdown(7, 70));
        let by_turns = lines(&[(&first, 1), (TURN, 5), (&last, 1)]);
        assert_eq!(turns(&by_turns, 1)[0].1.requests, Some(7));
        let haiku = tagged("claude-haiku-4.5");
        let by_tags = lines(&[(&first, 1), (&haiku, 5), (&last, 1)]);
        assert_eq!(turns(&by_tags, 1)[0].1.requests, Some(7));
        // One tag fewer tips it to running totals.
        let four = lines(&[(&first, 1), (&haiku, 4), (&last, 1)]);
        assert_eq!(turns(&four, 1)[0].1.requests, Some(3));
    }

    #[test]
    fn torn_and_empty_text_give_nothing() {
        assert_eq!(parse("", 0), (vec![], 0));
        let torn = &shutdown(1, 1)[..40];
        assert_eq!(parse(torn, 0), (vec![], 0));
    }

    #[test]
    fn only_claude_ids_are_rewritten() {
        assert_eq!(model_id("claude-haiku-4.5"), "claude-haiku-4-5");
        assert_eq!(model_id("gpt-5.4"), "gpt-5.4");
    }
}
