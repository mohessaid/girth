//! The local skill ledger: an append-only JSONL file of events, plus consent config.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Context;

pub const LEDGER_FILE: &str = "ledger.jsonl";
pub const CONFIG_FILE: &str = "config.json";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraftSection {
    pub heading: String,
    pub hash: String,
    pub src: Vec<String>,
}

/// How a crafted skill used one source skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceUse {
    pub skill: String,
    /// Sections in the source skill; 0 when the source could not be found.
    pub total: u32,
    /// Slugs of the source sections referenced by `src` markers.
    pub used: Vec<String>,
    /// Slug → content hash of the referenced source sections.
    pub hashes: BTreeMap<String, String>,
    /// Hash of all of the source's section hashes; empty when the source was not found.
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedSection {
    pub hash: String,
    pub src: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Event {
    Use {
        skill: String,
        project: String,
    },
    Craft {
        skill: String,
        project: String,
        sections: Vec<CraftSection>,
        sources: Vec<SourceUse>,
        added: Vec<String>,
        removed: Vec<RemovedSection>,
        body_bytes: u64,
    },
    Feedback {
        skill: String,
        project: String,
        rating: Rating,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
}

/// One ledger line: schema version, unix timestamp, event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub v: u32,
    pub ts: u64,
    #[serde(flatten)]
    pub event: Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sharing {
    Unset,
    Allowed,
    Denied,
}

/// `GIRTH_HOME`, else `~/.girth`.
pub fn girth_home(ctx: &Context) -> PathBuf {
    match ctx.env.get("GIRTH_HOME").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => ctx
            .home
            .clone()
            .unwrap_or_else(|| ctx.cwd.clone())
            .join(".girth"),
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Append one event as a single line.
pub fn append(home: &Path, event: Event) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(home)?;
    let record = Record {
        v: SCHEMA_VERSION,
        ts: now(),
        event,
    };
    let mut line = serde_json::to_string(&record).map_err(std::io::Error::other)?;
    line.push('\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join(LEDGER_FILE))?;
    // Begin on a fresh line if a previous write was cut short.
    if file.metadata()?.len() > 0 && !ends_with_newline(&home.join(LEDGER_FILE)) {
        line.insert(0, '\n');
    }
    file.write_all(line.as_bytes())
}

fn ends_with_newline(path: &Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return true;
    };
    let mut last = [0u8; 1];
    file.seek(SeekFrom::End(-1)).is_ok() && file.read_exact(&mut last).is_ok() && last[0] == b'\n'
}

/// All readable records; malformed or unknown lines are skipped.
pub fn read(home: &Path) -> Vec<Record> {
    let Ok(text) = std::fs::read_to_string(home.join(LEDGER_FILE)) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<Record>(line).ok())
        .collect()
}

fn read_config(home: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(home.join(CONFIG_FILE))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

pub fn sharing(home: &Path) -> Sharing {
    match read_config(home).get("sharing").and_then(|v| v.as_str()) {
        Some("allowed") => Sharing::Allowed,
        Some("denied") => Sharing::Denied,
        _ => Sharing::Unset,
    }
}

pub fn set_sharing(home: &Path, value: Sharing) -> std::io::Result<()> {
    let mut config = read_config(home);
    match value {
        Sharing::Unset => config.remove("sharing"),
        Sharing::Allowed => config.insert("sharing".into(), "allowed".into()),
        Sharing::Denied => config.insert("sharing".into(), "denied".into()),
    };
    std::fs::create_dir_all(home)?;
    let text = serde_json::to_string_pretty(&config).map_err(std::io::Error::other)?;
    std::fs::write(home.join(CONFIG_FILE), text + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn use_event(skill: &str) -> Event {
        Event::Use {
            skill: skill.into(),
            project: "/p".into(),
        }
    }

    #[test]
    fn girth_home_prefers_env_then_home() {
        let mut ctx = Context {
            home: Some(PathBuf::from("/h")),
            ..Default::default()
        };
        assert_eq!(girth_home(&ctx), PathBuf::from("/h/.girth"));
        ctx.env.insert("GIRTH_HOME".into(), "/custom".into());
        assert_eq!(girth_home(&ctx), PathBuf::from("/custom"));
    }

    #[test]
    fn append_and_read_round_trip_with_version_and_time() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("g");
        append(&home, use_event("a")).unwrap();
        append(
            &home,
            Event::Feedback {
                skill: "girth-x".into(),
                project: "/p".into(),
                rating: Rating::Down,
                note: Some("meh".into()),
            },
        )
        .unwrap();
        let records = read(&home);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].v, SCHEMA_VERSION);
        assert!(records[0].ts > 1_700_000_000);
        assert_eq!(records[0].event, use_event("a"));
        let text = std::fs::read_to_string(home.join(LEDGER_FILE)).unwrap();
        assert!(text.lines().next().unwrap().contains(r#""kind":"use""#));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn missing_ledger_reads_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(read(tmp.path()).is_empty());
    }

    #[test]
    fn malformed_truncated_and_unknown_lines_are_skipped() {
        let tmp = TempDir::new().unwrap();
        append(tmp.path(), use_event("ok")).unwrap();
        let mut text = std::fs::read_to_string(tmp.path().join(LEDGER_FILE)).unwrap();
        text.push_str("not json\n{\"v\":1,\"ts\":1,\"kind\":\"teleport\"}\n\n{\"v\":1,\"ts\":2,\"kind\":\"use\",\"skill\":\"half");
        std::fs::write(tmp.path().join(LEDGER_FILE), text).unwrap();
        append(tmp.path(), use_event("after")).unwrap();
        let skills: Vec<Event> = read(tmp.path()).into_iter().map(|r| r.event).collect();
        assert_eq!(skills, [use_event("ok"), use_event("after")]);
    }

    #[test]
    fn sharing_defaults_to_unset_and_round_trips() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("g");
        assert_eq!(sharing(&home), Sharing::Unset);
        set_sharing(&home, Sharing::Allowed).unwrap();
        assert_eq!(sharing(&home), Sharing::Allowed);
        set_sharing(&home, Sharing::Denied).unwrap();
        assert_eq!(sharing(&home), Sharing::Denied);
        std::fs::write(home.join(CONFIG_FILE), "{broken").unwrap();
        assert_eq!(sharing(&home), Sharing::Unset);
    }

    #[test]
    fn set_sharing_preserves_other_config_keys() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(CONFIG_FILE), r#"{"other": 1}"#).unwrap();
        set_sharing(tmp.path(), Sharing::Allowed).unwrap();
        let text = std::fs::read_to_string(tmp.path().join(CONFIG_FILE)).unwrap();
        assert!(text.contains(r#""other""#) && text.contains(r#""sharing""#));
    }
}
