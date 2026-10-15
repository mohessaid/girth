//! Keeper and doctor: keep the prompt cache warm through idle gaps, and say
//! when a turn had to write the cache again.
//!
//! Claude Code writes a 1-hour prompt cache. A request after a longer pause
//! writes the whole conversation again at 2× input price; a ping (a fork of
//! the conversation) reads it at 1/40 of that and resets the hour.

use std::path::Path;

use serde::Serialize;

use crate::ledger::{Event, Record, Turn};
use crate::price;

/// A ping fires this long after the last request (the cache lives 60 min).
pub const INTERVAL_MS: u64 = 55 * 60 * 1000;
/// A break re-writing at least this many tokens gets a line under the answer.
pub const LINE_MIN_TOKENS: u64 = 20_000;
/// Idle at least this long before a break means the cache expired.
pub const TTL_SECS: u64 = 3600;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub max_pings: u32,
    pub min_context: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            max_pings: 3,
            min_context: 20_000,
        }
    }
}

/// What the mod should do after a turn: ping this many times, this far apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub pings: u32,
    pub interval_ms: u64,
}

/// `keeper` settings from `GIRTH_HOME/config.json`, then the project's
/// `.girth/config.json` (which wins for numbers); off when either says so.
pub fn settings(project_dir: &Path, home: &Path) -> Settings {
    let mut s = Settings::default();
    for file in [
        home.join(crate::ledger::CONFIG_FILE),
        project_dir.join(crate::config::CONFIG_PATH),
    ] {
        let keeper = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.get("keeper").cloned());
        let Some(keeper) = keeper else { continue };
        if keeper.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
            s.enabled = false;
        }
        if let Some(n) = keeper.get("max_pings").and_then(|v| v.as_u64()) {
            s.max_pings = u32::try_from(n).unwrap_or(u32::MAX);
        }
        if let Some(n) = keeper.get("min_context").and_then(|v| v.as_u64()) {
            s.min_context = n;
        }
    }
    s
}

/// The plan for a conversation of `context` tokens.
pub fn plan(settings: &Settings, off: bool, interval_ms: Option<u64>, context: u64) -> Plan {
    let active = settings.enabled && !off && context >= settings.min_context;
    Plan {
        pings: if active { settings.max_pings } else { 0 },
        interval_ms: interval_ms.unwrap_or(INTERVAL_MS),
    }
}

/// `450K`, `1.2M`: rounded down, so a line never overstates.
fn short_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}.{}M", n / 1_000_000, n / 100_000 % 10)
    } else {
        format!("{}K", n / 1_000)
    }
}

/// The line under the answer for a turn whose cache was written again, if any.
pub fn doctor_line(turn: &Turn) -> Option<String> {
    let worst = turn.breaks.iter().max_by_key(|b| b.rewritten)?;
    if worst.rewritten < LINE_MIN_TOKENS {
        return None;
    }
    let why = if worst.idle_s >= TTL_SECS {
        format!(
            "the prompt cache had expired (idle {}h {}m)",
            worst.idle_s / 3600,
            worst.idle_s % 3600 / 60
        )
    } else {
        "the prompt changed before the cached part".to_string()
    };
    let mut line = format!(
        "girth: {why}; this turn re-wrote {} tokens",
        short_tokens(worst.rewritten)
    );
    if let Some(r) = price::rates(&turn.model) {
        line.push_str(&format!(
            " (≈ ${:.2})",
            worst.rewritten as f64 * r.cache_write / 1e6
        ));
    }
    Some(line)
}

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Summary {
    /// Requests that wrote the cache again, and the tokens they re-wrote.
    pub breaks: u64,
    pub rewritten: u64,
    pub rewrite_cost: f64,
    pub pings: u64,
    pub ping_cost: f64,
    /// Idle periods with pings whose next turn found the cache warm.
    pub warm_returns: u64,
    /// The re-write those returns avoided, at most.
    pub warm_saved: f64,
}

