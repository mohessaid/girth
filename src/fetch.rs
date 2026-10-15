//! `girth fetch`: shallow-clone a public GitHub skill repo into an untrusted scratch dir.
//!
//! Only `owner/repo[/path]` specs are accepted, so no URL, flag, ssh or file
//! transport can be smuggled into git. Fetched content is data: it is never
//! executed, and it never lands in a skills dir.

use std::path::{Path, PathBuf};

pub const FETCH_DIR: &str = "fetch";
pub const GIT_ENV: &str = "GIRTH_GIT";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub owner: String,
    pub repo: String,
    /// Sub-path inside the repo, if any (e.g. `skills/captions`).
    pub path: Option<String>,
}

impl Spec {
    pub fn url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.repo)
    }

    /// `GIRTH_HOME/fetch/<owner>__<repo>`.
    pub fn dest(&self, home: &Path) -> PathBuf {
        home.join(FETCH_DIR)
            .join(format!("{}__{}", self.owner, self.repo))
    }
}

/// Parse `owner/repo[/sub/path]` (a `.git` suffix on the repo is tolerated).
pub fn parse_spec(raw: &str) -> Result<Spec, String> {
    let bad = || format!("expected owner/repo[/path], got '{raw}'");
    let segments: Vec<&str> = raw.trim_end_matches('/').split('/').collect();
    if segments.len() < 2 || !segments.iter().all(|s| safe_segment(s)) {
        return Err(bad());
    }
    let repo = segments[1].strip_suffix(".git").unwrap_or(segments[1]);
    if !safe_segment(repo) {
        return Err(bad());
    }
    let path = (segments.len() > 2).then(|| segments[2..].join("/"));
    Ok(Spec {
        owner: segments[0].to_string(),
        repo: repo.to_string(),
        path,
    })
}

/// Letters, digits, `.`, `_`, `-`; never empty, never leading `.` or `-`.
fn safe_segment(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with(['.', '-'])
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The git arguments for a hardened shallow clone into `dest`.
pub fn clone_args(spec: &Spec, dest: &Path) -> Vec<String> {
    [
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "protocol.allow=never",
        "-c",
        "protocol.https.allow=always",
        "clone",
        "--depth",
        "1",
        "--no-recurse-submodules",
        "--quiet",
        "--",
    ]
    .iter()
    .map(|a| a.to_string())
    .chain([spec.url(), dest.display().to_string()])
    .collect()
}

/// Clone `spec` into a fresh dest dir (replacing an earlier fetch) and return the
/// path to search: the repo root, or the sub-path when one was given.
pub fn fetch(spec: &Spec, home: &Path, git: &str) -> Result<PathBuf, String> {
    let dest = spec.dest(home);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("could not clear {}: {e}", dest.display()))?;
    }
    std::fs::create_dir_all(home.join(FETCH_DIR)).map_err(|e| e.to_string())?;
    let status = std::process::Command::new(git)
        .args(clone_args(spec, &dest))
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !status.success() {
        return Err(format!("git clone failed for {} ({status})", spec.url()));
    }
    let target = match &spec.path {
        Some(path) => dest.join(path),
        None => dest,
    };
    if target.is_dir() {
        Ok(target)
    } else {
        Err(format!(
            "fetched {}/{}, but {} is missing",
            spec.owner,
            spec.repo,
            spec.path.as_deref().unwrap_or("the checkout")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn parses_owner_repo_and_optional_path() {
        assert_eq!(
            parse_spec("heygen-com/hyperframes"),
            Ok(Spec {
                owner: "heygen-com".into(),
                repo: "hyperframes".into(),
                path: None
            })
        );
        assert_eq!(
            parse_spec("obra/superpowers.git/skills/brainstorming/"),
            Ok(Spec {
                owner: "obra".into(),
                repo: "superpowers".into(),
                path: Some("skills/brainstorming".into())
            })
        );
        assert_eq!(parse_spec("a_b/c.d").unwrap().repo, "c.d");
    }

    #[test]
    fn rejects_anything_that_is_not_a_plain_spec() {
        for bad in [
            "",
            "justone",
            "https://github.com/a/b",
            "git@github.com:a/b",
            "file:///etc/x",
            "-oProxyCommand=x/b",
            "a/-b",
            "a/b/../../etc",
            "a/b/./c",
            "a/b//c",
            "../a/b",
            "a b/c",
            "a/b c",
            "a/b;rm",
            "a/.git",
            "a/..",
            "ä/b",
        ] {
            assert!(parse_spec(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn url_and_dest_are_derived_from_the_spec() {
        let spec = parse_spec("obra/superpowers/skills").unwrap();
        assert_eq!(spec.url(), "https://github.com/obra/superpowers.git");
        assert_eq!(
            spec.dest(Path::new("/h/.girth")),
            PathBuf::from("/h/.girth/fetch/obra__superpowers")
        );
    }

    #[test]
    fn clone_is_shallow_and_hardened() {
        let spec = parse_spec("a/b").unwrap();
        let args = clone_args(&spec, Path::new("/d"));
        let joined = args.join(" ");
        for needle in [
            "-c core.hooksPath=/dev/null",
            "-c protocol.allow=never",
            "-c protocol.https.allow=always",
            "clone --depth 1 --no-recurse-submodules --quiet",
            "-- https://github.com/a/b.git /d",
        ] {
            assert!(joined.contains(needle), "{joined} lacks {needle}");
        }
        // The URL and dest come after `--`, so they can never be read as options.
        let dashdash = args.iter().position(|a| a == "--").unwrap();
        assert_eq!(args.len(), dashdash + 3);
    }

    #[test]
    fn fetch_runs_git_into_a_fresh_dest_and_returns_the_sub_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let fake = tmp.path().join("fake-git");
        // The last argument is the dest: create a repo-like tree there.
        std::fs::write(
            &fake,
            "#!/bin/sh\nfor last; do :; done\nmkdir -p \"$last/skills/x\" && echo hi > \"$last/skills/x/SKILL.md\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let home = tmp.path().join("home");
        let spec = parse_spec("a/b/skills/x").unwrap();
        let stale = spec.dest(&home).join("stale.md");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, "old").unwrap();
        let got = fetch(&spec, &home, fake.to_str().unwrap()).unwrap();
        assert_eq!(got, spec.dest(&home).join("skills/x"));
        assert!(got.join("SKILL.md").is_file());
        assert!(!stale.exists(), "an earlier fetch is replaced");
    }

    #[test]
    fn fetch_reports_git_failure_and_missing_sub_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let spec = parse_spec("a/b").unwrap();
        let err = fetch(&spec, &home, "/bin/false").unwrap_err();
        assert!(err.contains("git clone failed"), "{err}");
        let err = fetch(&spec, &home, "/definitely/not/git").unwrap_err();
        assert!(err.contains("could not run git"), "{err}");
        let err = fetch(&parse_spec("a/b/missing").unwrap(), &home, "/bin/true").unwrap_err();
        assert!(err.contains("missing"), "{err}");
    }

    proptest! {
        #[test]
        fn accepted_specs_never_escape_or_look_like_options(raw in ".{0,60}") {
            if let Ok(spec) = parse_spec(&raw) {
                let parts = [&spec.owner, &spec.repo];
                for part in parts {
                    prop_assert!(!part.starts_with('-') && !part.starts_with('.'));
                }
                if let Some(path) = &spec.path {
                    prop_assert!(path.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != ".."));
                }
                prop_assert!(spec.url().starts_with("https://github.com/"));
            }
        }
    }
}
