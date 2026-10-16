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
    /// Compact a large conversation after an idle interval (opt-in).
    pub compact_idle: bool,
    pub compact_min_context: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            max_pings: 3,
            min_context: 20_000,
            compact_idle: false,
            compact_min_context: 150_000,
        }
    }
}

/// What the mod should do after a turn: ping this many times, this far apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub pings: u32,
    pub interval_ms: u64,
    /// Compact the conversation after `interval_ms` instead of pinging it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub compact: bool,
}

/// `keeper` and `compact` settings from `GIRTH_HOME/config.json`, then the
/// project's `.girth/config.json` (which wins for numbers); off when either
/// says so. Idle compaction is on only when a file says so and none says no.
pub fn settings(project_dir: &Path, home: &Path) -> Settings {
    let mut s = Settings::default();
    let mut compact_no = false;
    for file in [
        home.join(crate::ledger::CONFIG_FILE),
        project_dir.join(crate::config::CONFIG_PATH),
    ] {
        let value = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        if let Some(compact) = value.as_ref().and_then(|v| v.get("compact")) {
            match compact.get("idle").and_then(|v| v.as_bool()) {
                Some(true) => s.compact_idle = true,
                Some(false) => compact_no = true,
                None => {}
            }
            if let Some(n) = compact.get("min_context").and_then(|v| v.as_u64()) {
                s.compact_min_context = n;
            }
        }
        let Some(keeper) = value.and_then(|v| v.get("keeper").cloned()) else {
            continue;
        };
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
    s.compact_idle &= !compact_no;
    s
}

/// The model to run built-in subagents on (`agents.model`), from
/// `GIRTH_HOME/config.json`, then the project's config, which wins.
pub fn agents_model(project_dir: &Path, home: &Path) -> Option<String> {
    let mut model = None;
    for file in [
        home.join(crate::ledger::CONFIG_FILE),
        project_dir.join(crate::config::CONFIG_PATH),
    ] {
        let value = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        if let Some(m) = value
            .as_ref()
            .and_then(|v| v.pointer("/agents/model"))
            .and_then(|m| m.as_str())
            .filter(|m| !m.is_empty())
        {
            model = Some(m.to_string());
        }
    }
    model
}

/// The plan for a conversation of `context` tokens. For a conversation large
/// enough to compact, compaction replaces the pings: both would fire at once.
pub fn plan(
    settings: &Settings,
    keeper_off: bool,
    compact_off: bool,
    interval_ms: Option<u64>,
    context: u64,
) -> Plan {
    let compact = settings.compact_idle && !compact_off && context >= settings.compact_min_context;
    let ping = settings.enabled && !keeper_off && context >= settings.min_context && !compact;
    Plan {
        pings: if ping { settings.max_pings } else { 0 },
        interval_ms: interval_ms.unwrap_or(INTERVAL_MS),
        compact,
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

/// The line under the first answer after an idle compaction. Claude Code
/// puts the plugin's name in front, so the line has no `girth:` of its own.
pub fn compact_line(before: u64, after: Option<u64>) -> String {
    let sizes = match after {
        Some(after) => format!("{} → {} tokens", short_tokens(before), short_tokens(after)),
        None => format!("it was {} tokens", short_tokens(before)),
    };
    format!("compacted this conversation while you were away ({sizes})")
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
        "{why}; this turn re-wrote {} tokens",
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
    /// Idle compactions, and the conversation sizes before and after, summed.
    pub compactions: u64,
    pub compact_before: u64,
    pub compact_after: u64,
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
            Event::Compact(c) if wanted(&c.project) => {
                s.compactions += 1;
                s.compact_before += c.before;
                s.compact_after += c.after;
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
    if summary.compactions > 0 {
        lines.push(format!(
            "meter: idle compaction: {} times · {} → {} tokens on average",
            summary.compactions,
            short_tokens(summary.compact_before / summary.compactions),
            short_tokens(summary.compact_after / summary.compactions)
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
            plan(&s, false, false, None, 20_000),
            Plan {
                pings: 3,
                interval_ms: 55 * 60 * 1000,
                compact: false
            }
        );
        assert_eq!(plan(&s, false, false, None, 19_999).pings, 0);
        assert_eq!(plan(&s, true, false, None, 500_000).pings, 0);
        let off = Settings {
            enabled: false,
            ..Settings::default()
        };
        assert_eq!(plan(&off, false, false, None, 500_000).pings, 0);
        let two = Settings {
            max_pings: 2,
            min_context: 1_000,
            ..Settings::default()
        };
        assert_eq!(
            plan(&two, false, false, Some(50), 1_000),
            Plan {
                pings: 2,
                interval_ms: 50,
                compact: false
            }
        );
    }

    #[test]
    fn idle_compaction_replaces_pings_for_large_conversations_when_turned_on() {
        let off = Settings::default();
        assert!(!off.compact_idle);
        assert_eq!(off.compact_min_context, 150_000);
        // Off by default: v0.7 pings, and no `compact` key in the JSON.
        let p = plan(&off, false, false, None, 900_000);
        assert_eq!((p.pings, p.compact), (3, false));
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            r#"{"pings":3,"interval_ms":3300000}"#
        );
        let on = Settings {
            compact_idle: true,
            ..Settings::default()
        };
        let p = plan(&on, false, false, Some(50), 150_000);
        assert_eq!(
            p,
            Plan {
                pings: 0,
                interval_ms: 50,
                compact: true
            }
        );
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            r#"{"pings":0,"interval_ms":50,"compact":true}"#
        );
        // Below the threshold the keeper decides as before.
        assert_eq!(
            plan(&on, false, false, None, 149_999),
            plan(&off, false, false, None, 149_999)
        );
        // Turning compaction off leaves the pings; turning the keeper off leaves compaction.
        assert_eq!(plan(&on, false, true, None, 900_000).pings, 3);
        assert!(!plan(&on, false, true, None, 900_000).compact);
        assert!(plan(&on, true, false, None, 900_000).compact);
    }

    #[test]
    fn compact_settings_are_opt_in_and_a_no_anywhere_wins() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (project, home) = (tmp.path().join("p"), tmp.path().join("h"));
        std::fs::create_dir_all(project.join(".girth")).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let home_cfg = home.join("config.json");
        let project_cfg = project.join(".girth/config.json");
        std::fs::write(
            &home_cfg,
            r#"{"compact": {"idle": true, "min_context": 90000}}"#,
        )
        .unwrap();
        let s = settings(&project, &home);
        assert!(s.compact_idle);
        assert_eq!(s.compact_min_context, 90_000);
        // The project wins for numbers.
        std::fs::write(&project_cfg, r#"{"compact": {"min_context": 200000}}"#).unwrap();
        let s = settings(&project, &home);
        assert!(s.compact_idle);
        assert_eq!(s.compact_min_context, 200_000);
        // "idle": false in either file wins over true in the other.
        std::fs::write(&project_cfg, r#"{"compact": {"idle": false}}"#).unwrap();
        assert!(!settings(&project, &home).compact_idle);
        std::fs::write(&home_cfg, r#"{"compact": {"idle": false}}"#).unwrap();
        std::fs::write(&project_cfg, r#"{"compact": {"idle": true}}"#).unwrap();
        assert!(!settings(&project, &home).compact_idle);
        // The project alone can turn it on; malformed values are ignored.
        std::fs::write(
            &home_cfg,
            r#"{"compact": {"idle": "yes", "min_context": "big"}}"#,
        )
        .unwrap();
        let s = settings(&project, &home);
        assert!(s.compact_idle);
        assert_eq!(s.compact_min_context, 150_000);
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
                max_pings: 1,
                min_context: 1000,
                ..Settings::default()
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
            "the prompt cache had expired (idle 3h 12m); this turn re-wrote 450K tokens (≈ $3.60)"
        );
        // Under an hour: the prompt changed before the cached part.
        let t = turn("claude-sonnet-5-5", &[(90_000, 80_000, 125)]);
        assert_eq!(
            doctor_line(&t).unwrap(),
            "the prompt changed before the cached part; this turn re-wrote 80K tokens (≈ $0.32)"
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
            "the prompt cache had expired (idle 1h 6m); this turn re-wrote 1.2M tokens"
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
            rec(
                8,
                Event::Turn(Turn {
                    project: "/q".into(),
                    ..turn("claude-opus-5-5", &[(50_000, 50_000, 9_000)])
                }),
            ),
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
        assert_eq!((all.pings, all.breaks), (4, 3));
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
    fn compact_line_says_what_shrank_while_away() {
        assert_eq!(
            compact_line(583_412, Some(21_999)),
            "compacted this conversation while you were away (583K → 21K tokens)"
        );
        assert_eq!(
            compact_line(1_250_000, None),
            "compacted this conversation while you were away (it was 1.2M tokens)"
        );
    }

    #[test]
    fn summary_counts_idle_compactions_per_project() {
        let compact = |project: &str, before, after| {
            Event::Compact(crate::ledger::Compact {
                project: project.into(),
                model: "claude-opus-5-5".into(),
                before,
                after,
            })
        };
        let records = vec![
            rec(1, compact("/p", 600_000, 20_000)),
            rec(2, compact("/p", 400_000, 30_000)),
            rec(3, compact("/q", 1, 1)),
        ];
        let s = summarise(&records, Some("/p"));
        assert_eq!(
            (s.compactions, s.compact_before, s.compact_after),
            (2, 1_000_000, 50_000)
        );
        assert_eq!(summarise(&records, None).compactions, 3);
        assert_eq!(
            render(&s),
            "meter: idle compaction: 2 times · 500K → 25K tokens on average"
        );
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
            ..Summary::default()
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
