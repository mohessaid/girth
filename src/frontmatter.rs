//! SKILL.md frontmatter: strict YAML for validation, lenient recovery for discovery.

use serde_json::{Map, Number, Value};
use yaml_rust2::{Yaml, YamlLoader};

const BLOCK_INDICATORS: [&str; 6] = [">", "|", ">-", "|-", ">+", "|+"];

/// Split SKILL.md text into (frontmatter, body).
///
/// Missing or unterminated frontmatter yields `("", text)`.
pub fn split_frontmatter(text: &str) -> (String, String) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    if lines.first().is_none_or(|first| first.trim() != "---") {
        return (String::new(), text.to_string());
    }
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            return (lines[1..i].concat(), lines[i + 1..].concat());
        }
    }
    (String::new(), text.to_string())
}

/// Parse frontmatter as YAML. Errors when it is not valid YAML or not a mapping.
pub fn parse_strict(frontmatter: &str) -> Result<Map<String, Value>, String> {
    let docs = YamlLoader::load_from_str(frontmatter).map_err(|e| e.to_string())?;
    match docs.into_iter().next() {
        Some(Yaml::Hash(hash)) => Ok(hash_to_map(hash)),
        Some(_) => Err("frontmatter is not a mapping".into()),
        None => Err("frontmatter is empty".into()),
    }
}

/// Parsed frontmatter plus whether the lenient fallback had to be used.
#[derive(Debug, Clone, PartialEq)]
pub struct Lenient {
    pub meta: Map<String, Value>,
    pub fell_back: bool,
}

/// Parse strictly; if that fails, recover top-level scalar keys line by line.
pub fn parse_lenient(frontmatter: &str) -> Lenient {
    match parse_strict(frontmatter) {
        Ok(meta) => Lenient {
            meta,
            fell_back: false,
        },
        Err(_) => Lenient {
            meta: scan_top_level(frontmatter),
            fell_back: true,
        },
    }
}

fn hash_to_map(hash: yaml_rust2::yaml::Hash) -> Map<String, Value> {
    hash.into_iter()
        .filter_map(|(k, v)| scalar_key(&k).map(|key| (key, to_json(v))))
        .collect()
}

