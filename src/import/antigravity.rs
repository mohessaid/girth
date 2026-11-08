//! Antigravity CLI conversations (`~/.gemini/antigravity-cli/conversations/<uuid>.db`)
//! as ledger turns, read through the `sqlite3` command so Girth needs no SQLite code.
//!
//! Each database holds protobuf blobs. The fields below were read off real databases
//! (agy 1.5.2) and checked against agy's own `--output-format json` usage: input, output
//! and thinking summed exactly over a conversation's generations. tokscale's parser
//! (MIT, `tokscale-core/src/sessions/antigravity_cli.rs`) found the same layout.
//! - `gen_metadata.data`, one per model call: `#1` is the chat-model message. Its `#19`
//!   is the model id, and its `#4` the usage:
//!   - `#2`: uncached input
//!   - `#5`: cache reads
//!   - `#3`: output, thinking included (else `#9 + #10`)
//!   - `#11`: the response id
//!
//!   Usage `#1`, a fixed count, is not in agy's own input total, so it is not counted.
//! - `steps.metadata`: `#1` is `{seconds, nanos}` and `#9.#11` the response id of the
//!   generation the step carries. Step type 14 is the user's prompt.
//! - `trajectory_metadata_blob.data`: `#1.#1` is the workspace URI, and `#2` the
//!   creation time.
//!
//! A turn is one prompt and the generations after it, each one request. A turn is done
//! when a later prompt exists, or when the conversation has been idle for
//! `IDLE_DONE_S`. Every turn is a main-loop turn.

use std::collections::HashMap;

use crate::ledger::{Event, Turn};

const HARNESS: &str = "antigravity";
/// A conversation untouched this long has finished its last turn.
pub const IDLE_DONE_S: u64 = 30 * 60;
const USER_PROMPT: i64 = 14;

/// The rows `sqlite3` prints for [`QUERY`]: kind (`t`, `s`, `g`), index, step type and
/// the blob as hex, separated by `|`.
pub const QUERY: &str = "SELECT 't', 0, 0, hex(data) FROM trajectory_metadata_blob \
UNION ALL SELECT 's', idx, step_type, hex(metadata) FROM steps \
UNION ALL SELECT 'g', idx, 0, hex(data) FROM gen_metadata;";

/// One conversation's rows, decoded from hex.
#[derive(Debug, Default)]
pub struct Conversation {
    pub trajectory: Vec<u8>,
    /// (index, step type, metadata)
    pub steps: Vec<(i64, i64, Vec<u8>)>,
    /// (index, data)
    pub gens: Vec<(i64, Vec<u8>)>,
}

impl Conversation {
    /// Parse `sqlite3` output for [`QUERY`]; lines that don't fit are skipped.
    pub fn from_rows(text: &str) -> Conversation {
        let mut c = Conversation::default();
        for line in text.lines() {
            let mut parts = line.splitn(4, '|');
            let (Some(kind), Some(idx), Some(kind2), Some(hex)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let (Ok(idx), Ok(step_type), Some(blob)) =
                (idx.parse::<i64>(), kind2.parse::<i64>(), unhex(hex.trim()))
            else {
                continue;
            };
            match kind {
                "t" => c.trajectory = blob,
                "s" => c.steps.push((idx, step_type, blob)),
                "g" => c.gens.push((idx, blob)),
                _ => {}
            }
        }
        c.steps.sort_by_key(|s| s.0);
        c.gens.sort_by_key(|g| g.0);
        c
    }
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// One protobuf field: its number and either a varint or a length-delimited payload.
enum Field<'a> {
    Int(u64),
    Bytes(&'a [u8]),
}

fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*i)?;
        *i += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return Some(value);
        }
    }
    None
}

