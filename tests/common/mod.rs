use std::path::{Path, PathBuf};

pub fn write_skill(root: &Path, dirname: &str, frontmatter: &str, body: &str) -> PathBuf {
    let dir = root.join(dirname);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\n{frontmatter}\n---\n{body}"),
    )
    .unwrap();
    dir
}