fn scalar_key(key: &Yaml) -> Option<String> {
    match key {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn to_json(value: Yaml) -> Value {
    match value {
        Yaml::String(s) => Value::String(s),
        Yaml::Integer(i) => Value::Number(i.into()),
        Yaml::Real(s) => s
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map_or(Value::String(s), Value::Number),
        Yaml::Boolean(b) => Value::Bool(b),
        Yaml::Array(items) => Value::Array(items.into_iter().map(to_json).collect()),
        Yaml::Hash(hash) => Value::Object(hash_to_map(hash)),
        Yaml::Alias(_) | Yaml::Null | Yaml::BadValue => Value::Null,
    }
}

/// Recover `key: value` pairs at column 0, including `>`/`|` block scalars.
fn scan_top_level(frontmatter: &str) -> Map<String, Value> {
    let lines: Vec<&str> = frontmatter.lines().collect();
    let mut out = Map::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.trim().is_empty() || line.starts_with([' ', '\t', '#', '-']) {
            continue;
        }
        let Some((key, rest)) = split_key(line) else {
            continue;
        };
        if BLOCK_INDICATORS.contains(&rest) {
            let start = i;
            while i < lines.len()
                && (lines[i].trim().is_empty() || lines[i].starts_with([' ', '\t']))
            {
                i += 1;
            }
            let block: Vec<&str> = lines[start..i].iter().map(|l| l.trim()).collect();
            let text = if rest.starts_with('>') {
                fold(&block)
            } else {
                block.join("\n")
            };
            out.insert(key.to_string(), Value::String(text.trim_end().to_string()));
        } else if !rest.is_empty() {
            out.insert(key.to_string(), Value::String(unquote(rest)));
        }
    }
    out
}

/// Split at the first `:` that ends the line or is followed by whitespace.
fn split_key(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let idx = (0..bytes.len())
        .find(|&i| bytes[i] == b':' && bytes.get(i + 1).is_none_or(|b| b.is_ascii_whitespace()))?;
    let key = line[..idx].trim();
    (!key.is_empty()).then(|| (key, line[idx + 1..].trim()))
}

fn unquote(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if raw.len() >= 2 && bytes[0] == bytes[raw.len() - 1] && (bytes[0] == b'\'' || bytes[0] == b'"')
    {
        let inner = &raw[1..raw.len() - 1];
        return if bytes[0] == b'\'' {
            inner.replace("''", "'")
        } else {
            inner.replace("\\\"", "\"")
        };
    }
    match raw.find(" #") {
        Some(idx) => raw[..idx].trim_end().to_string(),
        None => raw.to_string(),
    }
}

fn fold(lines: &[&str]) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in lines {
        if line.is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join(" "));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join(" "));
    }
    paragraphs.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    fn strict(text: &str) -> Value {
        Value::Object(parse_strict(text).expect("valid yaml"))
    }

    #[test]
    fn split_returns_frontmatter_and_body() {
        assert_eq!(
            split_frontmatter("---\nname: x\n---\n# Hi\n"),
            ("name: x\n".into(), "# Hi\n".into())
        );
    }

    #[test]
    fn split_without_frontmatter_returns_whole_text_as_body() {
        assert_eq!(split_frontmatter("# Hi\n"), ("".into(), "# Hi\n".into()));
    }

    #[test]
    fn split_unterminated_frontmatter_is_treated_as_body() {
        let text = "---\nname: x\n# Hi\n";
        assert_eq!(split_frontmatter(text), ("".into(), text.into()));
    }

    #[test]
    fn split_handles_bom_and_crlf() {
        let (fm, body) = split_frontmatter("\u{feff}---\r\nname: x\r\n---\r\nBody\r\n");
        assert_eq!(strict(&fm), json!({"name": "x"}));
        assert_eq!(body, "Body\r\n");
    }

    #[test]
    fn quoted_scalars() {
        assert_eq!(
            strict("a: \"x: y\"\nb: 'it''s'\n"),
            json!({"a": "x: y", "b": "it's"})
        );
    }

    #[test]
    fn folded_block_scalar_joins_lines() {
        let text = "description: >\n  Use when the user\n  needs captions.\nname: x\n";
        assert_eq!(
            strict(text),
            json!({"description": "Use when the user needs captions.\n", "name": "x"})
        );
    }

    #[test]
    fn folded_block_scalar_keeps_paragraph_breaks() {
        assert_eq!(
            strict("d: >-\n  one\n  two\n\n  three\n")["d"],
            "one two\nthree"
        );
    }

    #[test]
    fn literal_block_scalar_keeps_newlines() {
        assert_eq!(strict("d: |-\n  one\n  two\n")["d"], "one\ntwo");
    }

    #[test]
    fn nested_maps_and_list_of_maps() {
        let text = "metadata:\n  girth:\n    sources:\n      - path-or-repo: ~/.agents/skills/hyperframes\n        ref: abc123\n        sections: [Captions, Timing]\n      - path-or-repo: owner/repo\n    crafted_for: caption timing\n    created: 2026-10-14\n";
        let girth = &strict(text)["metadata"]["girth"];
        assert_eq!(
            girth["sources"],
            json!([
                {"path-or-repo": "~/.agents/skills/hyperframes", "ref": "abc123", "sections": ["Captions", "Timing"]},
                {"path-or-repo": "owner/repo"}
            ])
        );
        assert_eq!(girth["crafted_for"], "caption timing");
        assert_eq!(girth["created"], "2026-10-14");
    }

    #[test]
    fn list_at_same_indent_as_key() {
        assert_eq!(
            strict("tags:\n- a\n- b\nname: x\n"),
            json!({"tags": ["a", "b"], "name": "x"})
        );
    }

    #[test]
    fn comments_and_scalar_types() {
        assert_eq!(
            strict("# c\n\nname: x  # trailing\na: true\nb: 3\nc: null\nd: []\ne:\n"),
            json!({"name": "x", "a": true, "b": 3, "c": null, "d": [], "e": null})
        );
    }

    #[test]
    fn strict_rejects_invalid_yaml_and_non_mappings() {
        assert!(parse_strict("description: Use when: x\n").is_err());
        assert!(parse_strict("- a\n- b\n").is_err());
        assert!(parse_strict("").is_err());
    }

    #[test]
    fn lenient_uses_strict_result_when_valid() {
        let parsed = parse_lenient("name: x\nmetadata:\n  k: v\n");
        assert_eq!(
            Value::Object(parsed.meta),
            json!({"name": "x", "metadata": {"k": "v"}})
        );
        assert!(!parsed.fell_back);
    }

    #[test]
    fn lenient_recovers_colons_in_plain_values() {
        let parsed = parse_lenient("name: girth\ndescription: Use when: you need it\n");
        assert_eq!(
            Value::Object(parsed.meta),
            json!({"name": "girth", "description": "Use when: you need it"})
        );
        assert!(parsed.fell_back);
    }

    #[test]
    fn lenient_recovers_quoted_and_block_values_beside_broken_lines() {
        let text = "name: 'my skill'\ndescription: >\n  Use when: the user\n  needs it.\nbroken: [unclosed\n\tindented: tab\n";
        let parsed = parse_lenient(text);
        assert_eq!(parsed.meta["name"], "my skill");
        assert_eq!(parsed.meta["description"], "Use when: the user needs it.");
        assert!(parsed.fell_back);
    }

    #[test]
    fn lenient_on_empty_or_non_mapping_is_empty() {
        assert!(parse_lenient("").meta.is_empty());
        assert!(parse_lenient("- a\n").meta.is_empty());
    }

    proptest! {
        #[test]
        fn lenient_never_panics(text in any::<String>()) {
            let _ = parse_lenient(&text);
        }

        #[test]
        fn flat_string_maps_round_trip(
            map in proptest::collection::btree_map("[a-z][a-z0-9_]{0,10}", "[A-Za-z][A-Za-z0-9 ,.]{0,30}", 0..5)
        ) {
            let map: std::collections::BTreeMap<String, String> = map
                .into_iter()
                .map(|(k, v)| (k, v.trim().to_string()))
                .filter(|(_, v)| !["true", "false", "null", "yes", "no", "on", "off", "y", "n"].contains(&v.to_lowercase().as_str()))
                .collect();
            let text: String = map.iter().map(|(k, v)| format!("{k}: {v}\n")).collect();
            let parsed = parse_lenient(&text);
            let expected: Map<String, Value> = map.into_iter().map(|(k, v)| (k, Value::String(v))).collect();
            prop_assert_eq!(parsed.meta, expected);
        }
    }
}