/// The message's fields in order; `None` when it isn't well-formed protobuf.
fn fields(b: &[u8]) -> Option<Vec<(u64, Field<'_>)>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let key = varint(b, &mut i)?;
        let field = match key & 7 {
            0 => Field::Int(varint(b, &mut i)?),
            1 => {
                i = i.checked_add(8).filter(|&e| e <= b.len())?;
                continue;
            }
            2 => {
                let n = usize::try_from(varint(b, &mut i)?).ok()?;
                let end = i.checked_add(n).filter(|&e| e <= b.len())?;
                let payload = &b[i..end];
                i = end;
                Field::Bytes(payload)
            }
            5 => {
                i = i.checked_add(4).filter(|&e| e <= b.len())?;
                continue;
            }
            _ => return None,
        };
        out.push((key >> 3, field));
    }
    Some(out)
}

fn message(b: &[u8], number: u64) -> Option<&[u8]> {
    fields(b)?.into_iter().find_map(|(n, f)| match f {
        Field::Bytes(p) if n == number => Some(p),
        _ => None,
    })
}

fn int(b: &[u8], number: u64) -> Option<u64> {
    fields(b)?.into_iter().find_map(|(n, f)| match f {
        Field::Int(v) if n == number => Some(v),
        _ => None,
    })
}

