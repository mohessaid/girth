//! Markdown sections: splitting, slugs, content hashes and provenance markers.

use sha2::{Digest, Sha256};

/// One section of a skill body: an ATX heading plus its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Heading text without `#`s or HTML comments.
    pub heading: String,
    pub slug: String,
    /// Provenance from `<!-- src: skill#slug, ... -->` on the heading line.
    pub src: Vec<String>,
    /// First 16 hex chars of SHA-256 over the normalised heading + content.
    pub hash: String,
}

pub fn strip_html_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start + 4..].find("-->") {
            Some(end) => rest = &rest[start + 4 + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

pub fn slug(heading: &str) -> String {
    let lower = strip_html_comments(heading).to_lowercase();
    let parts: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    parts.join("-")
}

/// Lowercase, drop HTML comments, markdown punctuation and list markers, collapse whitespace.
pub fn normalize(text: &str) -> String {
    let cleaned: String = strip_html_comments(text)
        .to_lowercase()
        .chars()
        .map(|c| if "#*_`>|".contains(c) { ' ' } else { c })
        .collect();
    let mut words: Vec<&str> = Vec::new();
    for line in cleaned.lines() {
        let line = line.trim_start();
        let line = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("+ "))
            .unwrap_or(line);
        words.extend(line.split_whitespace());
    }
    words.join(" ")
}

pub fn section_hash(heading_line: &str, content: &str) -> String {
    let digest = Sha256::digest(normalize(&format!("{heading_line}\n{content}")).as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Identity of a whole skill: hash of its sorted section hashes.
pub fn fingerprint(section_hashes: &[String]) -> String {
    let mut sorted: Vec<&str> = section_hashes.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    let digest = Sha256::digest(sorted.join("\n").as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub fn parse_src(heading_line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = heading_line;
    while let Some(start) = rest.find("<!--") {
        let after = &rest[start + 4..];
        let Some(end) = after.find("-->") else { break };
        if let Some(list) = after[..end].trim().strip_prefix("src:") {
            out.extend(
                list.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            );
        }
        rest = &after[end + 3..];
    }
    out
}

/// `Some(heading text)` when `line` is an ATX heading.
pub(crate) fn heading_text(line: &str) -> Option<String> {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) || !line[hashes..].starts_with([' ', '\t']) {
        return None;
    }
    let text = strip_html_comments(&line[hashes..]);
    let text = text.trim().trim_end_matches('#').trim_end();
    (!text.is_empty()).then(|| text.to_string())
}

pub fn split_sections(body: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current: Option<(String, String, Vec<&str>)> = None;
    let mut in_fence = false;
    let finish = |(line, heading, content): (String, String, Vec<&str>)| Section {
        slug: slug(&heading),
        src: parse_src(&line),
        hash: section_hash(&line, &content.join("\n")),
        heading,
    };
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            if let Some(heading) = heading_text(line) {
                if let Some(done) = current.take() {
                    sections.push(finish(done));
                }
                current = Some((line.to_string(), heading, Vec::new()));
                continue;
            }
        }
        if let Some((_, _, content)) = current.as_mut() {
            content.push(line);
        }
    }
    if let Some(done) = current.take() {
        sections.push(finish(done));
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn splits_at_each_heading_and_ignores_preamble_and_fenced_headings() {
        let body = "intro text\n# One\nalpha\n```\n# not a heading\n```\n## Two\nbeta\n";
        let sections = split_sections(body);
        let headings: Vec<&str> = sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(headings, ["One", "Two"]);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Caption timing"), "caption-timing");
        assert_eq!(slug("C# & Rust!"), "c-rust");
        assert_eq!(slug("  2. One caption track  "), "2-one-caption-track");
        assert_eq!(slug("Timing <!-- src: a#b -->"), "timing");
    }

    #[test]
    fn strip_html_comments_removes_all_comments() {
        assert_eq!(strip_html_comments("a <!-- x --> b <!--y-->c"), "a  b c");
        assert_eq!(strip_html_comments("unclosed <!-- x"), "unclosed ");
    }

    #[test]
    fn hash_is_stable_16_hex_and_ignores_case_whitespace_markup_and_comments() {
        let a = section_hash(
            "## Caption timing",
            "Never show **more** than 2 lines.\n- Each caption 1.2s",
        );
        let b = section_hash(
            "##   caption TIMING <!-- src: v#c -->",
            "never show more than 2 lines.\n\n* each   caption 1.2s",
        );
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(
            a,
            section_hash("## Caption timing", "Never show more than 3 lines.")
        );
    }

    #[test]
    fn parse_src_reads_markers() {
        assert_eq!(
            parse_src("## Timing <!-- src: video-mega#caption-timing -->"),
            ["video-mega#caption-timing"]
        );
        assert_eq!(parse_src("## T <!--src:a#x,  b#y -->"), ["a#x", "b#y"]);
        assert!(parse_src("## Timing").is_empty());
        assert!(parse_src("## Timing <!-- note: hi -->").is_empty());
    }

    #[test]
    fn parse_src_reads_past_other_comments_on_the_line() {
        assert_eq!(parse_src("## T <!----> <!-- src: a#b -->"), ["a#b"]);
        assert_eq!(
            parse_src("## T <!-- note --> <!-- src: a#b --> <!-- src: c#d -->"),
            ["a#b", "c#d"]
        );
    }

    #[test]
    fn sections_carry_heading_slug_src_and_hash() {
        let s =
            &split_sections("## Caption timing <!-- src: video-mega#caption-timing -->\nRule.\n")
                [0];
        assert_eq!(s.heading, "Caption timing");
        assert_eq!(s.slug, "caption-timing");
        assert_eq!(s.src, ["video-mega#caption-timing"]);
        assert_eq!(s.hash, section_hash("## Caption timing", "Rule."));
    }

    #[test]
    fn fingerprint_is_order_independent_and_16_hex() {
        let a = fingerprint(&["b".to_string(), "a".to_string()]);
        assert_eq!(a, fingerprint(&["a".to_string(), "b".to_string()]));
        assert_eq!(a.len(), 16);
        assert_ne!(a, fingerprint(&["a".to_string()]));
    }

    #[test]
    fn normalize_collapses_markup_and_whitespace() {
        assert_eq!(
            normalize("# Hello   **World**\n> - `x` | y_z"),
            "hello world x y z"
        );
    }

    proptest! {
        #[test]
        fn never_panics(body in any::<String>()) {
            let _ = split_sections(&body);
        }

        #[test]
        fn hash_invariant_under_case_and_whitespace(words in proptest::collection::vec("[a-zA-Z0-9]{1,8}", 1..10)) {
            let plain = words.join(" ");
            let noisy = words.iter().map(|w| w.to_uppercase()).collect::<Vec<_>>().join("  \n\t ");
            prop_assert_eq!(section_hash("## H", &plain), section_hash("## h", &noisy));
        }
    }
}
