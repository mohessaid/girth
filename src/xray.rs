//! `girth xray`: what one request was made of, from the mod's snapshot.

use serde::{Deserialize, Serialize};

use crate::measure::thousands;
use crate::meter::BYTES_PER_TOKEN;
use crate::price;

/// Rows shown before the rest are summed into one line.
pub const TOP: usize = 12;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Part {
    /// `attachment`, `section` or `tool`.
    pub kind: String,
    pub name: String,
    pub bytes: u64,
    /// Bytes before Lens rewrote this part, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Snapshot {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Report {
    pub model: String,
    pub bytes: u64,
    pub tokens: u64,
    /// Dollars per request when served from cache; None for an unknown model.
    pub read_cost: Option<f64>,
    /// Dollars to write it to cache once.
    pub write_cost: Option<f64>,
    pub lens_removed_bytes: u64,
    /// Parts, largest first.
    pub parts: Vec<Part>,
}

pub fn report(snapshot: &Snapshot) -> Report {
    let mut parts = snapshot.parts.clone();
    parts.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    let bytes: u64 = parts.iter().map(|p| p.bytes).sum();
    let tokens = bytes / BYTES_PER_TOKEN;
    let rates = price::rates(&snapshot.model);
    Report {
        model: snapshot.model.clone(),
        bytes,
        tokens,
        read_cost: rates.map(|r| tokens as f64 * r.cache_read / 1e6),
        write_cost: rates.map(|r| tokens as f64 * r.cache_write / 1e6),
        lens_removed_bytes: parts
            .iter()
            .filter_map(|p| p.before.map(|b| b.saturating_sub(p.bytes)))
            .sum(),
        parts,
    }
}

pub fn render(report: &Report) -> String {
    use std::fmt::Write;
    if report.parts.is_empty() {
        return "x-ray: nothing captured yet (send a message first)".to_string();
    }
    let mut out = format!(
        "x-ray: {} B ≈ {} tokens added to each request ({})\n",
        thousands(report.bytes),
        thousands(report.tokens),
        report.model
    );
    let share = |b: u64| 100.0 * b as f64 / report.bytes.max(1) as f64;
    for p in report.parts.iter().take(TOP) {
        let lens = p
            .before
            .map(|b| format!(" (Lens: was {} B)", thousands(b)))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "{:>8} B  {:>4.1}%  {} {}{lens}",
            thousands(p.bytes),
            share(p.bytes),
            p.kind,
            p.name
        );
    }
    if report.parts.len() > TOP {
        let rest = &report.parts[TOP..];
        let bytes: u64 = rest.iter().map(|p| p.bytes).sum();
        let _ = writeln!(
            out,
            "{:>8} B  {:>4.1}%  … {} smaller parts",
            thousands(bytes),
            share(bytes),
            rest.len()
        );
    }
    if report.lens_removed_bytes > 0 {
        let _ = writeln!(
            out,
            "\nLens removed {} B ≈ {} tokens from each request",
            thousands(report.lens_removed_bytes),
            thousands(report.lens_removed_bytes / BYTES_PER_TOKEN)
        );
    }
    if let (Some(read), Some(write)) = (report.read_cost, report.write_cost) {
        let _ = write!(
            out,
            "cost: ≈ ${read:.4} per request from cache · ≈ ${write:.4} to write it once"
        );
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(kind: &str, name: &str, bytes: u64, before: Option<u64>) -> Part {
        Part {
            kind: kind.into(),
            name: name.into(),
            bytes,
            before,
        }
    }

    fn probe() -> Snapshot {
        Snapshot {
            model: "claude-opus-5-5".into(),
            parts: vec![
                part("attachment", "mcp_instructions_delta", 287, Some(14_258)),
                part("attachment", "skill_listing", 7_869, Some(30_027)),
                part("section", "memory", 2_166, None),
                part("tool", "Bash", 4_000, None),
            ],
        }
    }

    #[test]
    fn report_sorts_sums_and_prices() {
        let r = report(&probe());
        assert_eq!(r.bytes, 14_322);
        assert_eq!(r.tokens, 14_322 / BYTES_PER_TOKEN);
        let names: Vec<&str> = r.parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            ["skill_listing", "Bash", "memory", "mcp_instructions_delta"]
        );
        assert_eq!(r.lens_removed_bytes, (30_027 - 7_869) + (14_258 - 287));
        let rates = price::rates("claude-opus-5-5").unwrap();
        let tokens = r.tokens as f64;
        assert!((r.read_cost.unwrap() - tokens * rates.cache_read / 1e6).abs() < 1e-12);
        assert!((r.write_cost.unwrap() - tokens * rates.cache_write / 1e6).abs() < 1e-12);
        let unknown = report(&Snapshot {
            model: "mystery".into(),
            ..probe()
        });
        assert_eq!((unknown.read_cost, unknown.write_cost), (None, None));
    }

    #[test]
    fn render_shows_shares_lens_and_costs() {
        let text = render(&report(&probe()));
        assert!(
            text.starts_with(
                "x-ray: 14,322 B ≈ 3,580 tokens added to each request (claude-opus-5-5)\n"
            ),
            "{text}"
        );
        assert!(
            text.contains("   7,869 B  54.9%  attachment skill_listing (Lens: was 30,027 B)\n"),
            "{text}"
        );
        assert!(
            text.contains(
                "     287 B   2.0%  attachment mcp_instructions_delta (Lens: was 14,258 B)\n"
            ),
            "{text}"
        );
        assert!(
            text.contains("\nLens removed 36,129 B ≈ 9,032 tokens from each request\n"),
            "{text}"
        );
        assert!(
            text.contains("cost: ≈ $0.0007 per request from cache · ≈ $0.0179 to write it once"),
            "{text}"
        );
    }

    #[test]
    fn many_small_parts_are_summed_and_empty_snapshots_say_so() {
        let mut s = Snapshot {
            model: "mystery".into(),
            parts: vec![],
        };
        for i in 0..20 {
            s.parts.push(part("tool", &format!("t{i}"), 100 + i, None));
        }
        let text = render(&report(&s));
        assert!(
            text.contains(&format!("… {} smaller parts", 20 - TOP)),
            "{text}"
        );
        assert!(
            !text.contains("cost:") && !text.contains("Lens removed"),
            "{text}"
        );
        assert_eq!(
            render(&report(&Snapshot::default())),
            "x-ray: nothing captured yet (send a message first)"
        );
    }
}