fn text(b: &[u8], number: u64) -> Option<String> {
    message(b, number)
        .and_then(|p| std::str::from_utf8(p).ok())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// One generation's numbers.
struct Generation {
    response: Option<String>,
    model: String,
    input: u64,
    output: u64,
    cache_read: u64,
}

fn generation(data: &[u8]) -> Option<Generation> {
    let chat = message(data, 1)?;
    let usage = message(chat, 4)?;
    let output = int(usage, 3).unwrap_or_else(|| {
        int(usage, 9)
            .unwrap_or(0)
            .saturating_add(int(usage, 10).unwrap_or(0))
    });
    Some(Generation {
        response: text(usage, 11),
        model: text(chat, 19).unwrap_or_default(),
        input: int(usage, 2).unwrap_or(0),
        output,
        cache_read: int(usage, 5).unwrap_or(0),
    })
}

/// The workspace path a `file://` URI names; other URIs are kept as they are.
fn workspace(uri: &str) -> String {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let mut out = Vec::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(b) = path
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Turns done after `cursor` (turns already imported), with their times, and the new
/// cursor. `idle` says the conversation has not changed for `IDLE_DONE_S`, which closes
/// its last turn.
pub fn parse(c: &Conversation, cursor: usize, idle: bool) -> (Vec<(u64, Event)>, usize) {
    let created = message(&c.trajectory, 2)
        .and_then(|t| int(t, 1))
        .unwrap_or(0);
    let project = message(&c.trajectory, 1)
        .and_then(|w| text(w, 1))
        .map(|u| workspace(&u))
        .unwrap_or_default();
    let gens: Vec<Generation> = c.gens.iter().filter_map(|(_, d)| generation(d)).collect();
    let by_response: HashMap<String, usize> = gens
        .iter()
        .enumerate()
        .filter_map(|(i, g)| g.response.clone().map(|r| (r, i)))
        .collect();

    // (time, generations) per prompt, in step order.
    let mut turns: Vec<(u64, Vec<usize>)> = Vec::new();
    let mut placed = vec![false; gens.len()];
    for (_, step_type, meta) in &c.steps {
        let at = message(meta, 1).and_then(|t| int(t, 1));
        if *step_type == USER_PROMPT {
            turns.push((at.unwrap_or(created), Vec::new()));
            continue;
        }
        let found = message(meta, 9)
            .and_then(|u| text(u, 11))
            .and_then(|r| by_response.get(&r).copied());
        if let Some(i) = found.filter(|&i| !placed[i]) {
            placed[i] = true;
            if turns.is_empty() {
                turns.push((at.unwrap_or(created), Vec::new()));
            }
            turns.last_mut().expect("a turn").1.push(i);
        }
    }
    // Generations no step names go with the last turn.
    let loose: Vec<usize> = (0..gens.len()).filter(|&i| !placed[i]).collect();
    if !loose.is_empty() {
        if turns.is_empty() {
            turns.push((created, Vec::new()));
        }
        turns.last_mut().expect("a turn").1.extend(loose);
    }

    let done = if idle {
        turns.len()
    } else {
        turns.len().saturating_sub(1)
    };
    let mut events = Vec::new();
    for (at, idx) in turns.iter().take(done).skip(cursor) {
        if idx.is_empty() {
            continue;
        }
        let sum = |f: fn(&Generation) -> u64| idx.iter().map(|&i| f(&gens[i])).sum::<u64>();
        let last = &gens[*idx.last().expect("non-empty")];
        let turn = Turn {
            project: project.clone(),
            session: None,
            model: last.model.clone(),
            input: sum(|g| g.input),
            output: sum(|g| g.output),
            cache_read: sum(|g| g.cache_read),
            cache_write: 0,
            agent: false,
            aborted: false,
            lens_before: None,
            lens_after: None,
            requests: Some(idx.len() as u64),
            mcp: vec![],
            breaks: vec![],
            outcomes: vec![],
            harness: Some(HARNESS.into()),
            premium: None,
            cost_micros: None,
            context: Some(last.input + last.cache_read),
            tool_output: Default::default(),
        };
        events.push((*at, Event::Turn(turn)));
    }
    (events, done.max(cursor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(mut n: u64, out: &mut Vec<u8>) {
        loop {
            let b = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                out.push(b);
                return;
            }
            out.push(b | 0x80);
        }
    }
    fn int_f(n: u64, x: u64) -> Vec<u8> {
        let mut out = Vec::new();
        v(n << 3, &mut out);
        v(x, &mut out);
        out
    }
    fn msg_f(n: u64, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        v((n << 3) | 2, &mut out);
        v(payload.len() as u64, &mut out);
        out.extend_from_slice(payload);
        out
    }
    fn gen_blob(
        model: &str,
        input: u64,
        cache: Option<u64>,
        output: Option<u64>,
        resp: &str,
    ) -> Vec<u8> {
        let mut usage = int_f(1, 1318);
        usage.extend(int_f(2, input));
        if let Some(c) = cache {
            usage.extend(int_f(5, c));
        }
        match output {
            Some(o) => usage.extend(int_f(3, o)),
            None => {
                usage.extend(int_f(9, 40));
                usage.extend(int_f(10, 2));
            }
        }
        usage.extend(int_f(6, 24));
        usage.extend(msg_f(11, resp.as_bytes()));
        let mut chat = msg_f(4, &usage);
        if !model.is_empty() {
            chat.extend(msg_f(19, model.as_bytes()));
        }
        msg_f(1, &chat)
    }
    fn step_blob(secs: u64, resp: Option<&str>) -> Vec<u8> {
        let mut out = msg_f(1, &[int_f(1, secs), int_f(2, 5)].concat());
        if let Some(r) = resp {
            out.extend(msg_f(9, &[int_f(2, 1), msg_f(11, r.as_bytes())].concat()));
        }
        out
    }
    fn trajectory(uri: &str, created: u64) -> Vec<u8> {
        [
            msg_f(1, &msg_f(1, uri.as_bytes())),
            msg_f(2, &int_f(1, created)),
        ]
        .concat()
    }

    /// Two prompts: the first with two generations (a tool call between), the second one.
    fn two_turns() -> Conversation {
        Conversation {
            trajectory: trajectory("file:///work/my%20app", 1_000),
            steps: vec![
                (0, 14, step_blob(1_000, None)),
                (1, 15, step_blob(1_005, Some("r1"))),
                (2, 132, step_blob(1_006, None)),
                (3, 15, step_blob(1_010, Some("r2"))),
                (4, 14, step_blob(2_000, None)),
                (5, 15, step_blob(2_003, Some("r3"))),
            ],
            gens: vec![
                (
                    0,
                    gen_blob("gemini-3.8-flash-n", 8_303, None, Some(270), "r1"),
                ),
                (
                    1,
                    gen_blob("gemini-3.8-flash-n", 8_666, Some(30_000), Some(36), "r2"),
                ),
                (2, gen_blob("", 9_217, Some(40_000), None, "r3")),
            ],
        }
    }

    fn turns(events: &[(u64, Event)]) -> Vec<(u64, &Turn)> {
        events
            .iter()
            .map(|(ts, e)| match e {
                Event::Turn(t) => (*ts, t),
                other => panic!("not a turn: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_turn_is_a_prompt_and_its_generations_without_the_fixed_count() {
        let (events, kept) = parse(&two_turns(), 0, true);
        let t = turns(&events);
        assert_eq!(kept, 2);
        assert_eq!(t.len(), 2);
        let (ts, first) = t[0];
        assert_eq!(ts, 1_000);
        assert_eq!(first.project, "/work/my app");
        assert_eq!(first.model, "gemini-3.8-flash-n");
        // Usage #1 (1,318) is not input: agy's own total leaves it out.
        assert_eq!(
            (first.input, first.output, first.cache_read),
            (8_303 + 8_666, 270 + 36, 30_000)
        );
        assert_eq!(
            (first.requests, first.context),
            (Some(2), Some(8_666 + 30_000))
        );
        assert_eq!(first.harness.as_deref(), Some("antigravity"));
        assert!(!first.agent && !first.aborted);
        // No #3: output is #9 + #10. No model id: left empty (unpriced).
        let (ts, second) = t[1];
        assert_eq!((ts, second.output, second.model.as_str()), (2_000, 42, ""));
    }

    #[test]
    fn the_last_turn_waits_until_the_conversation_is_idle_and_the_cursor_skips_old_turns() {
        let (events, kept) = parse(&two_turns(), 0, false);
        assert_eq!((turns(&events).len(), kept), (1, 1));
        let (events, kept) = parse(&two_turns(), 1, true);
        let t = turns(&events);
        assert_eq!((t.len(), kept, t[0].0), (1, 2, 2_000));
        let (events, kept) = parse(&two_turns(), 2, true);
        assert_eq!((events.len(), kept), (0, 2));
        // A cursor past what is done never moves back.
        assert_eq!(parse(&two_turns(), 2, false).1, 2);
    }

    #[test]
    fn generations_no_step_names_join_the_last_turn_and_bad_blobs_are_skipped() {
        let mut c = two_turns();
        c.gens.push((
            3,
            gen_blob("gemini-3.8-flash-n", 100, None, Some(1), "loose"),
        ));
        c.gens.push((4, vec![0xff, 0xff, 0xff]));
        c.steps.push((6, 15, vec![0x0a, 0x50]));
        let (events, _) = parse(&c, 1, true);
        let t = turns(&events);
        assert_eq!((t[0].1.input, t[0].1.requests), (9_217 + 100, Some(2)));
        // No steps at all: one turn at the conversation's creation time.
        let bare = Conversation {
            trajectory: trajectory("file:///w", 777),
            steps: vec![],
            gens: vec![(0, gen_blob("m", 5, None, Some(1), "r"))],
        };
        let (events, _) = parse(&bare, 0, true);
        assert_eq!(turns(&events)[0].0, 777);
        // A prompt with no generation (an error, say) is counted but records nothing.
        let mut lone = two_turns();
        lone.steps.push((7, 14, step_blob(3_000, None)));
        let (events, kept) = parse(&lone, 0, true);
        assert_eq!((turns(&events).len(), kept), (2, 3));
    }

    #[test]
    fn sqlite_rows_are_read_back_from_hex() {
        let c = two_turns();
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02X}")).collect::<String>();
        let mut text = format!("t|0|0|{}\n", hex(&c.trajectory));
        for (i, ty, m) in &c.steps {
            text.push_str(&format!("s|{i}|{ty}|{}\n", hex(m)));
        }
        for (i, d) in c.gens.iter().rev() {
            text.push_str(&format!("g|{i}|0|{}\n", hex(d)));
        }
        text.push_str("garbage\ng|x|0|00\ng|9|0|ABC\n");
        let back = Conversation::from_rows(&text);
        assert_eq!((back.steps.len(), back.gens.len()), (6, 3));
        assert_eq!(back.gens[0].0, 0);
        assert_eq!(parse(&back, 0, true).0, parse(&c, 0, true).0);
    }
}
