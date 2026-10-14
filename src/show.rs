//! `girth show`: print only the sections of a skill that a need touches.
//!
//! Source skills usually live outside the project, where reading them file by
//! file costs permission prompts and whole-file tokens. `show` prints just the
//! frontmatter and the requested sections, through the one launcher command.

use std::path::Path;

use crate::sections::{heading_text, slug};
use crate::skill::{markdown_files, read_lossy};

/// ATX heading level (1–6) of a line, if it is a heading.
fn heading_level(line: &str) -> Option<usize> {
    let line = line
        .strip_prefix("   ")
        .or_else(|| line.strip_prefix("  "))
        .or_else(|| line.strip_prefix(' '))
        .unwrap_or(line);
    heading_text(line).map(|_| line.bytes().take_while(|&b| b == b'#').count())
}

fn is_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// Sections of `markdown` whose heading slug is in `wanted`, each with its
/// subsections, up to the next heading of the same or a higher level.
/// Headings inside fenced code are ignored.
pub fn matching_sections(markdown: &str, wanted: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    // (level of the matched heading, text so far)
    let mut current: Option<(usize, String)> = None;
    let mut in_fence = false;
    for line in markdown.lines() {
        let level = if in_fence { None } else { heading_level(line) };
        if is_fence(line) {
            in_fence = !in_fence;
        }
        if let Some(level) = level {
            if current.as_ref().is_some_and(|(open, _)| level <= *open) {
                found.extend(current.take().map(|(_, text)| text));
            }
            let text = heading_text(line.trim_start()).unwrap_or_default();
            if current.is_none() && wanted.contains(&slug(&text)) {
                current = Some((level, String::new()));
            }
        }
        if let Some((_, text)) = current.as_mut() {
            text.push_str(line);
            text.push('\n');
        }
    }
    found.extend(current.map(|(_, text)| text));
    found
}

