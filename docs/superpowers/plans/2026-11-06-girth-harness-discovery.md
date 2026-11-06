# Harness discovery and output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `girth` finds skills in every Agent Skills harness's folders and writes crafted skills where the running harness reads them.

**Architecture:** A generated table, `src/harness_table.rs`, lists every harness from `vercel-labs/skills` `docs/agents.md`. `src/harness.rs` wraps the table plus the extra dirs confirmed on disk. `config::resolve_roots` appends the dirs that exist, deduplicated by canonical path. `config::resolve_output_dir` defaults to `.claude/skills` under `CLAUDECODE=1` and to `.agents/skills` otherwise.

**Tech Stack:** Rust (edition 2024), `cargo test`, POSIX sh plus awk for the sync script.

**Spec:** `docs/superpowers/specs/2026-11-06-girth-all-harnesses-design.md`, sections 1, 2 and 4 (the launcher). Spend import (§3) and badges (§5–6) have their own plans.

## Global Constraints
- Never hardcode a root that overrides the user: `--roots`, `GIRTH_SKILL_ROOTS` and config `roots` still replace all defaults.
- These frozen interfaces stay byte-identical: the config keys `roots` and `output`, the `output_dir: ` line, and the JSON keys of `search` and `measure`.
- The three existing defaults (`~/.claude/skills`, `~/.agents/skills`, `.claude/skills`) stay in the list even when missing. Existing users see no change, and tests assert `len() == 3` on an empty home.
- Cargo runs at `-j 2` under `nice -n 10`.
- Tests must not inherit the developer's `CLAUDECODE=1`. Unit tests build `Context` explicitly; CLI tests `env_remove("CLAUDECODE")` unless they test it.

## Review Focus
1. **Generic project dirs.** `openclaw` uses `skills/`, `astrbot` uses `data/skills/` and `eve` uses `agent/skills/`. In an arbitrary repo these are unrelated folders, and this repo ships `skills/girth`. Only project dirs that start with `.` are used. Test: `non_dot_project_dirs_are_ignored`.
2. **The same folder twice.** `~/.codex/skills/find-skills` links to `~/.agents/skills`, and `~/.config/agents/skills` may be a symlink to it. Each canonical path is searched once. Test: `symlinked_harness_dir_is_searched_once`.
3. **An empty home** (CI, a sandbox): exactly the three defaults plus plugin dirs, as before. Test: `empty_config_roots_warn_and_fall_back` keeps `len() == 3`.
4. **`CLAUDECODE` set to anything but `1`** (`0`, empty): the default output is `.agents/skills`. Test: `output_default_follows_harness`.
5. **The launcher run through a symlinked skills-CLI install with no repo above it:** it must not exec `cargo` from `$HOME`. Test in `tests/launcher.rs`.

---

### Task 1: Generated harness table

**Files:**
- Create: `scripts/sync-harnesses.sh`, `src/harness_table.rs` (generated, committed), `src/harness.rs`
- Modify: `src/lib.rs` (add `pub mod harness;`)

**Interfaces:**
- Produces: `pub struct Harness { pub id: &'static str, pub project: Option<&'static str>, pub user: Option<&'static str> }`, `pub const HARNESSES: &[Harness]` (in `harness_table.rs`, `include!`d by `harness.rs`), and `pub fn skill_dirs() -> Vec<&'static str>`. `skill_dirs` returns every user dir (`~/…`), every dot-prefixed project dir, and `EXTRA_USER_DIRS`, deduplicated with the first occurrence kept.
- `EXTRA_USER_DIRS = ["~/.codex/skills", "~/.gemini/antigravity-cli/skills", "~/.config/opencode/skills"]`.

- [ ] **Step 1: Write the failing tests** in `src/harness.rs`:
  - `table_covers_upstream`: `HARNESSES.len() >= 66`. It contains `codex` → (`.agents/skills`, `~/.agents/skills`), `cursor` → `~/.cursor/skills`, `gemini-cli` → `~/.gemini/skills`, `github-copilot` → `~/.copilot/skills`, `claude-code` → `.claude/skills`, and `eve` → user `None`.
  - `non_dot_project_dirs_are_ignored`: `skill_dirs()` contains no `skills`, `data/skills` or `agent/skills`. It does contain `.agents/skills` and `~/.codex/skills`.
  - `skill_dirs_are_unique`: there are no duplicates.
- [ ] **Step 2:** Run `nice -n 10 cargo test -j 2 -q harness`. Expected: it FAILS to compile (no module).
- [ ] **Step 3:** Write `scripts/sync-harnesses.sh`.
  - It curls `https://raw.githubusercontent.com/vercel-labs/skills/main/docs/agents.md` and takes the rows between `<!-- supported-agents:start -->` and the end marker.
  - It splits the comma-separated ids into one row each, strips backticks and the trailing `/`, and maps `N/A…` to `None`.
  - It writes `src/harness_table.rs` with a header comment naming the source URL and the date.
  - Run it and commit the output. Then write `harness.rs` around it.
