//! The crafted-skill contract.

use std::path::Path;

use serde_json::{Map, Value};

use crate::frontmatter::{parse_strict, split_frontmatter};

pub const MAX_BODY_BYTES: usize = 3072;
pub const MAX_DESCRIPTION_WORDS: usize = 30;

/// Contract violations for a SKILL.md whose directory is `dir_name`; empty when valid.
pub fn validate_text(dir_name: &str, text: &str) -> Vec<String> {
    let (frontmatter, body) = split_frontmatter(text);
    if frontmatter.trim().is_empty() {
        return vec!["missing frontmatter".into()];
    }
    let meta = match parse_strict(&frontmatter) {
        Ok(meta) => meta,
        Err(e) => return vec![format!("frontmatter is not valid YAML: {e}")],
    };
    let mut errors = check_name(meta.get("name"), dir_name);
    errors.extend(check_description(meta.get("description")));
    errors.extend(check_girth_metadata(meta.get("metadata")));
    if body.len() > MAX_BODY_BYTES {
        errors.push(format!(
            "body is {} bytes; max {MAX_BODY_BYTES} (move detail to references/)",
            body.len()
        ));
    }
    errors
}

pub fn validate(dir: &Path) -> Vec<String> {
    match std::fs::read(dir.join("SKILL.md")) {
        Ok(bytes) => {
            let name = dir.file_name().unwrap_or_default().to_string_lossy();
            validate_text(&name, &String::from_utf8_lossy(&bytes))
        }
        Err(_) => vec!["missing SKILL.md".into()],
    }
}

