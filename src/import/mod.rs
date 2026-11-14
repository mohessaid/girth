//! Spend from other harnesses' local session logs, appended to the ledger.
//!
//! Each source file's imported length is kept in `<girth home>/import.json`, so a
//! re-run appends only what is new. Logs are read, never changed.

pub mod antigravity;
pub mod codex;
pub mod copilot;
pub mod gemini;
pub mod hermes;
pub mod opencode;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ledger;

pub const OFFSETS_FILE: &str = "import.json";

/// The harnesses `girth import` reads, in the order it reads them.
pub const SOURCES: [&str; 6] = [
    "codex",
    "copilot",
    "opencode",
    "gemini",
    "antigravity",
    "hermes",
];

type Offsets = BTreeMap<String, BTreeMap<String, usize>>;

fn rollouts(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rollouts(&path, out);
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
        {
            out.push(path);
        }
    }
}

fn load(girth_home: &Path) -> Offsets {
    std::fs::read_to_string(girth_home.join(OFFSETS_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save(girth_home: &Path, offsets: &Offsets, added: usize) -> std::io::Result<()> {
    let path = girth_home.join(OFFSETS_FILE);
    if added > 0 || path.exists() {
        std::fs::create_dir_all(girth_home)?;
        let text = serde_json::to_string_pretty(offsets).map_err(std::io::Error::other)?;
        std::fs::write(&path, text)?;
    }
    Ok(())
}

/// Append the Codex turns under `<codex_home>/sessions` not imported before;
/// returns how many. A file shorter than its recorded offset is read again from the start.
pub fn import_codex(codex_home: &Path, girth_home: &Path) -> std::io::Result<usize> {
    let mut offsets = load(girth_home);
    let mut files = Vec::new();
    rollouts(&codex_home.join("sessions"), &mut files);
    files.sort();
    let seen = offsets.entry("codex".into()).or_default();
    let mut added = 0;
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let key = file.display().to_string();
        let start = seen
            .get(&key)
            .copied()
            .filter(|&o| o <= text.len())
            .unwrap_or(0);
        let Some(rest) = text.get(start..) else {
            continue;
        };
        let (events, consumed) = codex::parse(rest);
        for (ts, event) in events {
            ledger::append_at(girth_home, ts, event)?;
            added += 1;
        }
        if consumed > 0 {
            seen.insert(key, start + consumed);
        }
    }
    save(girth_home, &offsets, added)?;
    Ok(added)
}

/// Append the Copilot CLI turns and skill uses under `<copilot_home>/session-state` not
/// imported before; returns how many turns. Each session keeps its imported shutdowns, and
/// under `<file>#records` its imported usage records; a session holding fewer of either
/// than recorded is read again from the start.
pub fn import_copilot(copilot_home: &Path, girth_home: &Path) -> std::io::Result<usize> {
    let mut offsets = load(girth_home);
    let mut files: Vec<PathBuf> = std::fs::read_dir(copilot_home.join("session-state"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("events.jsonl"))
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    let seen = offsets.entry("copilot".into()).or_default();
    let (mut added, mut turns) = (0, 0);
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let key = file.display().to_string();
        let records_key = format!("{key}#records");
        let recorded = copilot::Cursor {
            shutdowns: seen.get(&key).copied().unwrap_or(0),
            records: seen.get(&records_key).copied().unwrap_or(0),
        };
        let (mut events, mut read) = copilot::parse(&text, recorded);
        if read.shutdowns < recorded.shutdowns || read.records < recorded.records {
            (events, read) = copilot::parse(&text, copilot::Cursor::default());
        }
        for (ts, event) in events {
            turns += usize::from(matches!(event, ledger::Event::Turn(_)));
            ledger::append_at(girth_home, ts, event)?;
            added += 1;
        }
        if read.shutdowns > 0 {
            seen.insert(key, read.shutdowns);
        }
        if read.records > 0 {
            seen.insert(records_key, read.records);
        }
    }
    save(girth_home, &offsets, added)?;
    Ok(turns)
}

/// Append the Gemini CLI turns and skill uses under `<gemini_dir>/tmp` not imported
/// before; returns how many turns. Each session file keeps the time of its last imported
/// prompt, since Gemini rewrites older session files whole.
pub fn import_gemini(gemini_dir: &Path, girth_home: &Path) -> std::io::Result<usize> {
    let mut offsets = load(girth_home);
    let seen = offsets.entry("gemini".into()).or_default();
    let (mut added, mut turns) = (0, 0);
    for (file, project) in gemini::sessions(gemini_dir) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let key = file.display().to_string();
        let cursor = seen.get(&key).copied().unwrap_or(0) as u64;
        let (events, kept) = gemini::parse(&text, &project, cursor);
        for (ts, event) in events {
            turns += usize::from(matches!(event, ledger::Event::Turn(_)));
            ledger::append_at(girth_home, ts, event)?;
            added += 1;
        }
        if kept > cursor {
            seen.insert(key, kept as usize);
        }
    }
    save(girth_home, &offsets, added)?;
    Ok(turns)
}

/// Append the Antigravity CLI turns in `<dir>/*.db` not imported before, read by running
/// `<sqlite3> -readonly` on each; returns how many. A conversation's last turn waits until
/// the file has been idle for `antigravity::IDLE_DONE_S` or a later prompt exists. A
/// `sqlite3` that can't be run is noted on stderr and imports nothing; a file it can't
/// read is skipped.
pub fn import_antigravity(
    sqlite3: &Path,
    dir: &Path,
    girth_home: &Path,
    now: u64,
) -> std::io::Result<usize> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(0);
    };
    let mut dbs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "db"))
        .collect();
    dbs.sort();
    let mut offsets = load(girth_home);
    let seen = offsets.entry("antigravity".into()).or_default();
    let (mut added, mut turns) = (0, 0);
    for db in dbs {
        let uri = format!("file:{}?mode=ro", db.display());
        let run = std::process::Command::new(sqlite3)
            .args([
                "-readonly",
                "-batch",
                "-noheader",
                "-separator",
                "|",
                &uri,
                antigravity::QUERY,
            ])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        let rows = match run {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
            Ok(_) => continue,
            Err(e) => {
                eprintln!(
                    "girth: skipped Antigravity: could not run {}: {e}",
                    sqlite3.display()
                );
                return Ok(0);
            }
        };
        // agy writes the database or its -wal on every step. Readers, Girth's own sqlite3
        // included, touch the -shm index and may create an empty -wal, so those don't count.
        let changed = ["", "-wal"]
            .iter()
            .filter_map(|x| std::fs::metadata(format!("{}{x}", db.display())).ok())
            .filter(|m| m.len() > 0)
            .filter_map(|m| m.modified().ok())
            .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .max()
            .unwrap_or(0);
        let idle = now.saturating_sub(changed) >= antigravity::IDLE_DONE_S;
        let key = db.display().to_string();
        let cursor = seen.get(&key).copied().unwrap_or(0);
        let (events, kept) =
            antigravity::parse(&antigravity::Conversation::from_rows(&rows), cursor, idle);
        for (ts, event) in events {
            ledger::append_at(girth_home, ts, event)?;
            added += 1;
            turns += 1;
        }
        if kept > cursor {
            seen.insert(key, kept);
        }
    }
    save(girth_home, &offsets, added)?;
    Ok(turns)
}

