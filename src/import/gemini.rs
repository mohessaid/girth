//! Gemini CLI sessions (`~/.gemini/tmp/<project>/chats/session-*.json` and `.jsonl`) as
//! ledger turns.
//!
//! Older sessions are one JSON object with a `messages` list, rewritten as they grow.
//! Newer ones are JSON lines: a header, then messages, where a message written again
//! under the same `id` replaces the earlier copy, and `{"$set": {"messages": [...]}}`
//! replaces the whole list. A turn is one user prompt and the `gemini` replies after it;
//! each reply with `tokens` is one request.
//!
//! Gemini's `tokens.input` includes `cached`; `thoughts` are billed as output and `tool`
//! (the tool-use prompt) as input, and none of the three is in `input` or `output`.
//! A turn is done when a later prompt exists, or when its last reply made no tool call.
//! A turn cut off mid-tool-call by the next prompt is recorded as aborted. A session
//! left mid-tool-call keeps its last turn open. Every turn is a main-loop turn: the
//! sessions seen so far carry `kind: "main"` or no kind at all.

use serde_json::Value;

use crate::ledger::{Event, Turn};

const HARNESS: &str = "gemini";

/// Milliseconds since the epoch for an RFC 3339 UTC timestamp, keeping its fraction.
fn millis(s: &str) -> Option<u64> {
    let secs = super::codex::unix_seconds(s)?;
    let frac = s
        .get(19..)
        .and_then(|r| r.strip_prefix('.'))
        .map(|r| {
            r.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
        })
        .unwrap_or_default();
    let ms = format!("{frac:0<3}").get(..3)?.parse::<u64>().ok()?;
    Some(secs * 1000 + ms)
}

/// The session id and messages, in order, of a `.json` or `.jsonl` session file.
fn messages(text: &str) -> (Option<String>, Vec<Value>) {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        if v.is_object() {
            let list = v["messages"].as_array().cloned().unwrap_or_default();
            return (v["sessionId"].as_str().map(str::to_string), list);
        }
    }
    let (mut session, mut list) = (None, Vec::<Value>::new());
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(id) = v["sessionId"].as_str() {
            session.get_or_insert_with(|| id.to_string());
        }
        if let Some(all) = v["$set"]["messages"].as_array() {
            list = all.clone();
        } else if let (Some(id), true) = (v["id"].as_str(), v["type"].is_string()) {
            match list.iter_mut().find(|m| m["id"].as_str() == Some(id)) {
                Some(m) => *m = v,
                None => list.push(v),
            }
        }
    }
    (session, list)
}

/// Ledger events for the done turns in one session file newer than `cursor` (the time of
/// the last imported prompt, in milliseconds), oldest first, and the new cursor.
pub fn parse(text: &str, project: &str, cursor: u64) -> (Vec<(u64, Event)>, u64) {
    let (session, list) = messages(text);
    let mut prompts: Vec<(u64, Vec<&Value>)> = Vec::new();
    for m in &list {
        match m["type"].as_str() {
            Some("user") => {
                let at = m["timestamp"].as_str().and_then(millis).unwrap_or(0);
                prompts.push((at, vec![]));
            }
            Some("gemini") => {
                if let Some((_, replies)) = prompts.last_mut() {
                    replies.push(m);
                }
            }
            _ => {}
        }
    }
    let (mut events, mut kept) = (Vec::new(), cursor);
    for (i, (at, replies)) in prompts.iter().enumerate() {
        if *at <= cursor {
            continue;
        }
        let calling = replies
            .last()
            .is_some_and(|r| r["toolCalls"].as_array().is_some_and(|c| !c.is_empty()));
        let later = i + 1 < prompts.len();
        if calling && !later {
            break;
        }
        kept = *at;
        let requests: Vec<&Value> = replies
            .iter()
            .copied()
            .filter(|r| r["tokens"].is_object())
            .collect();
        let Some(last) = requests.last() else {
            continue;
        };
        let ts = last["timestamp"].as_str().and_then(millis).unwrap_or(*at) / 1000;
        for r in replies {
            for call in r["toolCalls"].as_array().into_iter().flatten() {
                if call["name"] == "activate_skill" {
                    if let Some(skill) = call["args"]["name"].as_str() {
                        let when = r["timestamp"].as_str().and_then(millis).unwrap_or(*at);
                        events.push((when / 1000, use_event(skill, project, &session)));
                    }
                }
            }
        }
        let mut t = turn(&requests, project, &session);
        t.aborted = calling;
        events.push((ts, Event::Turn(t)));
    }
    (events, kept)
}

