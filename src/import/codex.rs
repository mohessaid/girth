//! Codex rollouts (`~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`) as ledger turns.

use serde_json::Value;

use std::collections::BTreeMap;

use crate::ledger::{Event, LARGE_OUTPUT_CHARS, Turn};

/// Ledger events for the completed turns in `text`, in file order, and the byte
/// offset just past the last one's end line. An open turn is left for the next run.
///
/// Codex's `input_tokens` includes cached and cache-write tokens; the ledger's
/// `input` is the uncached rest. A line cut short (Codex killed mid-write) is skipped.
/// Codex runs one turn at a time, so a turn still open when a later one ends was
/// cut off: it is recorded as aborted, at the time of its last request.
///
/// A tool result of `LARGE_OUTPUT_CHARS` or more goes with the turn its log line names
/// (or the open turn) as `tool_output`, keyed by the tool's name. The log holds what
/// the model was sent, after Codex's own truncation. `context` is the input of the
/// turn's last request, cached part included.
pub fn parse(text: &str) -> (Vec<(u64, Event)>, usize) {
    let mut session: Option<String> = None;
    let mut session_cwd = String::new();
    // Open turns in order: id, turn, time of its last request.
    let mut open: Vec<(String, Turn, u64)> = Vec::new();
    // Tool names by call id.
    let mut tools: BTreeMap<String, String> = BTreeMap::new();
    let (mut events, mut offset, mut pos) = (Vec::new(), 0, 0);
    for line in text.split_inclusive('\n') {
        pos += line.len();
        let Ok(e) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let p = &e["payload"];
        let turn_id = p["turn_id"].as_str().unwrap_or_default().to_string();
        match (e["type"].as_str(), p["type"].as_str()) {
            (Some("session_meta"), _) => {
                session = p["session_id"].as_str().map(str::to_string);
                session_cwd = p["cwd"].as_str().unwrap_or_default().to_string();
            }
            (Some("turn_context"), _) => {
                let t = &mut entry(&mut open, turn_id, &session).1;
                t.model = p["model"].as_str().unwrap_or(&t.model).to_string();
                if let Some(cwd) = p["cwd"].as_str() {
                    t.project = cwd.to_string();
                }
            }
            (Some("token_usage_record"), _) => {
                let u = &p["usage"];
                let n = |k: &str| u[k].as_u64().unwrap_or(0);
                let (cached, write) = (n("cached_input_tokens"), n("cache_write_input_tokens"));
                let ts = e["timestamp"].as_str().and_then(unix_seconds);
                let t = entry(&mut open, turn_id, &session);
                if let Some(ts) = ts {
                    t.2 = ts;
                }
                let t = &mut t.1;
                t.input += n("input_tokens").saturating_sub(cached + write);
                t.cache_read += cached;
                t.cache_write += write;
                t.output += n("output_tokens");
                t.requests = Some(t.requests.unwrap_or(0) + 1);
                t.context = Some(n("input_tokens"));
            }
            (Some("response_item"), Some(kind)) if kind.ends_with("_call") => {
                if let (Some(id), Some(name)) = (p["call_id"].as_str(), p["name"].as_str()) {
                    tools.insert(id.to_string(), name.to_string());
                }
            }
            (Some("response_item"), Some(kind)) if kind.ends_with("_call_output") => {
                let chars = output_chars(&p["output"]);
                if chars < LARGE_OUTPUT_CHARS {
                    continue;
                }
                let id = p["call_id"].as_str().unwrap_or_default();
                let Some(name) = tools.remove(id) else {
                    continue;
                };
                let named = p["internal_chat_message_metadata_passthrough"]["turn_id"].as_str();
                let turn = match named {
                    Some(n) => open.iter_mut().find(|(o, ..)| o == n),
                    None => open.last_mut(),
                };
                if let Some((_, t, _)) = turn {
                    *t.tool_output.entry(name).or_default() += chars.div_ceil(4) as u64;
                }
            }
            (Some("event_msg"), Some(kind @ ("task_complete" | "turn_aborted"))) => {
                let end = e["timestamp"].as_str().and_then(unix_seconds).unwrap_or(0);
                let Some(i) = open.iter().position(|(id, ..)| *id == turn_id) else {
                    continue;
                };
                for (id, mut t, last) in open.drain(..=i) {
                    let ended = id == turn_id;
                    t.aborted = !ended || kind == "turn_aborted";
                    if t.project.is_empty() {
                        t.project = session_cwd.clone();
                    }
                    if t.requests.is_some() {
                        events.push((if ended { end } else { last }, Event::Turn(t)));
                    }
                }
                offset = pos;
            }
            _ => {}
        }
    }
    (events, offset)
}

