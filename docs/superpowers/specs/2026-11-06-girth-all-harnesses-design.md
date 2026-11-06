# Girth on every agent harness

**Date:** 2026-11-06 · **Status:** approved by the author's standing mandate ("target other harnesses … every harness out there … add badges related to them")

## Why

Girth's two ideas are not tied to Claude Code. The first is just-in-time project skills distilled from installed ones. The second is knowing what your agent spends and why. The Agent Skills format is read by 66 agents in the `skills` CLI's table (`skills@1.7.6`) and 46 clients on agentskills.io. Girth today is Claude-only in three places:

- **Discovery** looks only in `~/.claude/skills`, `~/.agents/skills` and `.claude/skills`. On the author's machine it misses `~/.codex/skills`, `~/.gemini/skills` and `~/.cursor/skills`.
- **Crafted skills** are always written to `.claude/skills`. Every harness except Claude Code reads `.agents/skills` instead (Codex, Cursor, Copilot, Gemini CLI, Antigravity, OpenCode, Cline, Amp, Kilo and more).
- **Spend** (`stats`, `advise`) comes only from the Claude Code mod's ledger.

## Scope

**Portable after this work:** `search`, `measure`, `validate`, `show`, `fetch`, `made`, `drift`, `taste`, crafted skills, and `stats`/`advise` over imported spend.

**Stays Claude Code only, and the README says so:** Lens, the Meter mod, keeper, idle compaction and the `/girth-*` commands. They depend on the Claude Code mods API.

"Every harness" means every row of the `skills` CLI's agent table, held as **data** (`src/harness.rs`, one row per harness), not as 66 code paths.

## What

### 1. Harness table and discovery
- `src/harness.rs` holds `HARNESSES: [(id, project_dir, user_dir)]`, generated from `vercel-labs/skills` `docs/agents.md`.
  - `scripts/sync-harnesses.sh` regenerates it.
  - A unit test pins the row count and a few known rows.
- **Extra user dirs not in that table, confirmed on disk or in docs:** `~/.codex/skills` (Codex), `~/.gemini/config/skills` and `~/.gemini/antigravity-cli/skills` (Antigravity), `~/.copilot/skills` (Copilot CLI), `~/.config/opencode/skills` (OpenCode).
- **Default roots** are the current defaults plus every harness's user dir and project dir **that exists**. Missing dirs are dropped, so `measure` never lists dead paths.
  - A dir reached twice (symlinks, shared `.agents/skills`) is searched once, by its canonical path.
- `--roots`, `GIRTH_SKILL_ROOTS` and the `roots` key in `.girth/config.json` still replace the defaults. No root is hardcoded.

### 2. Where crafted skills go
- **Default output:**
  - `.claude/skills` when `CLAUDECODE=1` (Claude Code reads project skills only there, per code.claude.com/docs/en/skills);
  - otherwise `.agents/skills`.
- `GIRTH_OUTPUT_DIR` and the frozen `output` config key still override.
- The `output_dir: ` line in `search` output reports whichever applies, so SKILL.md keeps working unchanged.

### 3. Spend from other harnesses: `girth import [--from codex|copilot|opencode|gemini|all]`
- **What it does:** reads each harness's local session logs and appends `turn` and `use` records to the ledger. Each record gets a new `harness` field (`Option<String>`, serde default, absent means Claude Code).
- **Idempotent:** each source keeps a cursor of imported request ids in `~/.girth/import.json` (Codex `response_id`, Copilot `apiCallId`, OpenCode message id, Gemini message id). Re-running never double-counts.

