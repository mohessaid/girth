//! Discovering installed skills and sizing what they cost in context.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::frontmatter::{parse_lenient, split_frontmatter};

pub const CRAFTED_PREFIX: &str = "girth-";

/// One discovered skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub skill_md_bytes: u64,
    pub total_md_bytes: u64,
    /// `(file relative to the skill dir, heading text)`.
    pub headings: Vec<(String, String)>,
    /// Frontmatter was not valid YAML and was recovered leniently.
    pub fell_back: bool,
    /// Frontmatter carries a `metadata.girth` block (Girth wrote this skill).
    pub girth_meta: bool,
}

impl SkillInfo {
    /// Bytes of name + description, which sit in context every turn.
    pub fn always_on_bytes(&self) -> u64 {
        (self.name.len() + self.description.len()) as u64
    }

    /// Crafted by Girth: the `girth-` prefix alone could be a third-party skill.
    pub fn crafted(&self) -> bool {
        self.girth_meta && self.name.starts_with(CRAFTED_PREFIX)
    }
}

pub fn approx_tokens(bytes: u64) -> u64 {
    bytes.div_ceil(4)
}

/// ATX headings (`#`..`######`), skipping fenced code blocks.
pub fn extract_headings(markdown: &str) -> Vec<String> {
    let mut headings = Vec::new();
    let mut in_fence = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            headings.extend(crate::sections::heading_text(line));
        }
    }
    headings
}

pub(crate) fn read_lossy(path: &Path) -> Option<String> {
    std::fs::read(path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// Markdown files under `dir`, sorted; symlinked subdirectories are not entered.
pub(crate) fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            markdown_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") && path.is_file() {
            out.push(path);
        }
    }
}

