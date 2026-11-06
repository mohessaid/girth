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
- **Idempotent:** each source keeps a cursor per file or session in `~/.girth/import.json`. Re-running never double-counts. **As built:**
  - Codex: the byte offset after the last finished turn (a shorter file is read again).
  - Copilot: the number of shutdowns imported (fewer shutdowns means it is read again).
  - OpenCode: per session, the time of the last imported prompt.
  - Gemini: per file, the time of the last imported prompt, since older files are rewritten whole.

| Harness | Source (verified on this machine) | Per request | Turn grouping | Skill use |
|---|---|---|---|---|
| Codex | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`, `token_usage_record` | input, cached_input, cache_write_input, output | `turn_id` | inferred from reads of a `SKILL.md`, marked inferred |
| Copilot CLI | `~/.copilot/session-state/<id>/events.jsonl`, `session.shutdown` `modelMetrics` (built); `session.usage_record` (1.0.105+, follow-up) | requests, premium requests, input (includes cache), output, cacheRead, cacheWrite | one turn per model per shutdown | `skill.invoked` |
| OpenCode | `~/.local/share/opencode/opencode.db`, `message.data` | tokens.input, output, cache.read and cache.write, cost, modelID | message | `skill` tool parts |
| Gemini CLI | `~/.gemini/tmp/<project>/chats/session-*.json` (rewritten whole) and `.jsonl` (later copies replace earlier ones) | tokens.input (includes cached), cached, output, thoughts, tool | one turn per prompt | `activate_skill` calls |

- **Not imported:**
  - Cursor and Amp keep no token data on disk.
  - Antigravity's logs are opaque protobuf.
  - Goose keeps per-session totals only.
- **Prices:**
  - Use the log's own `cost` when present and nonzero (OpenCode). Copilot's `cost` is 0 for BYOK, and its `requests.cost` counts premium requests, not dollars. Girth prices Copilot's tokens at list prices and reports premium requests beside them, because that is what Copilot bills.
  - Otherwise take rates from the provider's official pricing page, cited in `price.rs`.
  - For an unpriced model, the turn keeps its tokens, and `advise` labels its share "price unknown" rather than guessing.
- `advise` and `stats` gain a `--harness <id>` filter, and their JSON gains a per-harness split under a new key. Existing keys are unchanged (frozen).
- Codex is built first (richest data, logs on disk here), then Copilot CLI, OpenCode and Gemini CLI.
- OpenCode keeps SQLite. **Changed (v0.12):** Girth runs `opencode db --pure --format json <query>` instead of linking `rusqlite`. That means no C dependency for the musl targets, and OpenCode resolves its own database and schema. It runs only when the database already exists, so Girth never creates OpenCode's state.

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

## Follow-up: one project cache across harnesses (decided 2026-11-06)

- **The split.** Crafted skills land in `.claude/skills` under Claude Code and in `.agents/skills` elsewhere. Claude Code reads only the first; most other harnesses read only the second. On a repo worked from two harnesses, a skill crafted in one does not load in the other, so the cache stops compounding across harnesses.
- **What was checked:**
  - **Claude Code follows a symlinked skill folder.** Its skills docs say: "a `<skill-name>` entry in the enterprise, personal, or project location can be a symlink to a directory elsewhere on disk. Claude Code reads `SKILL.md` from the target and loads the skill once even if several locations point at the same target."
  - **The other harnesses follow links too.** `npx skills add` installs by linking into each harness's folder. The headless runs in `examples/portability/RESULTS.md` loaded skills set up that way.
  - **Windows:** with git's default `core.symlinks=false`, a committed link checks out as a small file holding the target path. That isn't a skill folder, so every harness ignores it: no sharing, and no harm.
- **Decision: link at validate time.**
  - Where skills are written doesn't change: each harness writes where it reads, so a failed link never hides a skill from the harness that crafted it.
  - `girth validate` runs on every crafted skill in the craft flow. When a valid `girth-*` skill sits directly in `<project>/.claude/skills/` or `<project>/.agents/skills/`, and the same name doesn't exist on the other side, it creates a relative symlink there: `.agents/skills/girth-x → ../../.claude/skills/girth-x`, or the reverse.
  - A link that can't be made is noted on stderr, and validation still passes. Stdout is unchanged (`VALID <dir>` is frozen). Existing files or folders on the other side are never replaced.
  - Discovery already searches a folder reached twice only once, by its canonical path.
- **Committing.** Crafted skills are project files, meant to be committed so the cache compounds across machines and teammates; the link commits with them. On Windows checkouts the link degrades as described above.
- **Rejected:** writing to `.agents/skills` under Claude Code and linking back. If the link failed, Claude Code would lose the skill it just crafted. A separate `girth sync` command was also rejected: an extra step that agents and users would forget.

## Follow-up: Copilot per-request import

- **Built (v0.12):** shutdown totals, one turn per model per shutdown. These cover every Copilot version on the author's machine (0.0.394 to 1.0.106).
- **Decided (v0.13): per-request records.** Copilot 1.0.105+ also writes `session.usage_record` per request. On the author's machine (Copilot 1.0.106), 3 sessions carry them, and in each the records sum exactly to the session's shutdown totals.
  - A stretch (the events up to a `session.shutdown`) that holds any record is read from its records, and that shutdown's `modelMetrics` is ignored, so one stretch is never counted both ways. A stretch with no records falls back to its shutdown. The shutdown still feeds the running-totals check for the next stretch.
  - A turn is one `initiator: "user"` record and the `initiator: "agent"` records after it, one ledger turn per model, at the time of its last record. Prompts and turns need not match: one prompt on the author's machine was answered by agent-initiated requests only.
  - Token arithmetic is the shutdown path's (`inputTokens` includes cache). `premium` is the sum of each record's `cost`, read like `requests.cost` and rounded. Every record seen is a BYOK request with `cost: 0`; what `cost` holds for a non-BYOK record wasn't seen.
  - Records after the last shutdown (a live session) are imported once their turn is done: a later user-initiated record exists, or its last record finished for a reason other than `tool_calls`.
  - **Cursors.** The existing per-file count of imported shutdowns stays. A second count, of records imported, is kept under `<file>#records`, so a count written by 0.12 is never read as a record count. A stretch whose shutdown is already counted is skipped whole, records included; that covers sessions 0.12 imported from shutdowns.

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
