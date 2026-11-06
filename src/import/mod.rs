//! Spend from other harnesses' local session logs, appended to the ledger.
//!
//! Each source file's imported length is kept in `<girth home>/import.json`, so a
//! re-run appends only what is new. Logs are read, never changed.

pub mod codex;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ledger;

pub const OFFSETS_FILE: &str = "import.json";

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

/// Append the Codex turns under `<codex_home>/sessions` not imported before;
/// returns how many. A file shorter than its recorded offset is read again from the start.
pub fn import_codex(codex_home: &Path, girth_home: &Path) -> std::io::Result<usize> {
    let offsets_path = girth_home.join(OFFSETS_FILE);
    let mut offsets: Offsets = std::fs::read_to_string(&offsets_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
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
    if added > 0 || offsets_path.exists() {
        std::fs::create_dir_all(girth_home)?;
        let text = serde_json::to_string_pretty(&offsets).map_err(std::io::Error::other)?;
        std::fs::write(&offsets_path, text)?;
    }
    Ok(added)
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
        std::fs::write(dir.join("notes.txt"), FIXTURE).unwrap();
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
    fn nothing_to_import_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let girth = tmp.path().join("g");
        assert_eq!(import_codex(&tmp.path().join("none"), &girth).unwrap(), 0);
        assert!(!girth.exists());
    }
}