/// Append what grew in Hermes Agent's usage totals in `db` (its `state.db`) since the
/// last import, read with `<sqlite3> -readonly -json`; returns how many turns. Nothing
/// runs when `db` doesn't exist. A `sqlite3` that can't be run is noted on stderr and
/// imports nothing; a database it can't read imports nothing.
pub fn import_hermes(sqlite3: &Path, db: &Path, girth_home: &Path) -> std::io::Result<usize> {
    if !db.is_file() {
        return Ok(0);
    }
    let uri = format!("file:{}?mode=ro", db.display());
    let run = std::process::Command::new(sqlite3)
        .args(["-readonly", "-batch", "-json", &uri, hermes::QUERY])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let rows: Vec<hermes::Row> = match run {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            if text.trim().is_empty() {
                Vec::new()
            } else {
                serde_json::from_str(&text).unwrap_or_default()
            }
        }
        Ok(_) => return Ok(0),
        Err(e) => {
            eprintln!(
                "girth: skipped Hermes: could not run {}: {e}",
                sqlite3.display()
            );
            return Ok(0);
        }
    };
    let mut offsets = load(girth_home);
    let seen = offsets.entry("hermes".into()).or_default();
    let mut added = 0;
    for (ts, event) in hermes::parse(&rows, seen) {
        ledger::append_at(girth_home, ts, event)?;
        added += 1;
    }
    save(girth_home, &offsets, added)?;
    Ok(added)
}