fn non_empty_str(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// `girth-<domain>`: lowercase alphanumeric segments joined by single hyphens.
fn is_crafted_name(name: &str) -> bool {
    name.strip_prefix("girth-").is_some_and(|domain| {
        domain.split('-').all(|seg| {
            !seg.is_empty()
                && seg
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
    })
}

fn check_name(name: Option<&Value>, dir_name: &str) -> Vec<String> {
    match name.and_then(Value::as_str) {
        Some(name) if is_crafted_name(name) => {
            if name == dir_name {
                vec![]
            } else {
                vec![format!(
                    "name '{name}' must equal directory name '{dir_name}'"
                )]
            }
        }
        _ => vec!["name must match girth-<domain> (lowercase, hyphenated)".into()],
    }
}

fn check_description(description: Option<&Value>) -> Vec<String> {
    match non_empty_str(description) {
        None => vec!["description is required".into()],
        Some(d) => {
            let words = d.split_whitespace().count();
            if words > MAX_DESCRIPTION_WORDS {
                vec![format!(
                    "description has {words} words; max {MAX_DESCRIPTION_WORDS}"
                )]
            } else {
                vec![]
            }
        }
    }
}

fn is_date(value: Option<&Value>) -> bool {
    let Some(s) = value.and_then(Value::as_str) else {
        return false;
    };
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |range: std::ops::Range<usize>| s.get(range).and_then(|p| p.parse::<u32>().ok());
    let (Some(year), Some(month), Some(day)) = (num(0..4), num(5..7), num(8..10)) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

fn check_girth_metadata(metadata: Option<&Value>) -> Vec<String> {
    let Some(girth) = metadata
        .and_then(|m| m.get("girth"))
        .and_then(Value::as_object)
    else {
        return vec!["metadata.girth block is required".into()];
    };
    let mut errors = check_sources(girth);
    if non_empty_str(girth.get("crafted_for")).is_none() {
        errors.push("metadata.girth.crafted_for is required".into());
    }
    for key in ["created", "last_used"] {
        if !is_date(girth.get(key)) {
            errors.push(format!("metadata.girth.{key} must be YYYY-MM-DD"));
        }
    }
    errors
}

fn check_sources(girth: &Map<String, Value>) -> Vec<String> {
    match girth.get("sources").and_then(Value::as_array) {
        Some(sources) if !sources.is_empty() => sources
            .iter()
            .enumerate()
            .filter(|(_, s)| non_empty_str(s.get("path-or-repo")).is_none())
            .map(|(i, _)| format!("metadata.girth.sources[{i}] needs path-or-repo"))
            .collect(),
        _ => vec!["metadata.girth.sources must be a non-empty list".into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::write_skill;
    use proptest::prelude::*;
    use tempfile::TempDir;

    const VALID_FM: &str = "name: girth-captions
description: Caption timing rules for this project's hyperframes videos.
metadata:
  girth:
    sources:
      - path-or-repo: ~/.agents/skills/hyperframes-core
        sections: [Captions]
    crafted_for: caption timing for promo video
    created: 2026-10-14
    last_used: 2026-10-14
";
    const DESC: &str = "description: Caption timing rules for this project's hyperframes videos.";
    const SOURCE: &str =
        "      - path-or-repo: ~/.agents/skills/hyperframes-core\n        sections: [Captions]\n";

    fn check(fm: &str, body: &str) -> Vec<String> {
        let tmp = TempDir::new().unwrap();
        validate(&write_skill(tmp.path(), "girth-captions", fm, body, &[]))
    }

    #[test]
    fn valid_skill_has_no_violations() {
        assert_eq!(check(VALID_FM, "body\n"), Vec::<String>::new());
    }

    #[test]
    fn contract_violations() {
        let words31 = format!("description: {}", "word ".repeat(31));
        let cases: [(&str, &str, &str); 11] = [
            ("name: girth-captions", "name: captions", "name must match"),
            (
                "name: girth-captions",
                "name: girth-Captions",
                "name must match",
            ),
            (
                "name: girth-captions",
                "name: girth-other",
                "must equal directory name",
            ),
            (DESC, &words31, "max 30"),
            (DESC, "description: ''", "description is required"),
            (
                "    crafted_for: caption timing for promo video\n",
                "",
                "crafted_for is required",
            ),
            (
                "created: 2026-10-14",
                "created: 2026-13-40",
                "created must be YYYY-MM-DD",
            ),
            (
                "created: 2026-10-14",
                "created: 2026-02-30",
                "created must be YYYY-MM-DD",
            ),
            (
                "last_used: 2026-10-14",
                "last_used: yesterday",
                "last_used must be YYYY-MM-DD",
            ),
            (
                SOURCE,
                "      - sections: [Captions]\n",
                "sources[0] needs path-or-repo",
            ),
            (
                SOURCE,
                "      - just-a-string\n",
                "sources[0] needs path-or-repo",
            ),
        ];
        for (old, new, expected) in cases {
            let fm = VALID_FM.replacen(old, new, 1);
            assert_ne!(fm, VALID_FM, "{old}");
            let errors = check(&fm, "");
            assert!(
                errors.iter().any(|e| e.contains(expected)),
                "{expected}: {errors:?}"
            );
        }
    }

    #[test]
    fn leap_day_and_exactly_30_words_are_valid() {
        let fm = VALID_FM
            .replace("created: 2026-10-14", "created: 2028-02-29")
            .replace(DESC, &format!("description: {}", "word ".repeat(30)));
        assert_eq!(check(&fm, ""), Vec::<String>::new());
    }

    #[test]
    fn empty_sources_list() {
        let fm = VALID_FM.replace(&format!("    sources:\n{SOURCE}"), "    sources: []\n");
        assert!(
            check(&fm, "").contains(&"metadata.girth.sources must be a non-empty list".to_string())
        );
    }

    #[test]
    fn missing_metadata() {
        assert_eq!(
            check("name: girth-captions\ndescription: x", ""),
            ["metadata.girth block is required"]
        );
    }

    #[test]
    fn invalid_yaml_is_rejected_strictly() {
        let errors = check("name: girth-captions\ndescription: Use when: x", "");
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].starts_with("frontmatter is not valid YAML"),
            "{errors:?}"
        );
    }

    #[test]
    fn body_budget_boundary() {
        assert_eq!(check(VALID_FM, &"x".repeat(3072)), Vec::<String>::new());
        assert_eq!(
            check(VALID_FM, &"x".repeat(3073)),
            ["body is 3073 bytes; max 3072 (move detail to references/)"]
        );
    }

    #[test]
    fn missing_skill_md_and_missing_frontmatter() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("girth-x")).unwrap();
        assert_eq!(validate(&tmp.path().join("girth-x")), ["missing SKILL.md"]);
        assert_eq!(
            validate_text("girth-x", "# no frontmatter\n"),
            ["missing frontmatter"]
        );
    }

    proptest! {
        #[test]
        fn validate_text_never_panics(text in any::<String>()) {
            let _ = validate_text("girth-x", &format!("---\n{text}\n---\nbody"));
        }
    }
}
