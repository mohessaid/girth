//! One project cache across harnesses: a crafted skill in `<project>/.claude/skills`
//! (read by Claude Code) is linked into `<project>/.agents/skills` (read by most other
//! harnesses), and the reverse, so a skill crafted in one loads in both.
//!
//! The link is relative (`../../.claude/skills/<name>`), so it survives moving or
//! cloning the project. An existing entry on the other side is never replaced.

use std::io;
use std::path::{Path, PathBuf};

const SIDES: [&str; 2] = [".claude", ".agents"];

/// Where `dir`'s twin goes and what it points at, when `dir` sits directly in one
/// side's `skills` folder; `None` for any other layout.
fn twin(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let name = dir.file_name()?;
    let skills = dir.parent()?;
    if skills.file_name()? != "skills" {
        return None;
    }
    let side = skills.parent()?;
    let here = side.file_name()?.to_str()?;
    let other = SIDES
        .iter()
        .find(|s| SIDES.contains(&here) && **s != here)?;
    let project = side.parent()?;
    let at = project.join(other).join("skills").join(name);
    let target = Path::new("..")
        .join("..")
        .join(here)
        .join("skills")
        .join(name);
    Some((at, target))
}

/// Link `dir` into the other side's `skills` folder; returns the link's path when one
/// was made. Nothing is made when `dir` isn't in either side's `skills` folder, or when
/// the other side already has an entry by that name.
pub fn link_other_side(dir: &Path) -> io::Result<Option<PathBuf>> {
    let Some((at, target)) = twin(dir) else {
        return Ok(None);
    };
    if at.symlink_metadata().is_ok() {
        return Ok(None);
    }
    if let Some(parent) = at.parent() {
        std::fs::create_dir_all(parent)?;
    }
    symlink_dir(&target, &at)?;
    Ok(Some(at))
}

#[cfg(unix)]
fn symlink_dir(target: &Path, at: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(windows)]
fn symlink_dir(target: &Path, at: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, at)
}

#[cfg(not(any(unix, windows)))]
fn symlink_dir(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no symlinks here",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn skill(project: &Path, side: &str, name: &str) -> PathBuf {
        let dir = project.join(side).join("skills").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "---\nname: x\n---\nbody\n").unwrap();
        dir
    }

    #[test]
    fn a_claude_skill_is_linked_for_other_harnesses() {
        let tmp = TempDir::new().unwrap();
        let dir = skill(tmp.path(), ".claude", "girth-x");
        let at = link_other_side(&dir).unwrap().unwrap();
        assert_eq!(at, tmp.path().join(".agents/skills/girth-x"));
        assert_eq!(
            std::fs::read_link(&at).unwrap(),
            Path::new("../../.claude/skills/girth-x")
        );
        let through = std::fs::read_to_string(at.join("SKILL.md")).unwrap();
        assert!(through.contains("body"));
    }

    #[test]
    fn an_agents_skill_is_linked_for_claude_code() {
        let tmp = TempDir::new().unwrap();
        let dir = skill(tmp.path(), ".agents", "girth-y");
        let at = link_other_side(&dir).unwrap().unwrap();
        assert_eq!(at, tmp.path().join(".claude/skills/girth-y"));
        assert_eq!(
            std::fs::read_link(&at).unwrap(),
            Path::new("../../.agents/skills/girth-y")
        );
        // Linking again finds the twin and makes nothing.
        assert_eq!(link_other_side(&dir).unwrap(), None);
        assert_eq!(link_other_side(&at).unwrap(), None);
    }

    #[test]
    fn an_existing_entry_on_the_other_side_is_left_alone() {
        let tmp = TempDir::new().unwrap();
        let dir = skill(tmp.path(), ".claude", "girth-z");
        let theirs = skill(tmp.path(), ".agents", "girth-z");
        assert_eq!(link_other_side(&dir).unwrap(), None);
        assert!(!theirs.symlink_metadata().unwrap().file_type().is_symlink());
        // A dangling link counts as an entry too.
        let tmp = TempDir::new().unwrap();
        let dir = skill(tmp.path(), ".claude", "girth-d");
        std::fs::create_dir_all(tmp.path().join(".agents/skills")).unwrap();
        std::os::unix::fs::symlink("nowhere", tmp.path().join(".agents/skills/girth-d")).unwrap();
        assert_eq!(link_other_side(&dir).unwrap(), None);
    }

    #[test]
    fn other_layouts_get_no_link() {
        let tmp = TempDir::new().unwrap();
        for side in [".cursor", "skills-src", ".claude/other"] {
            let dir = skill(tmp.path(), side, "girth-x");
            assert_eq!(link_other_side(&dir).unwrap(), None, "{side}");
        }
        let loose = tmp.path().join("girth-x");
        std::fs::create_dir_all(&loose).unwrap();
        assert_eq!(link_other_side(&loose).unwrap(), None);
        assert_eq!(link_other_side(Path::new("girth-x")).unwrap(), None);
        assert!(!tmp.path().join(".agents").exists());
    }

    #[test]
    fn a_link_that_cannot_be_made_is_an_error() {
        let tmp = TempDir::new().unwrap();
        let dir = skill(tmp.path(), ".claude", "girth-x");
        std::fs::write(tmp.path().join(".agents"), "a file, not a folder").unwrap();
        assert!(link_other_side(&dir).is_err());
    }
}