/// Append the OpenCode turns and skill uses in `db` not imported before, read by running
/// `<bin> db` (OpenCode resolves its own database); returns how many turns. Nothing runs
/// when `db` doesn't exist, so Girth never creates OpenCode's state. An OpenCode that
/// can't be run, or has no `db` command, is noted on stderr and imports nothing.
pub fn import_opencode(bin: &Path, db: &Path, girth_home: &Path) -> std::io::Result<usize> {
    import_opencode_with(bin, Path::new("sqlite3"), db, girth_home)
}

/// `import_opencode`, reading an OpenCode 2 database (it has `session_message`, and no
/// `opencode db` command) with `<sqlite3> -readonly -json` instead.
pub fn import_opencode_with(
    bin: &Path,
    sqlite3: &Path,
    db: &Path,
    girth_home: &Path,
) -> std::io::Result<usize> {
    if !db.is_file() {
        return Ok(0);
    }
    let uri = format!("file:{}?mode=ro", db.display());
    let sqlite = |sql: &str| {
        std::process::Command::new(sqlite3)
            .args(["-readonly", "-batch", "-json", &uri, sql])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
    };
    let v2 = sqlite("SELECT 1 AS v2 FROM sqlite_master WHERE name = 'session_message'")
        .is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("v2"));
    let run = if v2 {
        sqlite(opencode::QUERY_V2)
    } else {
        std::process::Command::new(bin)
            .args(["db", "--pure", "--format", "json", opencode::QUERY])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
    };
    let rows = match run {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        Ok(out) => {
            eprintln!(
                "girth: skipped OpenCode: `opencode db` exited with {}",
                out.status
            );
            return Ok(0);
        }
        Err(e) => {
            eprintln!(
                "girth: skipped OpenCode: could not run {}: {e}",
                bin.display()
            );
            return Ok(0);
        }
    };
    let mut offsets = load(girth_home);
    let seen = offsets.entry("opencode".into()).or_default();
    let (events, kept) = opencode::parse(&rows, seen);
    let (mut added, mut turns) = (0, 0);
    for (ts, event) in events {
        turns += usize::from(matches!(event, ledger::Event::Turn(_)));
        ledger::append_at(girth_home, ts, event)?;
        added += 1;
    }
    seen.extend(kept);
    save(girth_home, &offsets, added)?;
    Ok(turns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const FIXTURE: &str = include_str!(
        "../../tests/fixtures/codex/sessions/2026/10/25/rollout-2026-10-25T11-20-54-s1.jsonl"
    );

    fn codex_home(tmp: &TempDir, text: &str) -> PathBuf {
        let dir = tmp.path().join("codex/sessions/2026/10/25");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("rollout-x.jsonl"), text).unwrap();
        // Only rollout-*.jsonl files are sessions.
        std::fs::write(dir.join("notes.txt"), FIXTURE).unwrap();
        std::fs::write(dir.join("history.jsonl"), FIXTURE).unwrap();
        std::fs::write(dir.join("rollout-y.txt"), FIXTURE).unwrap();
        tmp.path().join("codex")
    }

    #[test]
    fn appended_turns_are_imported_on_the_next_run() {
        let tmp = TempDir::new().unwrap();
        let (codex, girth) = (codex_home(&tmp, FIXTURE), tmp.path().join("g"));
        assert_eq!(import_codex(&codex, &girth).unwrap(), 3);
        let more = format!(
            "{FIXTURE}{}\n",
            r#"{"timestamp":"2026-10-25T10:51:00.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"t4"}}"#
        );
        std::fs::write(codex.join("sessions/2026/10/25/rollout-x.jsonl"), more).unwrap();
        assert_eq!(import_codex(&codex, &girth).unwrap(), 1);
        assert_eq!(import_codex(&codex, &girth).unwrap(), 0);
        assert_eq!(ledger::read(&girth).len(), 4);
    }

    #[test]
    fn a_rewritten_shorter_file_is_read_again() {
        let tmp = TempDir::new().unwrap();
        let (codex, girth) = (codex_home(&tmp, FIXTURE), tmp.path().join("g"));
        assert_eq!(import_codex(&codex, &girth).unwrap(), 3);
        // The session's first turn only: session meta through t1's end line.
        let short: String = FIXTURE.split_inclusive('\n').take(6).collect();
        std::fs::write(codex.join("sessions/2026/10/25/rollout-x.jsonl"), &short).unwrap();
        assert_eq!(import_codex(&codex, &girth).unwrap(), 1);
    }

    #[test]
    fn a_file_with_only_an_open_turn_gets_no_offset() {
        let tmp = TempDir::new().unwrap();
        let open: String = FIXTURE.split_inclusive('\n').skip(13).collect();
        let (codex, girth) = (codex_home(&tmp, &open), tmp.path().join("g"));
        std::fs::write(codex.join("sessions/2026/10/25/rollout-z.jsonl"), FIXTURE).unwrap();
        assert_eq!(import_codex(&codex, &girth).unwrap(), 3); // all from rollout-z
        let offsets = std::fs::read_to_string(girth.join(OFFSETS_FILE)).unwrap();
        assert!(!offsets.contains("rollout-x.jsonl"), "{offsets}");
    }

    #[test]
    fn nothing_to_import_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let girth = tmp.path().join("g");
        assert_eq!(import_codex(&tmp.path().join("none"), &girth).unwrap(), 0);
        assert!(!girth.exists());
    }

    const COPILOT: &str =
        include_str!("../../tests/fixtures/copilot/session-state/s1/events.jsonl");

    fn copilot_home(tmp: &TempDir, text: &str) -> PathBuf {
        let home = tmp.path().join("copilot");
        let dir = home.join("session-state/s1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("events.jsonl"), text).unwrap();
        // Only session-state/<id>/events.jsonl files are sessions.
        std::fs::write(home.join("session-state/events.jsonl"), COPILOT).unwrap();
        std::fs::create_dir_all(home.join("session-state/s2")).unwrap();
        std::fs::write(home.join("session-state/s2/other.jsonl"), COPILOT).unwrap();
        home
    }

    #[test]
    fn copilot_shutdowns_are_imported_once() {
        let tmp = TempDir::new().unwrap();
        let (copilot, girth) = (copilot_home(&tmp, COPILOT), tmp.path().join("g"));
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 5);
        assert_eq!(ledger::read(&girth).len(), 6); // 5 turns and 1 skill use
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 0);
        let more = format!(
            "{COPILOT}\n{}\n",
            r#"{"type":"session.shutdown","timestamp":"2026-07-09T10:00:00.000Z","data":{"modelMetrics":{"claude-sonnet-4.6":{"requests":{"count":1,"cost":1},"usage":{"inputTokens":10,"outputTokens":1,"cacheReadTokens":0,"cacheWriteTokens":0}}}}}"#
        );
        std::fs::write(copilot.join("session-state/s1/events.jsonl"), more).unwrap();
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 1);
        // The open stretch's skill use came with its shutdown.
        assert_eq!(ledger::read(&girth).len(), 8);
    }

    #[test]
    fn a_copilot_session_with_fewer_shutdowns_is_read_again() {
        let tmp = TempDir::new().unwrap();
        let (copilot, girth) = (copilot_home(&tmp, COPILOT), tmp.path().join("g"));
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 5);
        let first: String = COPILOT.split_inclusive('\n').take(8).collect();
        std::fs::write(copilot.join("session-state/s1/events.jsonl"), first).unwrap();
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 1);
    }

    const RECORDS: &str =
        include_str!("../../tests/fixtures/copilot/session-state/s2/events.jsonl");

    #[test]
    fn copilot_records_are_imported_once_with_their_own_cursor() {
        let tmp = TempDir::new().unwrap();
        let (copilot, girth) = (copilot_home(&tmp, RECORDS), tmp.path().join("g"));
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 5);
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 0);
        let file = copilot
            .join("session-state/s1/events.jsonl")
            .display()
            .to_string();
        let cursors = load(&girth);
        let mine = &cursors["copilot"];
        assert_eq!((mine[&file], mine[&format!("{file}#records")]), (2, 5));
    }

    #[test]
    fn a_copilot_cursor_from_before_records_skips_what_it_counted() {
        let tmp = TempDir::new().unwrap();
        let (copilot, girth) = (copilot_home(&tmp, RECORDS), tmp.path().join("g"));
        // Written by 0.12: both shutdowns imported, records not read.
        let file = copilot
            .join("session-state/s1/events.jsonl")
            .display()
            .to_string();
        let old: Offsets = [("copilot".to_string(), [(file, 2)].into())].into();
        save(&girth, &old, 1).unwrap();
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 1);
    }

    #[test]
    fn a_copilot_session_with_fewer_records_is_read_again() {
        let tmp = TempDir::new().unwrap();
        let (copilot, girth) = (copilot_home(&tmp, RECORDS), tmp.path().join("g"));
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 5);
        // Same two shutdowns, but the running stretch lost its done turn.
        let cut: String = RECORDS.split_inclusive('\n').take(16).collect();
        std::fs::write(copilot.join("session-state/s1/events.jsonl"), cut).unwrap();
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 4);
    }

    #[test]
    fn copilot_and_codex_keep_separate_cursors() {
        let tmp = TempDir::new().unwrap();
        let girth = tmp.path().join("g");
        let copilot = copilot_home(&tmp, COPILOT);
        let codex = codex_home(&tmp, FIXTURE);
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 5);
        assert_eq!(import_codex(&codex, &girth).unwrap(), 3);
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 0);
        assert_eq!(import_codex(&codex, &girth).unwrap(), 0);
    }

    #[test]
    fn a_copilot_session_not_yet_shut_down_gets_no_cursor() {
        let tmp = TempDir::new().unwrap();
        let open: String = COPILOT.split_inclusive('\n').take(4).collect();
        let (copilot, girth) = (copilot_home(&tmp, &open), tmp.path().join("g"));
        std::fs::create_dir_all(copilot.join("session-state/s3")).unwrap();
        std::fs::write(copilot.join("session-state/s3/events.jsonl"), COPILOT).unwrap();
        assert_eq!(import_copilot(&copilot, &girth).unwrap(), 5); // all from s3
        let cursors = std::fs::read_to_string(girth.join(OFFSETS_FILE)).unwrap();
        assert!(!cursors.contains("s1"), "{cursors}");
    }

    #[test]
    fn no_copilot_sessions_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let girth = tmp.path().join("g");
        assert_eq!(import_copilot(&tmp.path().join("none"), &girth).unwrap(), 0);
        assert!(!girth.exists());
    }

    fn opencode_fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/opencode/opencode")
    }

    #[test]
    fn opencode_turns_are_imported_once() {
        let tmp = TempDir::new().unwrap();
        let (db, girth) = (tmp.path().join("opencode.db"), tmp.path().join("g"));
        std::fs::write(&db, "").unwrap();
        assert_eq!(
            import_opencode(&opencode_fixture(), &db, &girth).unwrap(),
            4
        );
        assert_eq!(ledger::read(&girth).len(), 5); // 4 turns and 1 skill use
        assert_eq!(
            import_opencode(&opencode_fixture(), &db, &girth).unwrap(),
            0
        );
        assert_eq!(ledger::read(&girth).len(), 5);
    }

    #[test]
    fn opencode_is_not_run_without_its_database() {
        let tmp = TempDir::new().unwrap();
        let girth = tmp.path().join("g");
        // The stand-in would import 4 turns if it ran.
        let db = tmp.path().join("missing.db");
        assert_eq!(
            import_opencode(&opencode_fixture(), &db, &girth).unwrap(),
            0
        );
        assert!(!girth.exists());
    }

    #[test]
    fn an_opencode_that_fails_or_is_missing_imports_nothing() {
        let tmp = TempDir::new().unwrap();
        let (db, girth) = (tmp.path().join("opencode.db"), tmp.path().join("g"));
        std::fs::write(&db, "").unwrap();
        assert_eq!(
            import_opencode(Path::new("/bin/false"), &db, &girth).unwrap(),
            0
        );
        // Rows on stdout don't count when the command failed.
        let failing = tmp.path().join("failing-opencode");
        let script = format!(
            "#!/bin/sh\n{} \"$@\"\nexit 1\n",
            opencode_fixture().display()
        );
        std::fs::write(&failing, script).unwrap();
        std::fs::set_permissions(
            &failing,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
        assert_eq!(import_opencode(&failing, &db, &girth).unwrap(), 0);
        let none = tmp.path().join("no-such-opencode");
        assert_eq!(import_opencode(&none, &db, &girth).unwrap(), 0);
        assert!(!girth.exists());
    }

    fn gemini_home(tmp: &TempDir) -> PathBuf {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gemini/.gemini");
        let home = tmp.path().join("gemini");
        let status = std::process::Command::new("cp")
            .arg("-R")
            .arg(&fixture)
            .arg(&home)
            .status()
            .unwrap();
        assert!(status.success());
        home
    }

    #[test]
    fn gemini_turns_are_imported_once() {
        let tmp = TempDir::new().unwrap();
        let (gemini, girth) = (gemini_home(&tmp), tmp.path().join("g"));
        assert_eq!(import_gemini(&gemini, &girth).unwrap(), 4);
        assert_eq!(ledger::read(&girth).len(), 5); // 4 turns and 1 skill use
        assert_eq!(import_gemini(&gemini, &girth).unwrap(), 0);
        // The open turn is imported once a later prompt closes it.
        let file = gemini.join("tmp/proj/chats/session-2026-05-06T17-00-ga.json");
        let mut session: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        session["messages"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!(
                {"id": "u3", "timestamp": "2026-05-06T17:00:40.000Z", "type": "user"}
            ));
        std::fs::write(&file, session.to_string()).unwrap();
        assert_eq!(import_gemini(&gemini, &girth).unwrap(), 1);
        assert_eq!(import_gemini(&gemini, &girth).unwrap(), 0);
    }

    #[test]
    fn a_gemini_session_with_only_an_open_turn_gets_no_cursor() {
        let tmp = TempDir::new().unwrap();
        let (gemini, girth) = (gemini_home(&tmp), tmp.path().join("g"));
        let open = r#"{"sessionId":"o","messages":[{"id":"u","timestamp":"2026-05-06T18:00:00.000Z","type":"user"},{"id":"g","timestamp":"2026-05-06T18:00:01.000Z","type":"gemini","toolCalls":[{"name":"x"}],"tokens":{"input":1,"output":1,"cached":0}}]}"#;
        std::fs::write(gemini.join("tmp/proj/chats/session-open.json"), open).unwrap();
        assert_eq!(import_gemini(&gemini, &girth).unwrap(), 4);
        let cursors = std::fs::read_to_string(girth.join(OFFSETS_FILE)).unwrap();
        assert!(!cursors.contains("session-open"), "{cursors}");
    }

    #[test]
    fn no_gemini_sessions_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let girth = tmp.path().join("g");
        assert_eq!(import_gemini(&tmp.path().join("none"), &girth).unwrap(), 0);
        assert!(!girth.exists());
    }
}
