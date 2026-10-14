//! Where to look for skills and where crafted skills go.

use std::collections::HashMap;
use std::path::{Component, PathBuf};

use serde_json::{Map, Value};

pub const ROOTS_ENV: &str = "GIRTH_SKILL_ROOTS";
pub const OUTPUT_ENV: &str = "GIRTH_OUTPUT_DIR";
pub const CONFIG_PATH: &str = ".girth/config.json";
pub const DEFAULT_ROOTS: [&str; 3] = ["~/.claude/skills", "~/.agents/skills", ".claude/skills"];
pub const DEFAULT_OUTPUT: &str = ".claude/skills";

/// Process context the resolvers read: cwd, home and environment.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
    pub env: HashMap<String, String>,
}

impl Context {
    pub fn from_process() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            home: std::env::var_os("HOME").map(PathBuf::from),
            env: std::env::vars().collect(),
        }
    }

    fn var(&self, key: &str) -> Option<&str> {
        self.env
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }

    /// Expand `~`, anchor relative paths at cwd, and drop `.` components.
    fn to_path(&self, raw: &str) -> PathBuf {
        let expanded = match (raw.strip_prefix('~'), &self.home) {
            (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
                home.join(rest.trim_start_matches('/'))
            }
            _ => PathBuf::from(raw),
        };
        let anchored = if expanded.is_absolute() {
            expanded
        } else {
            self.cwd.join(expanded)
        };
        anchored
            .components()
            .filter(|c| *c != Component::CurDir)
            .collect()
    }

    fn config(&self, warnings: &mut Vec<String>) -> Map<String, Value> {
        let path = self.cwd.join(CONFIG_PATH);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Map::new();
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => map,
            _ => {
                // Several resolvers read the config; say it once.
                let warning = format!("girth: ignoring invalid {}", path.display());
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
                Map::new()
            }
        }
    }
}

/// Skill roots: `--roots`, then `GIRTH_SKILL_ROOTS`, then config `roots`, then defaults.
pub fn resolve_roots(cli: &[String], ctx: &Context, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    let raw: Vec<String> = if !cli.is_empty() {
        cli.to_vec()
    } else if let Some(env) = ctx.var(ROOTS_ENV) {
        env.split(':')
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect()
    } else {
        configured_roots(ctx, warnings)
            .unwrap_or_else(|| DEFAULT_ROOTS.iter().map(|r| r.to_string()).collect())
    };
    let mut roots: Vec<PathBuf> = Vec::new();
    for item in raw {
        let path = ctx.to_path(&item);
        if !roots.contains(&path) {
            roots.push(path);
        }
    }
    roots
}

fn configured_roots(ctx: &Context, warnings: &mut Vec<String>) -> Option<Vec<String>> {
    let value = ctx.config(warnings).remove("roots")?;
    let roots: Option<Vec<String>> = value.as_array().and_then(|items| {
        items
            .iter()
            .map(|i| i.as_str().filter(|s| !s.is_empty()).map(str::to_string))
            .collect()
    });
    match roots {
        Some(roots) if !roots.is_empty() => Some(roots),
        _ => {
            warnings.push(format!(
                "girth: ignoring invalid roots in {}",
                ctx.cwd.join(CONFIG_PATH).display()
            ));
            None
        }
    }
}