/// Summarise breaks and pings, optionally for one project.
pub fn summarise(records: &[Record], project: Option<&str>) -> Summary {
    use std::collections::HashMap;
    let mut s = Summary::default();
    // Project -> (model, expected) of the latest ping since that project's last turn.
    let mut pending: HashMap<&str, (&str, u64)> = HashMap::new();
    let wanted = |p: &str| project.is_none_or(|want| want == p);
    for record in records {
        match &record.event {
            Event::Ping(p) if wanted(&p.project) => {
                s.pings += 1;
                if let Some(r) = price::rates(&p.model) {
                    s.ping_cost += price::cost(r, p.input, p.output, p.read, p.write);
                }
                pending.insert(&p.project, (&p.model, p.expected));
            }
            Event::Turn(t) if wanted(&t.project) => {
                let rates = price::rates(&t.model);
                for b in &t.breaks {
                    s.breaks += 1;
                    s.rewritten += b.rewritten;
                    if let Some(r) = rates {
                        s.rewrite_cost += b.rewritten as f64 * r.cache_write / 1e6;
                    }
                }
                if t.agent {
                    continue;
                }
                if let Some((model, expected)) = pending.remove(t.project.as_str()) {
                    if t.breaks.is_empty() {
                        s.warm_returns += 1;
                        if let Some(r) = price::rates(model) {
                            s.warm_saved += expected as f64 * (r.cache_write - r.cache_read) / 1e6;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    s
}

/// The keeper's `meter:` lines; empty when there is nothing to say.
pub fn render(summary: &Summary) -> String {
    use crate::measure::thousands;
    let mut lines = Vec::new();
    if summary.breaks > 0 {
        lines.push(format!(
            "meter: cache re-written {} times: {} tokens (≈ ${:.2})",
            summary.breaks,
            thousands(summary.rewritten),
            summary.rewrite_cost
        ));
    }
    if summary.pings > 0 {
        lines.push(format!(
            "meter: keeper: {} pings (≈ ${:.2}); {} returns found the cache warm (up to ≈ ${:.2} not re-written)",
            summary.pings, summary.ping_cost, summary.warm_returns, summary.warm_saved
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{Break, Ping};

    fn turn(model: &str, breaks: &[(u64, u64, u64)]) -> Turn {
        Turn {
            project: "/p".into(),
            model: model.into(),
            input: 1,
            output: 1,
            cache_read: 0,
            cache_write: 0,
            agent: false,
            aborted: false,
            lens_before: None,
            lens_after: None,
            requests: None,
            mcp: vec![],
            breaks: breaks
                .iter()
                .map(|&(lost, rewritten, idle_s)| Break {
                    lost,
                    rewritten,
                    idle_s,
                })
                .collect(),
        }
    }

    fn rec(ts: u64, event: Event) -> Record {
        Record { v: 1, ts, event }
    }

    fn ping(project: &str, read: u64, expected: u64) -> Event {
        Event::Ping(Ping {
            project: project.into(),
            model: "claude-opus-5-5".into(),
            input: 3,
            output: 1,
            read,
            write: 0,
            expected,
        })
    }

    #[test]
    fn plan_pings_large_conversations_unless_turned_off() {
        let s = Settings::default();
        assert_eq!(
            plan(&s, false, None, 20_000),
            Plan {
                pings: 3,
                interval_ms: 55 * 60 * 1000
            }
        );
        assert_eq!(plan(&s, false, None, 19_999).pings, 0);
        assert_eq!(plan(&s, true, None, 500_000).pings, 0);
        let off = Settings {
            enabled: false,
            ..Settings::default()
        };
        assert_eq!(plan(&off, false, None, 500_000).pings, 0);
        let two = Settings {
            max_pings: 2,
            min_context: 1_000,
            ..Settings::default()
        };
        assert_eq!(
            plan(&two, false, Some(50), 1_000),
            Plan {
                pings: 2,
                interval_ms: 50
            }
        );
    }

    #[test]
    fn settings_read_both_configs_and_the_project_wins() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (project, home) = (tmp.path().join("p"), tmp.path().join("h"));
        std::fs::create_dir_all(project.join(".girth")).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(settings(&project, &home), Settings::default());
        std::fs::write(
            home.join("config.json"),
            r#"{"keeper": {"max_pings": 5, "min_context": 1000}}"#,
        )
        .unwrap();
        std::fs::write(
            project.join(".girth/config.json"),
            r#"{"roots": [], "keeper": {"max_pings": 1}}"#,
        )
        .unwrap();
        assert_eq!(
            settings(&project, &home),
            Settings {
                enabled: true,
                max_pings: 1,
                min_context: 1000
            }
        );
        std::fs::write(
            home.join("config.json"),
            r#"{"keeper": {"enabled": false}}"#,
        )
        .unwrap();
        assert!(!settings(&project, &home).enabled);
        // Malformed values are ignored.
        std::fs::write(
            home.join("config.json"),
            r#"{"keeper": {"max_pings": "lots", "enabled": "no"}}"#,
        )
        .unwrap();
        assert_eq!(settings(&project, &home).max_pings, 1);
        assert!(settings(&project, &home).enabled);
    }

    #[test]
    fn doctor_names_the_largest_break_and_why() {
        // Idle for over an hour: the cache expired.
        let t = turn(
            "claude-opus-5-5",
            &[(460_000, 450_000, 11_520), (30_000, 25_000, 10)],
        );
        assert_eq!(
            doctor_line(&t).unwrap(),
            "girth: the prompt cache had expired (idle 3h 12m); this turn re-wrote 450K tokens (≈ $3.60)"
        );
        // Under an hour: the prompt changed before the cached part.
        let t = turn("claude-sonnet-5-5", &[(90_000, 80_000, 125)]);
        assert_eq!(
            doctor_line(&t).unwrap(),
            "girth: the prompt changed before the cached part; this turn re-wrote 80K tokens (≈ $0.32)"
        );
        // Exactly an hour counts as expired; minutes only when under an hour.
        let t = turn("claude-opus-5-5", &[(50_000, 40_000, 3_600)]);
        assert!(doctor_line(&t).unwrap().contains("(idle 1h 0m)"), "{t:?}");
        // Small breaks, no breaks: no line. The threshold is inclusive.
        assert_eq!(
            doctor_line(&turn("claude-opus-5-5", &[(19_999, 19_999, 9_000)])),
            None
        );
        assert_eq!(doctor_line(&turn("claude-opus-5-5", &[])), None);
        assert!(doctor_line(&turn("claude-opus-5-5", &[(20_000, 20_000, 9_000)])).is_some());
        // An unknown model has no price, and tokens of a million or more read as M.
        assert_eq!(
            doctor_line(&turn("mystery", &[(1_500_000, 1_250_000, 4_000)])).unwrap(),
            "girth: the prompt cache had expired (idle 1h 6m); this turn re-wrote 1.2M tokens"
        );
    }

    #[test]
    fn summary_counts_rewrites_pings_and_warm_returns() {
        let opus = price::rates("claude-opus-5-5").unwrap();
        let records = vec![
            rec(
                1,
                Event::Turn(turn("claude-opus-5-5", &[(100_000, 90_000, 4_000)])),
            ),
            // An idle period with two pings, then a turn with no break: a warm return.
            rec(2, ping("/p", 200_000, 200_100)),
            rec(3, ping("/p", 200_000, 200_100)),
            rec(4, Event::Turn(turn("claude-opus-5-5", &[]))),
            // A ping, then a turn that still broke: not warm.
            rec(5, ping("/p", 200_000, 200_100)),
            rec(
                6,
                Event::Turn(turn("claude-opus-5-5", &[(300_000, 300_000, 9_000)])),
            ),
            // Another project's ping and turn are left out with a project filter.
            rec(7, ping("/q", 1_000, 1_000)),
        ];
        let s = summarise(&records, Some("/p"));
        assert_eq!((s.breaks, s.rewritten), (2, 390_000));
        assert!((s.rewrite_cost - 390_000.0 * opus.cache_write / 1e6).abs() < 1e-9);
        assert_eq!(s.pings, 3);
        assert!((s.ping_cost - 3.0 * price::cost(opus, 3, 1, 200_000, 0)).abs() < 1e-9);
        assert_eq!(s.warm_returns, 1);
        assert!(
            (s.warm_saved - 200_100.0 * (opus.cache_write - opus.cache_read) / 1e6).abs() < 1e-9
        );
        let all = summarise(&records, None);
        assert_eq!(all.pings, 4);
        // A turn of another project between pings doesn't end this project's idle period.
        let records = vec![
            rec(1, ping("/p", 5, 5)),
            rec(
                2,
                Event::Turn(Turn {
                    project: "/q".into(),
                    ..turn("claude-opus-5-5", &[])
                }),
            ),
            rec(3, Event::Turn(turn("claude-opus-5-5", &[]))),
        ];
        assert_eq!(summarise(&records, Some("/p")).warm_returns, 1);
        assert_eq!(summarise(&records, None).warm_returns, 1);
        // A subagent's turn is not a return.
        let records = vec![
            rec(1, ping("/p", 5, 5)),
            rec(
                2,
                Event::Turn(Turn {
                    agent: true,
                    ..turn("claude-opus-5-5", &[])
                }),
            ),
        ];
        assert_eq!(summarise(&records, None).warm_returns, 0);
    }

    #[test]
    fn render_says_nothing_without_breaks_or_pings() {
        assert_eq!(render(&Summary::default()), "");
        let s = Summary {
            breaks: 2,
            rewritten: 390_000,
            rewrite_cost: 3.12,
            pings: 3,
            ping_cost: 0.12,
            warm_returns: 1,
            warm_saved: 1.56,
        };
        assert_eq!(
            render(&s),
            "meter: cache re-written 2 times: 390,000 tokens (≈ $3.12)\n\
             meter: keeper: 3 pings (≈ $0.12); 1 returns found the cache warm (up to ≈ $1.56 not re-written)"
        );
        let only_pings = Summary {
            pings: 1,
            ping_cost: 0.04,
            ..Summary::default()
        };
        assert_eq!(
            render(&only_pings),
            "meter: keeper: 1 pings (≈ $0.04); 0 returns found the cache warm (up to ≈ $0.00 not re-written)"
        );
    }
}