/// The text `girth show` prints for a skill dir. With no headings: the
/// frontmatter plus a table of contents per file. Otherwise the matching
/// sections per file, and a `not found:` line for headings that matched nothing.
pub fn show(dir: &Path, headings: &[String]) -> Result<String, String> {
    use std::fmt::Write;
    let skill_md = dir.join("SKILL.md");
    let Some(main) = read_lossy(&skill_md) else {
        return Err(format!("no SKILL.md in {}", dir.display()));
    };
    let mut files = Vec::new();
    markdown_files(dir, &mut files);
    // SKILL.md first, then the rest in path order.
    files.sort_by_key(|f| *f != skill_md);
    let rel = |f: &Path| {
        f.strip_prefix(dir)
            .unwrap_or(f)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let mut out = String::new();
    if headings.is_empty() {
        let (frontmatter, _) = crate::frontmatter::split_frontmatter(&main);
        let _ = writeln!(out, "---\n{}\n---", frontmatter.trim_matches('\n'));
        for file in &files {
            let text = read_lossy(file).unwrap_or_default();
            let (_, body) = crate::frontmatter::split_frontmatter(&text);
            let names = crate::skill::extract_headings(&body);
            if !names.is_empty() {
                let _ = writeln!(out, "{}: {}", rel(file), names.join(" · "));
            }
        }
        return Ok(out);
    }
    let wanted: Vec<String> = headings.iter().map(|h| slug(h)).collect();
    let mut hit = vec![false; wanted.len()];
    for file in &files {
        let text = read_lossy(file).unwrap_or_default();
        let (_, body) = crate::frontmatter::split_frontmatter(&text);
        let sections = matching_sections(&body, &wanted);
        if sections.is_empty() {
            continue;
        }
        for section in &sections {
            let first = section.lines().next().unwrap_or_default().trim_start();
            let s = slug(&heading_text(first).unwrap_or_default());
            for (i, w) in wanted.iter().enumerate() {
                hit[i] |= *w == s;
            }
        }
        let _ = writeln!(out, "=== {} ===", rel(file));
        out.push_str(&sections.concat());
    }
    for (heading, found) in headings.iter().zip(hit) {
        if !found {
            let _ = writeln!(out, "not found: {heading}");
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::write_skill;
    use proptest::prelude::*;
    use tempfile::TempDir;

    const DOC: &str = "\
# Mega
intro
## Caption timing
Two lines max.
### Edge cases
Hold 1.2 s.
```md
## Not a heading
```
## Audio mixing
Duck 12 dB.
#### Deep
deep text
# Appendix
end
";

    #[test]
    fn levels() {
        assert_eq!(heading_level("## A"), Some(2));
        assert_eq!(heading_level("###### F"), Some(6));
        assert_eq!(heading_level("####### seven"), None);
        assert_eq!(heading_level("#nospace"), None);
        assert_eq!(heading_level("text # no"), None);
        assert_eq!(heading_level("   ## indented"), Some(2));
    }

    #[test]
    fn a_section_keeps_its_subsections_and_fenced_lines() {
        let got = matching_sections(DOC, &["caption-timing".into()]);
        assert_eq!(
            got,
            vec![
                "## Caption timing\nTwo lines max.\n### Edge cases\nHold 1.2 s.\n```md\n## Not a heading\n```\n"
                    .to_string()
            ]
        );
    }

    #[test]
    fn a_section_stops_at_a_higher_level_heading() {
        let got = matching_sections(DOC, &["audio-mixing".into()]);
        assert_eq!(
            got,
            vec!["## Audio mixing\nDuck 12 dB.\n#### Deep\ndeep text\n".to_string()]
        );
        let deep = matching_sections(DOC, &["deep".into()]);
        assert_eq!(deep, vec!["#### Deep\ndeep text\n".to_string()]);
    }

    #[test]
    fn several_and_no_matches() {
        assert_eq!(
            matching_sections(DOC, &["appendix".into(), "edge-cases".into()]).len(),
            2
        );
        assert!(matching_sections(DOC, &["not-a-heading".into()]).is_empty());
        assert!(matching_sections(DOC, &[]).is_empty());
    }

    #[test]
    fn show_without_headings_is_frontmatter_plus_contents() {
        let tmp = TempDir::new().unwrap();
        let dir = write_skill(
            tmp.path(),
            "video-mega",
            "name: video-mega\ndescription: All video.",
            DOC,
            &[("references/export.md", "## Export settings\nH.264\n")],
        );
        let out = show(&dir, &[]).unwrap();
        assert!(
            out.starts_with("---\nname: video-mega\ndescription: All video.\n---\n"),
            "{out}"
        );
        assert!(
            out.contains(
                "SKILL.md: Mega · Caption timing · Edge cases · Audio mixing · Deep · Appendix\n"
            ),
            "{out}"
        );
        assert!(
            out.contains("references/export.md: Export settings\n"),
            "{out}"
        );
        assert!(!out.contains("Duck 12 dB"), "no bodies without --heading");
    }

    #[test]
    fn show_with_headings_prints_matches_per_file_and_misses() {
        let tmp = TempDir::new().unwrap();
        let dir = write_skill(
            tmp.path(),
            "video-mega",
            "name: video-mega\ndescription: All video.",
            DOC,
            &[("references/export.md", "## Export settings\nH.264\n")],
        );
        let out = show(
            &dir,
            &[
                "Caption timing".into(),
                "export settings!".into(),
                "Nope".into(),
            ],
        )
        .unwrap();
        assert!(
            out.contains("=== SKILL.md ===\n## Caption timing\n"),
            "{out}"
        );
        assert!(
            out.contains("=== references/export.md ===\n## Export settings\nH.264\n"),
            "{out}"
        );
        assert!(out.ends_with("not found: Nope\n"), "{out}");
        assert!(!out.contains("Duck 12 dB"), "{out}");
        assert!(
            !out.contains("description: All video."),
            "frontmatter only in the overview"
        );
    }

    #[test]
    fn yaml_comments_in_frontmatter_are_not_sections() {
        let tmp = TempDir::new().unwrap();
        let dir = write_skill(
            tmp.path(),
            "s",
            "name: s\n# Secret\ndescription: d",
            "## Real\nr\n",
            &[],
        );
        assert!(
            show(&dir, &["Secret".into()])
                .unwrap()
                .contains("not found: Secret")
        );
    }

    #[test]
    fn show_rejects_a_dir_without_skill_md() {
        let tmp = TempDir::new().unwrap();
        assert!(show(tmp.path(), &[]).unwrap_err().contains("no SKILL.md"));
    }

    proptest! {
        #[test]
        fn never_panics_and_only_returns_text_from_the_input(md in "(#{0,7} ?[a-z ]{0,8}\n|```\n|[a-z ]{0,10}\n){0,30}", w in "[a-z-]{0,6}") {
            for section in matching_sections(&md, &[w]) {
                prop_assert!(md.contains(section.trim_end_matches('\n')));
            }
        }
    }
}