/// Characters in a tool result: a string, or a list of text parts.
fn output_chars(output: &Value) -> usize {
    match output {
        Value::String(s) => s.chars().count(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .map(|s| s.chars().count())
            .sum(),
        _ => 0,
    }
}

fn entry<'a>(
    open: &'a mut Vec<(String, Turn, u64)>,
    id: String,
    session: &Option<String>,
) -> &'a mut (String, Turn, u64) {
    let i = match open.iter().position(|(o, ..)| *o == id) {
        Some(i) => i,
        None => {
            open.push((id, new_turn(session), 0));
            open.len() - 1
        }
    };
    &mut open[i]
}

fn new_turn(session: &Option<String>) -> Turn {
    Turn {
        project: String::new(),
        session: session.clone(),
        model: String::new(),
        input: 0,
        output: 0,
        cache_read: 0,
        cache_write: 0,
        agent: false,
        aborted: false,
        lens_before: None,
        lens_after: None,
        requests: None,
        mcp: vec![],
        breaks: vec![],
        outcomes: vec![],
        harness: Some("codex".into()),
        premium: None,
        cost_micros: None,
        context: None,
        tool_output: Default::default(),
    }
}

/// Unix seconds for an RFC 3339 UTC timestamp such as `2026-10-25T10:30:00.000Z`.
pub(crate) fn unix_seconds(s: &str) -> Option<u64> {
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // Days from 1970-01-01 (Howard Hinnant's days_from_civil).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::Turn;

    const FIXTURE: &str = include_str!(
        "../../tests/fixtures/codex/sessions/2026/10/25/rollout-2026-10-25T11-20-54-s1.jsonl"
    );

    fn turns(text: &str) -> Vec<(u64, Turn)> {
        parse(text)
            .0
            .into_iter()
            .filter_map(|(ts, e)| match e {
                Event::Turn(t) => Some((ts, t)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn parses_completed_turns() {
        let t = turns(FIXTURE);
        assert_eq!(t.len(), 3);
        let (ts, first) = &t[0];
        assert_eq!(*ts, 1_792_924_200); // 2026-10-25T10:30:00Z, the end event
        assert_eq!(first.model, "gpt-6-astra");
        assert_eq!(first.project, "/work/app");
        assert_eq!(first.session.as_deref(), Some("s1"));
        assert_eq!(first.harness.as_deref(), Some("codex"));
        assert_eq!(
            (
                first.input,
                first.cache_read,
                first.cache_write,
                first.output
            ),
            (16971 - 6912, 6912, 0, 273)
        );
        assert_eq!(first.requests, Some(1));
        assert!(!first.aborted && !first.agent);
        // Two requests summed; cwd falls back to the session's.
        let second = &t[1].1;
        assert_eq!(second.model, "gpt-6.1-sol");
        assert_eq!(second.project, "/work/app");
        assert_eq!(
            (
                second.input,
                second.cache_read,
                second.cache_write,
                second.output
            ),
            (4000 + 1000, 35000, 1000, 800)
        );
        assert_eq!(second.requests, Some(2));
        assert!(t[2].1.aborted);
    }

    #[test]
    fn open_turn_is_left_for_next_run() {
        let (_, offset) = parse(FIXTURE);
        let end = FIXTURE.find(r#""type":"turn_aborted""#).unwrap();
        assert_eq!(offset, end + FIXTURE[end..].find('\n').unwrap() + 1);
        let rest = format!(
            "{}{}\n",
            &FIXTURE[offset..],
            r#"{"timestamp":"2026-10-25T10:51:00.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"t4"}}"#
        );
        // The open turn's context and usage are in `rest`; the session's cwd is not.
        let more = turns(&rest);
        assert_eq!(more.len(), 1);
        assert_eq!(more[0].1.input, 1000);
    }

    #[test]
    fn a_turn_that_never_ended_closes_when_a_later_one_does() {
        let text = concat!(
            r#"{"timestamp":"2026-10-25T10:00:00Z","type":"turn_context","payload":{"turn_id":"a","cwd":"/w","model":"gpt-6-sol"}}"#,
            "\n",
            r#"{"timestamp":"2026-10-25T10:00:05Z","type":"token_usage_record","payload":{"turn_id":"a","usage":{"input_tokens":10,"output_tokens":1}}}"#,
            "\n",
            r#"{"timestamp":"2026-10-25T10:05:00Z","type":"turn_context","payload":{"turn_id":"b","cwd":"/w","model":"gpt-6-sol"}}"#,
            "\n",
            r#"{"timestamp":"2026-10-25T10:05:05Z","type":"token_usage_record","payload":{"turn_id":"b","usage":{"input_tokens":20,"output_tokens":2}}}"#,
            "\n",
            r#"{"timestamp":"2026-10-25T10:05:09Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"b"}}"#,
            "\n",
        );
        let (events, offset) = parse(text);
        assert_eq!(offset, text.len());
        let t = turns(text);
        assert_eq!(t.len(), 2);
        assert_eq!(
            (t[0].0, t[0].1.input, t[0].1.aborted),
            (1_792_922_405, 10, true)
        );
        assert_eq!((t[1].1.input, t[1].1.aborted), (20, false));
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn large_tool_output_and_context_go_with_their_turn() {
        let meta = |turn: &str| {
            format!(r#""internal_chat_message_metadata_passthrough":{{"turn_id":"{turn}"}}"#)
        };
        let call = |kind: &str, id: &str, name: &str| {
            format!(
                r#"{{"timestamp":"2026-10-25T10:00:01Z","type":"response_item","payload":{{"type":"{kind}","call_id":"{id}","name":"{name}"}}}}"#
            )
        };
        let lines = [
            r#"{"timestamp":"2026-10-25T10:00:00Z","type":"turn_context","payload":{"turn_id":"a","cwd":"/w","model":"gpt-6-sol"}}"#.to_string(),
            call("custom_tool_call", "c1", "exec"),
            // The current form: a list of text parts (exactly the threshold), named by turn.
            format!(
                r#"{{"timestamp":"2026-10-25T10:00:02Z","type":"response_item","payload":{{"type":"custom_tool_call_output","call_id":"c1","output":[{{"type":"input_text","text":"{}"}},{{"type":"input_text","text":"x"}}],{}}}}}"#,
                "x".repeat(7999),
                meta("a")
            ),
            call("function_call", "c2", "shell"),
            // Just under the threshold: not large.
            format!(
                r#"{{"timestamp":"2026-10-25T10:00:03Z","type":"response_item","payload":{{"type":"function_call_output","call_id":"c2","output":"{}"}}}}"#,
                "y".repeat(7999)
            ),
            call("function_call", "c3", "shell"),
            // The older string form, with no turn named: the open turn's.
            format!(
                r#"{{"timestamp":"2026-10-25T10:00:04Z","type":"response_item","payload":{{"type":"function_call_output","call_id":"c3","output":"{}"}}}}"#,
                "z".repeat(9001)
            ),
            r#"{"timestamp":"2026-10-25T10:00:05Z","type":"token_usage_record","payload":{"turn_id":"a","usage":{"input_tokens":100,"output_tokens":1}}}"#.to_string(),
            r#"{"timestamp":"2026-10-25T10:00:06Z","type":"token_usage_record","payload":{"turn_id":"a","usage":{"input_tokens":300,"cached_input_tokens":100,"output_tokens":1}}}"#.to_string(),
            r#"{"timestamp":"2026-10-25T10:00:07Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"a"}}"#.to_string(),
            // An output with no open turn is dropped.
            call("function_call", "c4", "shell"),
            format!(
                r#"{{"timestamp":"2026-10-25T10:00:08Z","type":"response_item","payload":{{"type":"function_call_output","call_id":"c4","output":"{}"}}}}"#,
                "w".repeat(9000)
            ),
        ];
        let text = lines.join("\n") + "\n";
        let t = turns(&text);
        assert_eq!(t.len(), 1);
        let a = &t[0].1;
        // 8000 / 4 and ceil(9001 / 4) estimated tokens.
        let want: std::collections::BTreeMap<String, u64> =
            [("exec".to_string(), 2000), ("shell".to_string(), 2251)].into();
        assert_eq!(a.tool_output, want);
        // The last request's whole input, cached part included.
        assert_eq!(a.context, Some(300));
    }

    #[test]
    fn torn_line_is_skipped() {
        let torn = format!("{FIXTURE}{{\"timestamp\":");
        assert_eq!(parse(&torn), parse(FIXTURE));
    }

    #[test]
    fn timestamps_are_unix_seconds() {
        assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            unix_seconds("2026-10-25T10:30:00.000Z"),
            Some(1_792_924_200)
        );
        assert_eq!(unix_seconds("2024-02-29T23:59:59Z"), Some(1_709_251_199));
        assert_eq!(unix_seconds("2026-03-01T00:00:00Z"), Some(1_772_323_200));
        assert_eq!(unix_seconds("garbage"), None);
    }

    #[test]
    fn empty_text_gives_nothing() {
        assert_eq!(parse(""), (vec![], 0));
    }
}
