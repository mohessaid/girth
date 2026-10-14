//! Test helpers shared by unit tests.

use std::path::{Path, PathBuf};

/// Write `<root>/<dirname>/SKILL.md` as `---\n{frontmatter}\n---\n{body}` plus extra files.
pub fn write_skill(
    root: &Path,
    dirname: &str,
    frontmatter: &str,
    body: &str,
    files: &[(&str, &str)],
) -> PathBuf {
    let dir = root.join(dirname);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\n{}\n---\n{body}", frontmatter.trim_matches('\n')),
    )
    .unwrap();
    for (rel, content) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    dir
}
