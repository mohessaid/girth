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
//!
//! Copilot 1.0.105+ also writes a `session.usage_record` per request. A stretch holding
//! any is read from its records instead of its shutdown, so it is never counted both ways.
//! A turn starts at a user-initiated request, at the first request after a prompt, or
//! after a request that ended its turn (it finished for a reason other than `tool_calls`,
//! or called `task_complete`); there is one ledger turn per model. A turn cut off by the
//! next one mid-tool-call is recorded as aborted. Records after the last shutdown (a
//! running session) are imported once their turn is done, so they split into the same
//! turns as when read after the shutdown. A record's `cost` is read as premium requests,
//! like a shutdown's `requests.cost`.

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
    /// A shutdown's totals for one model.
    fn read(v: &Value) -> Usage {
        let n = |x: &Value| x.as_u64().unwrap_or(0);
        Usage {
            requests: n(&v["requests"]["count"]),
            premium: n(&v["requests"]["cost"]),
            ..Usage::tokens(&v["usage"])
        }
    }

    /// The token counts of a shutdown's `usage` or of one usage record.
    fn tokens(u: &Value) -> Usage {
        let n = |x: &Value| x.as_u64().unwrap_or(0);
        Usage {
            requests: 0,
            premium: 0,
            input: n(&u["inputTokens"]),
            output: n(&u["outputTokens"]),
            cache_read: n(&u["cacheReadTokens"]),
            cache_write: n(&u["cacheWriteTokens"]),
        }
    }

    /// Two records' requests and tokens together; premium comes from their summed `cost`.
    fn plus(&self, more: &Usage) -> Usage {
        Usage {
            requests: self.requests + more.requests,
            premium: 0,
            input: self.input + more.input,
            output: self.output + more.output,
            cache_read: self.cache_read + more.cache_read,
            cache_write: self.cache_write + more.cache_write,
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

/// How far a session file has been imported: its shutdowns, and its usage records.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cursor {
    pub shutdowns: usize,
    pub records: usize,
}

/// One `session.usage_record`.
struct Record {
    ts: u64,
    model: String,
    usage: Usage,
    cost: f64,
    /// Sent by the user, or the first request after a prompt.
    starts: bool,
    /// The last request of its turn: it finished for a reason other than a tool call, or
    /// called `task_complete`.
    stopped: bool,
}

impl Record {
    fn read(ts: u64, u: &Value, prompted: bool) -> Record {
        Record {
            ts,
            model: u["model"].as_str().unwrap_or_default().to_string(),
            usage: Usage {
                requests: 1,
                ..Usage::tokens(u)
            },
            cost: u["cost"].as_f64().unwrap_or(0.0),
            starts: prompted || u["initiator"] == "user",
            stopped: u["finishReason"]
                .as_str()
                .is_some_and(|f| f != "tool_calls")
                || u["toolCounts"]["task_complete"].is_u64(),
        }
    }
}

/// Where a session is while reading: its id and folder.
struct At<'a> {
    session: &'a Option<String>,
    cwd: &'a str,
}

/// The turns in one stretch's records (`first` is the file-wide index of `recs[0]`),
/// skipping those before record `from`. `uses` hold the record index each belongs with.
/// Returns how many of `recs` are done; with `closed`, all are.
fn from_records(
    recs: &[Record],
    uses: &mut Vec<(usize, u64, Event)>,
    first: usize,
    from: usize,
    closed: bool,
    at: &At,
    events: &mut Vec<(u64, Event)>,
) -> usize {
    let starts: Vec<usize> = (0..recs.len())
        .filter(|&i| i == 0 || recs[i].starts || recs[i - 1].stopped)
        .chain([recs.len()])
        .collect();
    let mut done = 0;
    for w in starts.windows(2) {
        let (start, end) = (w[0], w[1]);
        let last = &recs[end - 1];
        let later = end < recs.len();
        if !(closed || later || last.stopped) {
            break;
        }
        done = end;
        // The last turn of a closed stretch also takes skills used after its requests.
        let all = closed && !later;
        let (now, rest): (Vec<_>, Vec<_>) = uses.drain(..).partition(|u| u.0 < end || all);
        *uses = rest;
        if first + start < from {
            continue;
        }
        events.extend(now.into_iter().map(|(_, ts, e)| (ts, e)));
        let mut models: Vec<(&str, Usage, f64)> = Vec::new();
        for r in &recs[start..end] {
            match models.iter_mut().find(|m| m.0 == r.model) {
                Some(m) => {
                    m.1 = m.1.plus(&r.usage);
                    m.2 += r.cost;
                }
                None => models.push((&r.model, r.usage, r.cost)),
            }
        }
        for (model, mut u, cost) in models {
            u.premium = cost.round() as u64;
            let mut t = turn(model, &u, at.cwd, at.session);
            t.aborted = !last.stopped;
            events.push((last.ts, Event::Turn(t)));
        }
    }
    done
}