- [ ] **Step 4:** Run `nice -n 10 cargo test -j 2 -q harness`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(harness): table of every Agent Skills harness, synced from the skills CLI`.

### Task 2: Default roots include existing harness dirs

**Files:**
- Modify: `src/config.rs` (`resolve_roots`, tests)

**Interfaces:**
- Consumes: `harness::skill_dirs()`.
- Produces: unchanged signature `resolve_roots(&[String], &Context, &mut Vec<String>) -> Vec<PathBuf>`.
  - Order: the 3 `DEFAULT_ROOTS`, then plugin dirs, then each harness dir that `is_dir()` after `to_path`.
  - Any root whose `canonicalize()` equals an earlier root's is skipped. A missing path compares by its own value.

- [ ] **Step 1: Write the failing tests:**
  - `existing_harness_dirs_join_defaults`: create `home/.codex/skills` and `.cursor/skills` in cwd. The roots contain both, and the length is 5.
  - `symlinked_harness_dir_is_searched_once`: `home/.config/agents/skills` symlinks to `home/.agents/skills`. Only `home/.agents/skills` appears.
  - `explicit_roots_get_no_harness_dirs`: with `home/.codex/skills` present and `--roots x`, the result is `[cwd/x]`.
- [ ] **Step 2:** Run `nice -n 10 cargo test -j 2 -q config::`. Expected: the new tests FAIL.
- [ ] **Step 3:** Implement it in `resolve_roots`'s default branch and in the dedup loop.
- [ ] **Step 4:** Run `cargo test -j 2 -q config:: && cargo test -j 2 -q --test cli`. Expected: PASS. `empty_config_roots_warn_and_fall_back` still gives 3.
- [ ] **Step 5:** Commit: `feat(config): discover skills in every harness's folders`.

### Task 3: Output default follows the harness

**Files:**
- Modify: `src/config.rs` (`DEFAULT_OUTPUT` → `CLAUDE_OUTPUT = ".claude/skills"` and `AGENTS_OUTPUT = ".agents/skills"`), `tests/cli.rs`, `skills/girth/SKILL.md` and references wherever they name `.claude/skills` as *the* output.

**Interfaces:**
- Produces: `resolve_output_dir` with the same signature. The final fallback is `CLAUDE_OUTPUT` iff `ctx.var("CLAUDECODE") == Some("1")`, else `AGENTS_OUTPUT`.

- [ ] **Step 1: Write the failing tests:**
  - `output_default_follows_harness`: no env gives `cwd/.agents/skills`; `CLAUDECODE=1` gives `cwd/.claude/skills`; `CLAUDECODE=0` gives `.agents/skills`.
  - Update `output_dir_resolution_order`'s first assertion to pass `CLAUDECODE=1`.
  - In `tests/cli.rs`, `search_prints_output_dir_for_harness` runs `girth search x` with `env_remove("CLAUDECODE")` and expects stdout to contain `output_dir: <tmp>/.agents/skills`. With `CLAUDECODE=1` it expects `.claude/skills`.
- [ ] **Step 2:** Run `cargo test -j 2 -q output_`. Expected: FAIL.
- [ ] **Step 3:** Implement it. Then `grep -rn '\.claude/skills' skills/girth`. Each line that tells the model where crafts go must defer to the `output_dir:` line instead. Follow `.claude/skills/girth-skill-authoring`, and keep the frozen strings.
- [ ] **Step 4:** Run `cargo test -j 2 -q` (all). Expected: PASS, including `tests/skill_md.rs`.
- [ ] **Step 5:** Commit: `feat(config): crafted skills go to .agents/skills outside Claude Code`.

### Task 4: Launcher under a skills-CLI install

**Files:**
- Modify: `tests/launcher.rs`, and `skills/girth/scripts/girth` only if the test fails.

- [ ] **Step 1: Write the test** `skills_cli_install_without_repo_downloads`.
  - Copy the launcher to `<tmp>/home/.agents/skills/girth/scripts/girth`, and symlink `<tmp>/home/.codex/skills/girth` to it.
  - Run the symlinked path with `HOME=<tmp>/home`, a `PATH` with no `girth` or `cargo`, and `GIRTH_DOWNLOAD_BASE` pointing at a `file://` fixture with a fake binary and checksum, following the existing download tests' fixture helper.
  - Assert that it execs the fake binary and that no `cargo` was attempted.
- [ ] **Step 2:** Run `cargo test -j 2 -q --test launcher skills_cli`. If it passes, the launcher already handles this; keep the test as a guard.
- [ ] **Step 3:** If it fails, fix the `repo=` resolution so the repo checkout branches apply only when `$repo/Cargo.toml` names `girth`.
- [ ] **Step 4:** Run `cargo test -j 2 -q --test launcher`. Expected: PASS.
- [ ] **Step 5:** Commit: `test(launcher): a skills-CLI install with no repo above downloads the binary`.

### Task 5: Gate and record
- [ ] Run `make check` (fmt, clippy, tests, coverage ≥ 90%, plugin validate). Expected: all green.
- [ ] Run `nice -n 10 cargo mutants -j 2 -f src/harness.rs -f src/config.rs`. Expected: ≥ 80% caught; equivalents go in `docs/mutation-notes.md`.
- [ ] Record before and after `girth measure` on this machine in `examples/portability/RESULTS.md` (skills found, roots), and add a CHANGELOG `[Unreleased]` entry.
- [ ] Commit: `docs: harness discovery measured on the author's machine`.