fn turn(requests: &[&Value], project: &str, session: &Option<String>) -> Turn {
    let sum = |k: &str| {
        requests
            .iter()
            .map(|r| r["tokens"][k].as_u64().unwrap_or(0))
            .sum::<u64>()
    };
    let cached = sum("cached");
    Turn {
        project: project.to_string(),
        session: session.clone(),
        model: requests
            .last()
            .and_then(|r| r["model"].as_str())
            .unwrap_or_default()
            .to_string(),
        input: sum("input").saturating_sub(cached) + sum("tool"),
        output: sum("output") + sum("thoughts"),
        cache_read: cached,
        cache_write: 0,
        agent: false,
        aborted: false,
        lens_before: None,
        lens_after: None,
        requests: Some(requests.len() as u64),
        mcp: vec![],
        breaks: vec![],
        outcomes: vec![],
        harness: Some(HARNESS.into()),
        premium: None,
        cost_micros: None,
        context: None,
        tool_output: Default::default(),
    }
}

fn use_event(skill: &str, project: &str, session: &Option<String>) -> Event {
    Event::Use {
        skill: skill.to_string(),
        project: project.to_string(),
        session: session.clone(),
        agent: false,
        harness: Some(HARNESS.into()),
    }
}

/// Session files to read under `<gemini dir>/tmp`, each with its project's folder.
pub fn sessions(gemini_dir: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
    let mut out = Vec::new();
    let projects = std::fs::read_dir(gemini_dir.join("tmp"))
        .into_iter()
        .flatten()
        .flatten();
    for project in projects.map(|e| e.path()) {
        let root = std::fs::read_to_string(project.join(".project_root")).unwrap_or_default();
        let chats = std::fs::read_dir(project.join("chats"))
            .into_iter()
            .flatten()
            .flatten();
        for file in chats.map(|e| e.path()) {
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name.starts_with("session-") && (name.ends_with(".json") || name.ends_with(".jsonl"))
            {
                out.push((file, root.trim().to_string()));
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = include_str!(
        "../../tests/fixtures/gemini/.gemini/tmp/proj/chats/session-2026-05-06T17-00-ga.json"
    );
    const NEW: &str = include_str!(
        "../../tests/fixtures/gemini/.gemini/tmp/proj/chats/session-2026-05-06T17-01-gb.jsonl"
    );

    fn turns(text: &str, cursor: u64) -> Vec<Turn> {
        parse(text, "/work/g", cursor)
            .0
            .into_iter()
            .filter_map(|(_, e)| match e {
                Event::Turn(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    fn row(t: &Turn) -> (&str, u64, u64, u64, u64, bool) {
        (
            t.model.as_str(),
            t.requests.unwrap(),
            t.input,
            t.cache_read,
            t.output,
            t.aborted,
        )
    }

    #[test]
    fn a_whole_session_file_gives_its_done_turns() {
        let (events, _) = parse(OLD, "/work/g", 0);
        let ts = millis("2026-05-06T17:00:09.000Z").unwrap() / 1000;
        assert!(matches!(events.last(), Some((t, Event::Turn(_))) if *t == ts));
        let got = turns(OLD, 0);
        let rows: Vec<_> = got.iter().map(row).collect();
        // (1000 - 400 + 20) + (1200 - 1000) input, thoughts as output; the open turn waits.
        assert_eq!(rows, [("gemini-2.5-flash", 2, 820, 1400, 45, false)]);
        assert_eq!(
            (
                got[0].project.as_str(),
                got[0].session.as_deref(),
                got[0].harness.as_deref()
            ),
            ("/work/g", Some("ga"), Some("gemini"))
        );
    }

    #[test]
    fn a_jsonl_session_keeps_each_message_s_last_copy() {
        let got = turns(NEW, 0);
        let rows: Vec<_> = got.iter().map(row).collect();
        assert_eq!(
            rows,
            [
                ("gemini-3-pro-preview", 1, 500, 0, 7, false),
                // Cut off mid-tool-call by the next prompt.
                ("gemini-3-flash-preview", 1, 300, 500, 6, true),
                ("gemini-3-flash-preview", 1, 100, 800, 6, false),
            ]
        );
    }

    #[test]
    fn a_set_of_messages_replaces_the_list() {
        let reset = format!(
            "{NEW}{}\n",
            r#"{"$set":{"messages":[{"id":"v1","timestamp":"2026-05-06T17:01:41.000Z","type":"user"},{"id":"h1","timestamp":"2026-05-06T17:01:44.000Z","type":"gemini","tokens":{"input":9,"output":1,"cached":0,"thoughts":0,"tool":0},"model":"gemini-2.5-pro"}]}}"#
        );
        let rows: Vec<_> = turns(&reset, 0)
            .iter()
            .map(|t| (t.model.clone(), t.input))
            .collect();
        assert_eq!(rows, [("gemini-2.5-pro".to_string(), 9)]);
    }

    #[test]
    fn skills_activated_come_with_their_turn() {
        let uses: Vec<_> = parse(OLD, "/work/g", 0)
            .0
            .into_iter()
            .filter_map(|(ts, e)| match e {
                Event::Use { skill, project, .. } => Some((ts, skill, project)),
                _ => None,
            })
            .collect();
        let at = millis("2026-05-06T17:00:05.000Z").unwrap() / 1000;
        assert_eq!(uses, [(at, "girth".to_string(), "/work/g".to_string())]);
    }

    #[test]
    fn the_cursor_skips_imported_prompts() {
        let (_, cursor) = parse(NEW, "/work/g", 0);
        assert_eq!(cursor, millis("2026-05-06T17:02:00.000Z").unwrap());
        assert!(parse(NEW, "/work/g", cursor).0.is_empty());
        let from_v2 = millis("2026-05-06T17:01:41.000Z").unwrap();
        assert_eq!(turns(NEW, from_v2).len(), 2);
        // The open turn leaves the cursor at the last done prompt.
        assert_eq!(
            parse(OLD, "/work/g", 0).1,
            millis("2026-05-06T17:00:00.000Z").unwrap()
        );
    }

    #[test]
    fn timestamps_keep_their_milliseconds() {
        assert_eq!(millis("1970-01-01T00:00:01.5Z"), Some(1500));
        assert_eq!(millis("1970-01-01T00:00:01.123456Z"), Some(1123));
        assert_eq!(millis("1970-01-01T00:00:02Z"), Some(2000));
        assert_eq!(millis("nope"), None);
    }

    #[test]
    fn unreadable_text_gives_nothing() {
        assert_eq!(parse("", "/p", 0), (vec![], 0));
        assert_eq!(parse("{\"messages\":5}", "/p", 0), (vec![], 0));
    }

    #[test]
    fn session_files_are_found_with_their_project() {
        let home =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gemini/.gemini");
        let found: Vec<_> = sessions(&home)
            .into_iter()
            .map(|(p, root)| (p.file_name().unwrap().to_string_lossy().into_owned(), root))
            .collect();
        assert_eq!(
            found,
            [
                (
                    "session-2026-05-06T17-00-ga.json".to_string(),
                    "/work/g".to_string()
                ),
                (
                    "session-2026-05-06T17-01-gb.jsonl".to_string(),
                    "/work/g".to_string()
                ),
            ]
        );
    }
}
