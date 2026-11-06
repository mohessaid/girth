//! Every agent harness that reads Agent Skills, and the folders it reads them from.

/// One harness: its `skills` CLI id, its project skill dir and its user skill dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Harness {
    pub id: &'static str,
    pub project: Option<&'static str>,
    pub user: Option<&'static str>,
}

include!("harness_table.rs");

/// User skill dirs the harnesses read beyond the `skills` CLI's table (checked
/// on disk or in their docs, 2026-11-06).
pub const EXTRA_USER_DIRS: [&str; 3] = [
    "~/.codex/skills",
    "~/.gemini/antigravity-cli/skills",
    "~/.config/opencode/skills",
];

/// Every dir a harness reads skills from, first occurrence kept. Project dirs
/// count only when they are dot-dirs: `skills/` or `agent/skills/` in an
/// arbitrary repo is somebody's code, not a skill root.
pub fn skill_dirs() -> Vec<&'static str> {
    let mut dirs: Vec<&'static str> = Vec::new();
    let candidates = HARNESSES
        .iter()
        .flat_map(|h| [h.user, h.project.filter(|p| p.starts_with('.'))])
        .flatten()
        .chain(EXTRA_USER_DIRS);
    for dir in candidates {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(id: &str) -> Harness {
        *HARNESSES.iter().find(|h| h.id == id).unwrap()
    }

    #[test]
    fn table_covers_upstream() {
        assert!(HARNESSES.len() >= 66, "{}", HARNESSES.len());
        assert_eq!(find("codex").project, Some(".agents/skills"));
        assert_eq!(find("codex").user, Some("~/.agents/skills"));
        assert_eq!(find("cursor").user, Some("~/.cursor/skills"));
        assert_eq!(find("gemini-cli").user, Some("~/.gemini/skills"));
        assert_eq!(find("github-copilot").user, Some("~/.copilot/skills"));
        assert_eq!(find("claude-code").project, Some(".claude/skills"));
        assert_eq!(find("eve").user, None);
        assert!(HARNESSES.iter().all(|h| {
            h.id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && !h.id.starts_with('-')
        }));
    }

    #[test]
    fn non_dot_project_dirs_are_ignored() {
        let dirs = skill_dirs();
        for generic in ["skills", "data/skills", "agent/skills"] {
            assert!(!dirs.contains(&generic), "{generic}");
        }
        assert!(dirs.contains(&".agents/skills"));
        assert!(dirs.contains(&"~/.codex/skills"));
        assert!(dirs.contains(&"~/.astrbot/data/skills"));
    }

    #[test]
    fn skill_dirs_are_unique() {
        let dirs = skill_dirs();
        let mut sorted = dirs.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), dirs.len());
    }
}