| Harness | Source (verified on this machine) | Per request | Turn grouping | Skill use |
|---|---|---|---|---|
| Codex | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`, `token_usage_record` | input, cached_input, cache_write_input, output | `turn_id` | inferred from reads of a `SKILL.md`, marked inferred |
| Copilot CLI | `~/.copilot/session-state/<id>/events.jsonl`, `session.usage_record` | input, output, cacheRead, cacheWrite, cost, model | one turn per request | `skill.invoked` |
| OpenCode | `~/.local/share/opencode/opencode.db`, `message.data` | tokens.input, output, cache.read and cache.write, cost, modelID | message | `skill` tool parts |
| Gemini CLI | `~/.gemini/tmp/<project>/chats/session-*.jsonl` | tokens.input, output, cached (no cache write) | message | `activate_skill` calls |

- **Not imported:**
  - Cursor and Amp keep no token data on disk.
  - Antigravity's logs are opaque protobuf.
  - Goose keeps per-session totals only.
- **Prices:**
  - Use the log's own `cost` when present (Copilot, OpenCode).
  - Otherwise take rates from the provider's official pricing page, cited in `price.rs`.
  - For an unpriced model, the turn keeps its tokens, and `advise` labels its share "price unknown" rather than guessing.
- `advise` and `stats` gain a `--harness <id>` filter, and their JSON gains a per-harness split under a new key. Existing keys are unchanged (frozen).
- Codex is built first (richest data, logs on disk here), then Copilot CLI, OpenCode and Gemini CLI.
- OpenCode needs SQLite. Use `rusqlite` with `bundled`, only after `cargo deny` passes and the musl release targets build.

### 4. Shipped skill
- SKILL.md and its references mark the Claude-only steps ("In Claude Code, …"). Elsewhere, the skill tells Claude to run `girth import` before `advise`.
- It follows `.claude/skills/girth-skill-authoring`. Frozen strings are unchanged.
- **Launcher:** under a skills-CLI install (`~/.agents/skills/girth/scripts/girth`, often reached through a symlink), there is no repo above it. The launcher must fall through to the checksum-verified download. This gets a new `tests/launcher.rs` case.

### 5. Distribution manifests and badges
- **Local manifests** (zero risk): `gemini-extension.json` (Gemini CLI, and Antigravity via `agy plugin import`), `.codex-plugin/plugin.json`, `.cursor-plugin/plugin.json`. Each points at `skills/girth`. `claude plugin validate` must still pass.
- **README:**
  - A compatibility table: harness, skill dir, discovery, crafted skills, spend import, and *verified* (a headless run recorded in `examples/portability/`).
  - **Badges, truthful only:**
    - a static "Agent Skills format" badge (agentskills.io has no official one);
    - one "works with <harness>" badge per harness verified by a headless run;
    - the skills.sh install badge (`https://skills.sh/b/mohessaid/girth`) once the listing exists.
    - Untested harnesses get a table row, not a badge.
- **Outward-facing, so they need the author's go:**
  - creating the skills.sh listing with `npx skills add mohessaid/girth` (already the author's to-do);
  - the GitHub topic `gemini-cli-extension`, which the Gemini gallery crawls;
  - Cursor Marketplace and OpenAI plugin submissions;
  - a PR to OpenCode's ecosystem page;
  - a PR adding Girth to agentskills.io's client list.

### 6. Verification runs (earn the badges)
- A headless, read-only run per installed harness (Codex, Copilot CLI, OpenCode, `agy`, plus Cursor once logged in): in a fresh folder with a crafted skill in `.agents/skills`, ask the agent to list its `girth-` skills, then to run `girth search` through the skill. Record the results in `examples/portability/RESULTS.md`.
- These runs spend other providers' credits, not the Claude plan. Run them one at a time, never alongside cargo.

## Follow-up: one project cache across harnesses

- **The split.** Crafted skills now land in `.claude/skills` under Claude Code and in `.agents/skills` elsewhere. Claude Code reads only the first, and most other harnesses only the second. `girth search` finds both, as root hits. But on a repo worked from two harnesses, a skill crafted in one does not load in the other, so the compounding cache stops compounding across harnesses.
- **Options, not built yet:**
  - **Link at craft time.** Under Claude Code, write to `.agents/skills/girth-x` and link `.claude/skills/girth-x` to it. One copy then serves every harness.
  - **The reverse**, or a `girth sync` command that links whichever side is missing.
- **Before choosing, check:**
  - Claude Code follows a symlinked skill dir. Plugin installs suggest so, but this needs checking for project skills.
  - Links survive `git` on Windows checkouts.

## Not in scope
- Porting Lens, Meter, keeper or compaction to other harnesses' hook systems. Codex's `PostModelRequest` hook carries `token_usage`, so a live Codex meter is a possible follow-up once import shows the data is useful.
- Rewriting the A/B example scripts. They measure Claude Code and keep `.claude/skills`.

## Tests
- **Unit:**
  - the harness table (count and sample rows);
  - root discovery keeps existing dirs only and deduplicates by canonical path;
  - the output default with and without `CLAUDECODE`;
  - each importer on synthetic fixtures copied from the real line shapes (the Codex sample above, the Copilot `session.usage_record`, an OpenCode row);
  - re-import adds nothing;
  - an unpriced model is labelled, not priced.
- **CLI:** `girth import --from codex` on a fixture home, then `advise --harness codex --json`.
- **Launcher:** a skills-CLI install with no repo above it.
- **Gates:** `make check` (coverage ≥ 90%), `make mutate` on the new modules, and one `make evals` run at the end.