/// Crafted-skill dir: `--output`, then `GIRTH_OUTPUT_DIR`, then config `output`, then default.
pub fn resolve_output_dir(cli: Option<&str>, ctx: &Context, warnings: &mut Vec<String>) -> PathBuf {
    let raw = match cli
        .filter(|c| !c.is_empty())
        .or_else(|| ctx.var(OUTPUT_ENV))
    {
        Some(raw) => raw.to_string(),
        None => ctx
            .config(warnings)
            .get("output")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_OUTPUT)
            .to_string(),
    };
    ctx.to_path(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &TempDir, env: &[(&str, &str)]) -> Context {
        Context {
            cwd: dir.path().to_path_buf(),
            home: Some(dir.path().join("home")),
            env: env
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    fn config(dir: &TempDir, text: &str) {
        std::fs::create_dir_all(dir.path().join(".girth")).unwrap();
        std::fs::write(dir.path().join(CONFIG_PATH), text).unwrap();
    }

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|i| i.to_string()).collect()
    }

    #[test]
    fn cli_roots_win_over_env_and_config() {
        let dir = TempDir::new().unwrap();
        config(&dir, r#"{"roots": ["cfg"]}"#);
        let roots = resolve_roots(
            &s(&["cli"]),
            &ctx(&dir, &[(ROOTS_ENV, "envroot")]),
            &mut vec![],
        );
        assert_eq!(roots, vec![dir.path().join("cli")]);
    }

    #[test]
    fn env_roots_split_on_colon_and_win_over_config() {
        let dir = TempDir::new().unwrap();
        config(&dir, r#"{"roots": ["cfg"]}"#);
        let env = format!("a:{}:", dir.path().join("b").display());
        let roots = resolve_roots(&[], &ctx(&dir, &[(ROOTS_ENV, &env)]), &mut vec![]);
        assert_eq!(roots, vec![dir.path().join("a"), dir.path().join("b")]);
    }

    #[test]
    fn config_roots_used_when_no_cli_or_env() {
        let dir = TempDir::new().unwrap();
        config(
            &dir,
            r#"{"roots": ["vendor-skills", "/abs/skills", "~/mine"]}"#,
        );
        let roots = resolve_roots(&[], &ctx(&dir, &[]), &mut vec![]);
        assert_eq!(
            roots,
            vec![
                dir.path().join("vendor-skills"),
                PathBuf::from("/abs/skills"),
                dir.path().join("home/mine")
            ]
        );
    }

    #[test]
    fn defaults_expand_home() {
        let dir = TempDir::new().unwrap();
        let roots = resolve_roots(&[], &ctx(&dir, &[]), &mut vec![]);
        assert_eq!(
            roots,
            vec![
                dir.path().join("home/.claude/skills"),
                dir.path().join("home/.agents/skills"),
                dir.path().join(".claude/skills"),
            ]
        );
    }

    #[test]
    fn invalid_config_json_falls_back_to_defaults_with_warning() {
        let dir = TempDir::new().unwrap();
        config(&dir, "{not json");
        let mut warnings = vec![];
        let roots = resolve_roots(&[], &ctx(&dir, &[]), &mut warnings);
        assert_eq!(roots.len(), 3);
        assert!(
            warnings[0].starts_with("girth: ignoring invalid"),
            "{warnings:?}"
        );
    }

    #[test]
    fn invalid_config_roots_value_warns() {
        let dir = TempDir::new().unwrap();
        config(&dir, r#"{"roots": 5}"#);
        let mut warnings = vec![];
        assert_eq!(resolve_roots(&[], &ctx(&dir, &[]), &mut warnings).len(), 3);
        assert!(
            warnings[0].starts_with("girth: ignoring invalid roots"),
            "{warnings:?}"
        );
    }

    #[test]
    fn duplicate_roots_are_removed_preserving_order() {
        let dir = TempDir::new().unwrap();
        let abs_a = dir.path().join("a").display().to_string();
        let roots = resolve_roots(&s(&["b", "a", "./b", &abs_a]), &ctx(&dir, &[]), &mut vec![]);
        assert_eq!(roots, vec![dir.path().join("b"), dir.path().join("a")]);
        assert_eq!(
            roots[0].display().to_string(),
            format!("{}/b", dir.path().display())
        );
    }

    #[test]
    fn tilde_user_paths_are_not_expanded() {
        let dir = TempDir::new().unwrap();
        let roots = resolve_roots(&s(&["~other/x", "~"]), &ctx(&dir, &[]), &mut vec![]);
        assert_eq!(
            roots,
            vec![dir.path().join("~other/x"), dir.path().join("home")]
        );
    }

    #[test]
    fn empty_config_roots_warn_and_fall_back() {
        let dir = TempDir::new().unwrap();
        config(&dir, r#"{"roots": []}"#);
        let mut warnings = vec![];
        assert_eq!(resolve_roots(&[], &ctx(&dir, &[]), &mut warnings).len(), 3);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn output_dir_resolution_order() {
        let dir = TempDir::new().unwrap();
        let none = ctx(&dir, &[]);
        assert_eq!(
            resolve_output_dir(None, &none, &mut vec![]),
            dir.path().join(".claude/skills")
        );
        config(&dir, r#"{"output": "crafted"}"#);
        assert_eq!(
            resolve_output_dir(None, &none, &mut vec![]),
            dir.path().join("crafted")
        );
        let env = ctx(&dir, &[(OUTPUT_ENV, "envout")]);
        assert_eq!(
            resolve_output_dir(None, &env, &mut vec![]),
            dir.path().join("envout")
        );
        assert_eq!(
            resolve_output_dir(Some("cli"), &env, &mut vec![]),
            dir.path().join("cli")
        );
    }
}
