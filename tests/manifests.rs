//! The plugin manifests for each harness agree with the crate and point at the shipped skill.

use std::path::Path;

fn manifest(path: &str) -> serde_json::Value {
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let text = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn every_manifest_has_the_crate_version() {
    for path in [
        ".claude-plugin/plugin.json",
        ".codex-plugin/plugin.json",
        ".cursor-plugin/plugin.json",
        "gemini-extension.json",
    ] {
        let m = manifest(path);
        assert_eq!(m["name"], "girth", "{path}");
        assert_eq!(m["version"], env!("CARGO_PKG_VERSION"), "{path}");
    }
}

#[test]
fn codex_and_cursor_plugins_ship_the_skills_dir() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in [".codex-plugin/plugin.json", ".cursor-plugin/plugin.json"] {
        let skills = manifest(path)["skills"].as_str().unwrap().to_string();
        assert!(
            root.join(&skills).join("girth/SKILL.md").is_file(),
            "{path}: {skills}"
        );
    }
    // Gemini CLI and Antigravity load an extension's skills/ dir by convention.
    assert!(root.join("skills/girth/SKILL.md").is_file());
}