pub fn load_skill(dir: &Path) -> Option<SkillInfo> {
    let skill_md = dir.join("SKILL.md");
    if !skill_md.is_file() {
        return None;
    }
    let (frontmatter, body) = split_frontmatter(&read_lossy(&skill_md)?);
    let parsed = parse_lenient(&frontmatter);
    let text = |key: &str| parsed.meta.get(key).and_then(Value::as_str).map(str::trim);
    let name = text("name")
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            dir.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    let description = text("description")
        .map(|d| d.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    let mut headings: Vec<(String, String)> = extract_headings(&body)
        .into_iter()
        .map(|h| ("SKILL.md".to_string(), h))
        .collect();
    let mut files = Vec::new();
    markdown_files(dir, &mut files);
    let mut total = 0;
    for file in files {
        let Ok(meta) = std::fs::metadata(&file) else {
            continue;
        };
        total += meta.len();
        let rel = file
            .strip_prefix(dir)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        if rel != "SKILL.md" {
            let content = read_lossy(&file).unwrap_or_default();
            headings.extend(
                extract_headings(&content)
                    .into_iter()
                    .map(|h| (rel.clone(), h)),
            );
        }
    }
    Some(SkillInfo {
        name,
        description,
        path: dir.to_path_buf(),
        skill_md_bytes: std::fs::metadata(&skill_md).ok()?.len(),
        total_md_bytes: total,
        headings,
        fell_back: parsed.fell_back,
        girth_meta: parsed
            .meta
            .get("metadata")
            .and_then(|m| m.get("girth"))
            .is_some_and(Value::is_object),
    })
}

/// Skills one level under each root, in root order then name.
///
/// Deduped by real path (symlinks) and by name (copies): the first root wins,
/// mirroring how the harness shadows same-named skills.
pub fn discover_skills(roots: &[PathBuf]) -> Vec<SkillInfo> {
    let mut skills = Vec::new();
    let mut seen_paths = HashSet::new();
    let mut seen_names = HashSet::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        children.sort();
        for child in children {
            let Ok(real) = child.canonicalize() else {
                continue;
            };
            if !real.is_dir() || seen_paths.contains(&real) {
                continue;
            }
            if let Some(info) = load_skill(&child) {
                if seen_names.insert(info.name.clone()) {
                    seen_paths.insert(real);
                    skills.push(info);
                }
            }
        }
    }
    skills
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::write_skill;
    use tempfile::TempDir;

    fn names(skills: &[SkillInfo]) -> Vec<&str> {
        skills.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn load_skill_reads_name_description_sizes_and_headings() {
        let tmp = TempDir::new().unwrap();
        let d = write_skill(
            tmp.path(),
            "vid",
            "name: video-mega\ndescription: >\n  Everything about\n  video.",
            "# Video\n## Caption timing\ntext\n",
            &[
                ("references/audio.md", "# Audio mixing\n## Ducking\n"),
                ("notes.txt", "xxxx"),
            ],
        );
        let s = load_skill(&d).unwrap();
        assert_eq!(s.name, "video-mega");
        assert_eq!(s.description, "Everything about video.");
        let skill_md = std::fs::metadata(d.join("SKILL.md")).unwrap().len();
        let audio = std::fs::metadata(d.join("references/audio.md"))
            .unwrap()
            .len();
        assert_eq!(s.skill_md_bytes, skill_md);
        assert_eq!(s.total_md_bytes, skill_md + audio);
        assert!(
            s.headings
                .contains(&("SKILL.md".into(), "Caption timing".into()))
        );
        assert!(
            s.headings
                .contains(&("references/audio.md".into(), "Ducking".into()))
        );
        assert_eq!(
            s.always_on_bytes(),
            ("video-mega".len() + "Everything about video.".len()) as u64
        );
        assert!(!s.crafted() && !s.fell_back);
    }

    #[test]
    fn crafted_flag_and_multibyte_always_on() {
        let tmp = TempDir::new().unwrap();
        let s = load_skill(&write_skill(
            tmp.path(),
            "girth-x",
            "name: girth-x\ndescription: café\nmetadata:\n  girth:\n    created: 2026-10-14",
            "",
            &[],
        ))
        .unwrap();
        assert!(s.crafted());
        assert_eq!(s.always_on_bytes(), (7 + "café".len()) as u64);
    }

    #[test]
    fn girth_prefix_without_girth_metadata_is_not_crafted() {
        let tmp = TempDir::new().unwrap();
        let s = load_skill(&write_skill(
            tmp.path(),
            "girth-lookalike",
            "name: girth-lookalike\ndescription: third-party skill",
            "",
            &[],
        ))
        .unwrap();
        assert!(!s.crafted());
    }

    #[test]
    fn load_skill_falls_back_to_dir_name_and_empty_description() {
        let tmp = TempDir::new().unwrap();
        let s = load_skill(&write_skill(
            tmp.path(),
            "nameless",
            "license: MIT",
            "",
            &[],
        ))
        .unwrap();
        assert_eq!((s.name.as_str(), s.description.as_str()), ("nameless", ""));
    }

    #[test]
    fn load_skill_recovers_invalid_yaml_leniently() {
        let tmp = TempDir::new().unwrap();
        let d = write_skill(
            tmp.path(),
            "x",
            "name: loose\ndescription: Use when: you need it",
            "",
            &[],
        );
        let s = load_skill(&d).unwrap();
        assert_eq!(
            (s.name.as_str(), s.description.as_str()),
            ("loose", "Use when: you need it")
        );
        assert!(s.fell_back);
    }

    #[test]
    fn load_skill_returns_none_without_skill_md() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("empty")).unwrap();
        assert!(load_skill(&tmp.path().join("empty")).is_none());
    }

    #[test]
    fn load_skill_tolerates_invalid_utf8() {
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("bad");
        std::fs::create_dir(&d).unwrap();
        std::fs::write(d.join("SKILL.md"), b"---\nname: bad\n---\n\xff\xfe body").unwrap();
        assert_eq!(load_skill(&d).unwrap().name, "bad");
    }

    #[test]
    fn headings_inside_code_fences_are_ignored() {
        let md = "# Top\n```\n# not a heading\n```\n~~~\n## nor this\n~~~\n## Real ##\n#nospace\n####### seven\n";
        assert_eq!(extract_headings(md), vec!["Top", "Real"]);
    }

    #[test]
    fn headings_drop_provenance_markers() {
        assert_eq!(
            extract_headings("## Timing <!-- src: a#b -->\n"),
            vec!["Timing"]
        );
    }

    #[test]
    fn discover_one_level_deep_in_root_then_name_order() {
        let tmp = TempDir::new().unwrap();
        let (r1, r2) = (tmp.path().join("r1"), tmp.path().join("r2"));
        write_skill(&r1, "b", "name: b", "", &[]);
        write_skill(&r1, "a", "name: a", "", &[]);
        write_skill(&r2, "c", "name: c", "", &[]);
        write_skill(&r1.join("a"), "nested", "name: nested", "", &[]);
        assert_eq!(names(&discover_skills(&[r2, r1])), vec!["c", "a", "b"]);
    }

    #[test]
    fn discover_skips_missing_roots_files_and_dirs_without_skill_md() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("r");
        write_skill(&root, "ok", "name: ok", "", &[]);
        std::fs::create_dir(root.join("no-skill")).unwrap();
        std::fs::write(root.join("file.md"), "x").unwrap();
        assert_eq!(
            names(&discover_skills(&[tmp.path().join("missing"), root])),
            vec!["ok"]
        );
    }

    #[test]
    fn discover_dedupes_symlinked_skills() {
        let tmp = TempDir::new().unwrap();
        let real = write_skill(
            &tmp.path().join("agents"),
            "hf",
            "name: hf\ndescription: d",
            "",
            &[],
        );
        std::fs::create_dir(tmp.path().join("claude")).unwrap();
        std::os::unix::fs::symlink(&real, tmp.path().join("claude/hf")).unwrap();
        let skills = discover_skills(&[tmp.path().join("claude"), tmp.path().join("agents")]);
        assert_eq!(names(&skills), vec!["hf"]);
        assert_eq!(skills[0].path, tmp.path().join("claude/hf"));
    }

    #[test]
    fn discover_dedupes_copies_by_name_first_root_wins() {
        let tmp = TempDir::new().unwrap();
        write_skill(
            &tmp.path().join("claude"),
            "hf",
            "name: hf\ndescription: copy",
            "",
            &[],
        );
        write_skill(
            &tmp.path().join("agents"),
            "hf",
            "name: hf\ndescription: original",
            "",
            &[],
        );
        let skills = discover_skills(&[tmp.path().join("claude"), tmp.path().join("agents")]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].description, "copy");
    }

    #[test]
    fn discover_skips_broken_symlinks() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("r");
        write_skill(&root, "ok", "name: ok", "", &[]);
        std::os::unix::fs::symlink(tmp.path().join("nowhere"), root.join("dangling")).unwrap();
        assert_eq!(names(&discover_skills(&[root])), vec!["ok"]);
    }

    #[test]
    fn approx_tokens_rounds_up() {
        assert_eq!([0, 1, 4, 5].map(approx_tokens), [0, 1, 1, 2]);
    }
}
