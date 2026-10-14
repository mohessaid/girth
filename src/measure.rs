//! What installed skills cost in context.

use serde::Serialize;

use crate::skill::{SkillInfo, approx_tokens};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub path: String,
    pub crafted: bool,
    pub always_on_bytes: u64,
    pub skill_md_bytes: u64,
    pub total_md_bytes: u64,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Totals {
    pub count: u64,
    pub crafted_count: u64,
    pub always_on_bytes: u64,
    pub always_on_tokens: u64,
    pub skill_md_bytes: u64,
    pub total_md_bytes: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Report {
    pub skills: Vec<Row>,
    pub totals: Totals,
}

/// Rows sorted by always-on bytes (desc), then name, then path.
pub fn measure(skills: &[SkillInfo]) -> Report {
    let mut sorted: Vec<&SkillInfo> = skills.iter().collect();
    sorted.sort_by(|a, b| {
        b.always_on_bytes()
            .cmp(&a.always_on_bytes())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });
    let rows: Vec<Row> = sorted
        .into_iter()
        .map(|s| Row {
            name: s.name.clone(),
            path: s.path.display().to_string(),
            crafted: s.crafted(),
            always_on_bytes: s.always_on_bytes(),
            skill_md_bytes: s.skill_md_bytes,
            total_md_bytes: s.total_md_bytes,
        })
        .collect();
    let always_on_bytes = rows.iter().map(|r| r.always_on_bytes).sum();
    let totals = Totals {
        count: rows.len() as u64,
        crafted_count: rows.iter().filter(|r| r.crafted).count() as u64,
        always_on_bytes,
        always_on_tokens: approx_tokens(always_on_bytes),
        skill_md_bytes: rows.iter().map(|r| r.skill_md_bytes).sum(),
        total_md_bytes: rows.iter().map(|r| r.total_md_bytes).sum(),
    };
    Report {
        skills: rows,
        totals,
    }
}

/// `1234567` → `"1,234,567"`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

pub fn format_table(report: &Report) -> String {
    let mut lines = vec![format!(
        "{:>10} {:>10} {:>11}  NAME",
        "ALWAYS-ON", "SKILL.md", "ALL-MD"
    )];
    for r in &report.skills {
        let mark = if r.crafted { " *" } else { "" };
        lines.push(format!(
            "{:>10} {:>10} {:>11}  {}{mark}",
            thousands(r.always_on_bytes),
            thousands(r.skill_md_bytes),
            thousands(r.total_md_bytes),
            r.name
        ));
    }
    let t = &report.totals;
    lines.push(format!(
        "TOTAL: {} skills ({} crafted) · always-on {} B ≈ {} tok · SKILL.md {} B · all md {} B",
        t.count,
        t.crafted_count,
        thousands(t.always_on_bytes),
        thousands(t.always_on_tokens),
        thousands(t.skill_md_bytes),
        thousands(t.total_md_bytes)
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::discover_skills;
    use crate::testutil::write_skill;
    use proptest::prelude::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn measure_rows_and_totals() {
        let tmp = TempDir::new().unwrap();
        write_skill(
            tmp.path(),
            "a",
            "name: a\ndescription: aaaa",
            &"x".repeat(100),
            &[],
        );
        write_skill(
            tmp.path(),
            "girth-b",
            "name: girth-b\ndescription: bb",
            "",
            &[],
        );
        let report = measure(&discover_skills(&[tmp.path().to_path_buf()]));
        let t = &report.totals;
        assert_eq!((t.count, t.crafted_count), (2, 1));
        assert_eq!(t.always_on_bytes, (1 + 4) + (7 + 2));
        assert_eq!(t.always_on_tokens, approx_tokens(t.always_on_bytes));
        assert_eq!(
            t.skill_md_bytes,
            report.skills.iter().map(|r| r.skill_md_bytes).sum::<u64>()
        );
        assert_eq!(
            t.total_md_bytes,
            report.skills.iter().map(|r| r.total_md_bytes).sum::<u64>()
        );
        assert_eq!(
            report
                .skills
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            ["girth-b", "a"]
        );
        assert!(report.skills[0].crafted);
    }

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(
            [0, 999, 1000, 1_234_567].map(thousands),
            ["0", "999", "1,000", "1,234,567"]
        );
    }

    #[test]
    fn measure_empty() {
        assert_eq!(measure(&[]).totals, Totals::default());
    }

    #[test]
    fn json_shape_is_frozen() {
        let json = serde_json::to_value(measure(&[])).unwrap();
        let keys: Vec<&String> = json["totals"].as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            [
                "always_on_bytes",
                "always_on_tokens",
                "count",
                "crafted_count",
                "skill_md_bytes",
                "total_md_bytes"
            ]
        );
        assert!(json["skills"].as_array().unwrap().is_empty());
    }

    #[test]
    fn table_lists_skills_and_total_line_with_thousands_separators() {
        let skill = SkillInfo {
            name: "girth-b".into(),
            description: "bb".into(),
            path: PathBuf::from("/s"),
            skill_md_bytes: 1234,
            total_md_bytes: 1_234_567,
            headings: vec![],
            fell_back: false,
        };
        let table = format_table(&measure(&[skill]));
        assert!(table.contains("girth-b *"), "{table}");
        assert!(table.contains("1,234,567"), "{table}");
        let last = table.lines().last().unwrap();
        assert!(
            last.starts_with("TOTAL: 1 skills (1 crafted) · always-on 9 B ≈ 3 tok"),
            "{last}"
        );
    }

    fn skill_strategy() -> impl Strategy<Value = SkillInfo> {
        ("[a-z][a-z-]{0,12}", ".{0,40}", 0u64..10_000, 0u64..100_000).prop_map(
            |(name, description, a, b)| SkillInfo {
                name,
                description,
                path: PathBuf::from("/s"),
                skill_md_bytes: a,
                total_md_bytes: b,
                headings: vec![],
                fell_back: false,
            },
        )
    }

    proptest! {
        #[test]
        fn byte_totals_are_additive(
            xs in proptest::collection::vec(skill_strategy(), 0..6),
            ys in proptest::collection::vec(skill_strategy(), 0..6),
        ) {
            let (a, b) = (measure(&xs).totals, measure(&ys).totals);
            let both: Vec<SkillInfo> = xs.iter().chain(ys.iter()).cloned().collect();
            let ab = measure(&both).totals;
            prop_assert_eq!(ab.count, a.count + b.count);
            prop_assert_eq!(ab.crafted_count, a.crafted_count + b.crafted_count);
            prop_assert_eq!(ab.always_on_bytes, a.always_on_bytes + b.always_on_bytes);
            prop_assert_eq!(ab.skill_md_bytes, a.skill_md_bytes + b.skill_md_bytes);
            prop_assert_eq!(ab.total_md_bytes, a.total_md_bytes + b.total_md_bytes);
        }
    }
}