/// Ledger events for what `text` holds past `done`, in file order, and how far it has
/// now been read. Skills used in a stretch are recorded with its shutdown, or with their
/// turn when the stretch has records, so a stretch or turn still open is left whole.
pub fn parse(text: &str, done: Cursor) -> (Vec<(u64, Event)>, Cursor) {
    let (mut session, mut cwd) = (None::<String>, String::new());
    let mut prev = Metrics::new();
    let mut tagged = BTreeMap::<String, u64>::new();
    let (mut turns, mut shutdowns) = (0u64, 0usize);
    let mut uses = Vec::new();
    let mut recs = Vec::<Record>::new();
    let (mut seen, mut prompted) = (0usize, false);
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
            "user.message" => prompted = true,
            "assistant.turn_start" => turns += 1,
            "assistant.message" => {
                if let Some(m) = d["model"].as_str() {
                    *tagged.entry(m.to_string()).or_default() += 1;
                }
            }
            "session.usage_record" => {
                recs.push(Record::read(ts, &d["usage"], prompted));
                prompted = false;
            }
            "skill.invoked" => {
                if let Some(skill) = d["name"].as_str() {
                    // A skill typed with a prompt goes with that prompt's first request;
                    // one called mid-turn, with the request that called it.
                    let anchor = if prompted || recs.is_empty() {
                        recs.len()
                    } else {
                        recs.len() - 1
                    };
                    let event = Event::Use {
                        skill: skill.to_string(),
                        project: cwd.clone(),
                        session: session.clone(),
                        agent: false,
                        harness: Some(HARNESS.into()),
                    };
                    uses.push((anchor, ts, event));
                }
            }
            "session.shutdown" => {
                shutdowns += 1;
                let cur: Metrics = d["modelMetrics"]
                    .as_object()
                    .map(|o| o.iter().map(|(m, v)| (m.clone(), Usage::read(v))).collect())
                    .unwrap_or_default();
                let new = shutdowns > done.shutdowns;
                let at = At {
                    session: &session,
                    cwd: &cwd,
                };
                if !recs.is_empty() {
                    if new {
                        from_records(&recs, &mut uses, seen, done.records, true, &at, &mut events);
                    }
                } else {
                    if new {
                        events.extend(uses.drain(..).map(|(_, ts, e)| (ts, e)));
                    }
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
                    }
                }
                if !cur.is_empty() {
                    prev = cur;
                }
                seen += recs.len();
                recs.clear();
                uses.clear();
                tagged.clear();
                turns = 0;
            }
            _ => {}
        }
    }
    let at = At {
        session: &session,
        cwd: &cwd,
    };
    let open = from_records(
        &recs,
        &mut uses,
        seen,
        done.records,
        false,
        &at,
        &mut events,
    );
    let read = Cursor {
        shutdowns,
        records: seen + open,
    };
    (events, read)
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
        cost_micros: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str =
        include_str!("../../tests/fixtures/copilot/session-state/s1/events.jsonl");

    fn at(shutdowns: usize) -> Cursor {
        Cursor {
            shutdowns,
            records: 0,
        }
    }

    fn turns(text: &str, done: usize) -> Vec<(u64, Turn)> {
        turns_from(text, at(done))
    }

    fn turns_from(text: &str, done: Cursor) -> Vec<(u64, Turn)> {
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
        assert_eq!(parse(FIXTURE, at(0)).1, at(5));
    }

    #[test]
    fn shutdowns_already_imported_are_skipped() {
        let rows: Vec<_> = turns(FIXTURE, 3)
            .iter()
            .map(|(_, t)| t.model.clone())
            .collect();
        assert_eq!(rows, ["claude-opus-4-6"]);
        assert!(parse(FIXTURE, at(5)).0.is_empty());
    }

    #[test]
    fn skills_are_recorded_once_their_stretch_shuts_down() {
        let uses: Vec<_> = parse(FIXTURE, at(0))
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
        let later = parse(FIXTURE, at(1)).0;
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
        assert_eq!(parse("", at(0)), (vec![], at(0)));
        let torn = &shutdown(1, 1)[..40];
        assert_eq!(parse(torn, at(0)), (vec![], at(0)));
    }

    const RECORDS: &str =
        include_str!("../../tests/fixtures/copilot/session-state/s2/events.jsonl");

    fn uses(text: &str, done: Cursor) -> Vec<(String, String)> {
        parse(text, done)
            .0
            .into_iter()
            .filter_map(|(_, e)| match e {
                Event::Use { skill, project, .. } => Some((skill, project)),
                _ => None,
            })
            .collect()
    }

    fn record(ts: &str, user: bool, finish: &str) -> String {
        let initiator = if user { "user" } else { "agent" };
        format!(
            r#"{{"type":"session.usage_record","timestamp":"{ts}","data":{{"usage":{{"model":"claude-haiku-4.5","inputTokens":10,"outputTokens":1,"cacheReadTokens":0,"cacheWriteTokens":2,"cost":1.0,"initiator":"{initiator}","finishReason":"{finish}"}}}}}}"#
        )
    }

    #[test]
    fn records_replace_their_stretch_s_shutdown() {
        let got = turns_from(RECORDS, Cursor::default());
        let rows: Vec<_> = got.iter().map(|(_, t)| summary(t)).collect();
        assert_eq!(
            rows,
            [
                // No records in this stretch: its shutdown is read.
                ("claude-haiku-4-5", 1, 1, 100, 0, 2, "/work/r"),
                // A prompt and the request it set off; cache reads leave input.
                ("claude-haiku-4-5", 2, 1, 250, 250, 7, "/work/r"),
                // A prompt that used two models is one turn per model.
                ("gpt-5.4", 1, 1, 100, 0, 1, "/work/r"),
                ("claude-haiku-4-5", 1, 0, 50, 0, 1, "/work/r"),
                // Still running, but this prompt's turn is done.
                ("claude-haiku-4-5", 1, 1, 100, 300, 5, "/work/q"),
            ]
        );
        assert_eq!(got[1].0, unix_seconds("2026-10-17T10:00:09.000Z").unwrap());
        assert!(got.iter().all(|(_, t)| !t.aborted && !t.agent));
        assert_eq!(got[1].1.session.as_deref(), Some("s2"));
    }

    #[test]
    fn records_already_imported_are_skipped() {
        let (_, done) = parse(RECORDS, Cursor::default());
        assert_eq!(
            done,
            Cursor {
                shutdowns: 2,
                records: 5
            }
        );
        assert!(parse(RECORDS, done).0.is_empty());
    }

    #[test]
    fn a_stretch_imported_from_its_shutdown_is_not_read_again_from_records() {
        // A cursor from before records were read: two shutdowns, no records.
        let got = turns_from(RECORDS, at(2));
        let rows: Vec<_> = got.iter().map(|(_, t)| summary(t)).collect();
        assert_eq!(rows, [("claude-haiku-4-5", 1, 1, 100, 300, 5, "/work/q")]);
    }

    #[test]
    fn the_open_turn_lands_once_it_ends() {
        let done = parse(RECORDS, Cursor::default()).1;
        assert_eq!(
            uses(RECORDS, Cursor::default()),
            [("girth".to_string(), "/work/r".to_string())]
        );
        // Shut down mid-tool-call: the turn is done, and cut off.
        let shut = format!("{RECORDS}{}\n", shutdown(9, 9));
        let got = turns_from(&shut, done);
        let rows: Vec<_> = got.iter().map(|(_, t)| (summary(t), t.aborted)).collect();
        assert_eq!(
            rows,
            [(("claude-haiku-4-5", 1, 1, 100, 400, 6, "/work/q"), true)]
        );
        assert_eq!(
            uses(&shut, done),
            [("girth-open".to_string(), "/work/q".to_string())]
        );
        assert_eq!(
            parse(&shut, done).1,
            Cursor {
                shutdowns: 3,
                records: 6
            }
        );
        // A next prompt cuts it off too; a request that stopped ends its own turn.
        let next = [
            RECORDS.trim_end().to_string(),
            record("2026-10-18T11:02:00.000Z", false, "tool_calls"),
            record("2026-10-18T11:03:00.000Z", true, "stop"),
        ]
        .join("\n");
        let got = turns_from(&next, done);
        let rows: Vec<_> = got
            .iter()
            .map(|(_, t)| (t.requests, t.premium, t.cache_write, t.aborted))
            .collect();
        assert_eq!(
            rows,
            [(Some(2), Some(2), 2, true), (Some(1), Some(1), 2, false)]
        );
        assert_eq!(parse(&next, done).1.records, 8);
    }

    const PROMPT: &str = r#"{"type":"user.message","timestamp":"2026-10-18T11:01:30.000Z","data":{"content":"five"}}"#;

    #[test]
    fn a_turn_read_live_splits_as_it_would_once_closed() {
        let base = parse(RECORDS, Cursor::default()).1;
        let stop = |ts: &str| record(ts, false, "stop");
        let one = [
            RECORDS.trim_end().to_string(),
            stop("2026-10-18T11:02:00.000Z"),
        ]
        .join("\n");
        let two = [one.clone(), stop("2026-10-18T11:03:00.000Z")].join("\n");
        let (first, mid) = parse(&one, base);
        let later = turns_from(&two, mid);
        let at_once = turns_from(&two, base);
        let n = |got: &[(u64, Turn)]| got.iter().map(|(_, t)| t.requests).collect::<Vec<_>>();
        // Requests after one that stopped are a turn of their own, read live or not.
        assert_eq!(first.len(), 2); // the open turn's use, and the turn
        assert_eq!(n(&later), [Some(1)]);
        assert_eq!(n(&at_once), [Some(2), Some(1)]);
    }

    #[test]
    fn a_prompt_starts_a_turn_even_when_the_agent_answers_it() {
        let next = [
            RECORDS.trim_end(),
            PROMPT,
            &record("2026-10-18T11:02:00.000Z", false, "stop"),
        ]
        .join("\n");
        let done = parse(RECORDS, Cursor::default()).1;
        let rows: Vec<_> = turns_from(&next, done)
            .iter()
            .map(|(_, t)| (t.requests, t.aborted))
            .collect();
        assert_eq!(rows, [(Some(1), true), (Some(1), false)]);
    }

    #[test]
    fn task_complete_ends_a_turn() {
        let done_call = r#"{"type":"session.usage_record","timestamp":"2026-10-18T11:02:00.000Z","data":{"usage":{"model":"claude-haiku-4.5","inputTokens":10,"outputTokens":1,"cost":0.0,"initiator":"agent","finishReason":"tool_calls","toolCounts":{"task_complete":1}}}}"#;
        let next = [RECORDS.trim_end(), done_call].join("\n");
        let done = parse(RECORDS, Cursor::default()).1;
        let rows: Vec<_> = turns_from(&next, done)
            .iter()
            .map(|(_, t)| (t.requests, t.aborted))
            .collect();
        assert_eq!(rows, [(Some(2), false)]);
    }

    #[test]
    fn a_skill_called_mid_turn_belongs_to_that_turn() {
        // girth-mid ran during prompt "four"; the prompt after it is still open.
        let skill = r#"{"type":"skill.invoked","timestamp":"2026-10-18T11:01:07.000Z","data":{"name":"girth-mid"}}"#;
        let next = [
            RECORDS.trim_end(),
            skill,
            &record("2026-10-18T11:02:00.000Z", true, "tool_calls"),
        ]
        .join("\n");
        let done = parse(RECORDS, Cursor::default()).1;
        let got: Vec<_> = uses(&next, done).into_iter().map(|u| u.0).collect();
        assert_eq!(got, ["girth-open", "girth-mid"]);
    }

    #[test]
    fn a_skill_typed_with_a_prompt_belongs_to_that_prompt_s_turn() {
        // girth-open came after prompt "four" and before its first request.
        let next = format!(
            "{RECORDS}{}\n",
            record("2026-10-18T11:01:09.000Z", false, "stop")
        );
        let done = parse(RECORDS, Cursor::default()).1;
        assert_eq!(
            uses(&next, done),
            [("girth-open".to_string(), "/work/q".to_string())]
        );
    }

    #[test]
    fn only_claude_ids_are_rewritten() {
        assert_eq!(model_id("claude-haiku-4.5"), "claude-haiku-4-5");
        assert_eq!(model_id("gpt-5.4"), "gpt-5.4");
    }
}
