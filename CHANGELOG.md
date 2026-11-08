# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Search finds DeepSeek Harness skills** in `.dsh/skills` and `~/.dsh/skills`, the folders its own docs name. That makes 80 harnesses. The `skills` CLI's table doesn't list DeepSeek Harness yet, so the folders go in Girth's hand-kept extras.

## [0.19.0] - 2026-11-08

### Changed
- **Each turn is priced at the rates of its own day.** Girth prices turns from stored tokens every time it reports, so before this release a vendor's price change also repriced every earlier turn. The meter and `advise` now look up each turn's and side query's rates as of when it ran. Dated rows so far:
  - Claude Sonnet 5.5 cache reads were $0.20 before 2026-10-08.
  - Google doubles Gemini 3.6 and 3.8 Flash on 2027-01-01.
  Live estimates (X-ray, the keeper) use today's rates. When a dated change starts, it applies without a new release.

## [0.18.0] - 2026-11-08

### Added
- **`girth import` reads Antigravity CLI.**
  - **What it reads:** each conversation in `~/.gemini/antigravity-cli/conversations/` is a SQLite database. Girth reads it through the `sqlite3` command, read-only, so it needs no SQLite code. Without `sqlite3`, Antigravity is skipped with a note.
  - **Turns:** a turn is one prompt and the model calls after it. A conversation's last turn is imported once a later prompt exists, or once the conversation has been idle for 30 minutes.
  - **Checked against agy's own report:** the token fields were read off agy 1.5.2's databases. On one conversation, input (35,102), output (475) and thinking (403) matched agy's own `--output-format json` usage exactly. tokscale's parser adds a fixed per-call count to input (1,318 tokens on agy 1.5.2), and agy's report leaves it out, so Girth does too.
  - `girth advise --harness antigravity` names `/clear` and `AGENTS.md`.
  - **Limit:** on agy versions before 1.5, whose steps don't name their model call, a conversation's model calls all go to its last prompt. The totals are still right.
- **List prices for `gemini-3.8-flash` (logged by agy as `gemini-3.8-flash-n`) and `gemini-3.6-flash`:** $0.75 input, $3.75 output and $0.075 cache reads per million tokens. These are Google's prices through 2026-12-31; Google doubles them on 2027-01-01.

### Fixed
- **A plugin update no longer switches Girth off while GitHub's release downloads are failing.** Each Claude Code plugin version lives in its own folder and downloads its binary the first time it runs. On 2026-11-08, GitHub served HTTP 504 for release files for a while, so a fresh 0.17.0 had no binary. The launcher now runs the newest earlier version's binary from the plugin cache, with a note on stderr. It doesn't copy that binary into place, so the next run tries the download again. A checkout, whose folder isn't named as a version, never looks at the folders beside it.

## [0.17.0] - 2026-11-08

### Changed
- **Claude Code's main-loop turns are priced at Claude Code's own figure.** Each request's record carries `cost_usd_micros`. The mod adds these up for each main-loop turn and stores the total as `cost_micros`, but only when the requests' tokens equal the turn's usage exactly, so a lost record never gives a wrong price. The meter and `advise` prefer that figure to Girth's price table. A stale table row, such as Sonnet 5.5's until 0.16, no longer skews these turns, and Haiku 5.5's long-prompt prices are applied as Claude Code bills them. A probe on Sonnet 5.5 matched the table to the millionth of a dollar. Subagent turns, which can run several at once, still use list prices.

### Fixed
- **Cost per outcome sees through `rtk bypass`.** RTK 1.1 renamed `rtk proxy <cmd>` to `rtk bypass <cmd>`, and its instructions now tell agents to use it. A test run or commit behind the new name had no verdict until now.

## [0.16.0] - 2026-11-08

### Added
- **Claude Code's side queries are metered.** These are the requests it sends outside the conversation:
  - progress summaries of background agents (`agent_summary`)
  - prompt suggestions, session titles and compaction
  - A probe showed that Girth's turns hold exactly the main-loop and subagent requests, so before this release none of these was counted (`examples/sidequery/RESULTS.md`).
  - The mod now reads each request's record (its `telemetry.log` collector event, which reaches plugins with telemetry off). It totals the numbers per source and model: requests, tokens and Claude Code's own price. After each answer it saves the totals to the ledger as `side` events. It never copies account, organization or request ids.
  - `girth stats --meter` adds a `side queries` line, and their cost counts in the meter's total and in `advise`.
  - **New `advise` cause: background-agent progress summaries.** Claude Code sends these about every 30 s while a background agent runs, and none of their input is cached. The fix is `CLAUDE_CODE_DISABLE_AGENT_PROGRESS_SUMMARIES=1`, which stopped them on Claude Code 2.1.319 (4 requests became 0 on the same task).
  - Older Girth versions skip `side` events.

### Fixed
- **Claude Sonnet 5.5 cache reads are priced at $0.10 per million tokens, not $0.20.** Anthropic halved them on 2026-10-08, two days after the price list Girth was built from. Cache reads are most of a Claude Code session's input, so Sonnet 5.5 costs came out too high. Prices are worked out from the stored token counts each time, so past turns are corrected too. Claude Haiku 5.5 charges more when a request's prompt is over 100,000 tokens. A turn stores token sums across its requests, not each request's prompt size, so Girth prices every Haiku 5.5 turn at the lower rate.

## [0.15.0] - 2026-11-07

### Added
- **Claude Code's large tool output covers every tool, not just Bash.** Each tool result carries `text`, the text the model reads, which a headless probe confirmed for Read (line-numbered), Grep, Glob, WebFetch and ToolSearch. The mod now measures that for every tool, MCP tools included, and records it per tool. An image Read's `text` is empty, so images count as 0, and a call Girth denies (an MCP server's held-back instructions) isn't counted.

### Fixed
- `advise`'s large-tool-output fix gave `| tail` for every tool it didn't know, such as searches, MCP calls and subagents. Those now count toward the total without a fix of their own, and if only such tools qualify, the fix asks for narrower searches and ranges. Gemini's `read_file` and MCP `get_file_contents` count as file reads.

## [0.14.0] - 2026-11-07

### Added
- **`girth advise --harness codex` flags large tool output.** `girth import` now measures each tool result in a Codex rollout. One of 8,000 characters or more is recorded with its turn as `tool_output`, keyed by tool name (`exec`, `js`, `shell`), along with the turn's last request size (`context`). Codex logs results after its own truncation, so this is what the model was sent. Codex's fix suggests only `| tail` and `| head`, since RTK filters Claude Code's Bash tool. Codex turns imported before this release keep no such data, as each rollout is read once.
- **List prices for `gpt-5.4`, `gpt-5.3-codex` and `deepseek-v4-pro`,** models Copilot and OpenCode sessions run. OpenAI lists no GPT-5 cache-write price, so a write is priced as input. DeepSeek bills by the hour, and Girth uses its off-peak rates; peak hours (weekdays 01:00–04:00 and 06:00–10:00 UTC) bill twice these.

- **Copilot large tool output.** `girth import --from copilot` records tool results of 8,000+ characters with their turn (`tool_output`, by tool name) and each turn's last request size (`context`), in stretches read from per-request records. `content`, what the model was sent, is measured; `detailedContent` is not. Stretches read from shutdown totals get neither, because one such turn spans hundreds of requests and their compactions.
- **Gemini CLI large tool output.** `girth import --from gemini` records tool calls whose `functionResponse` output (what the model was sent; `resultDisplay` is not counted) is 8,000+ characters, and each turn's last request size (`context`).
- **OpenCode large tool output.** `girth import --from opencode` records tool results (`part.state.output`, which OpenCode caps at 50 KB before sending) of 8,000+ characters with their prompt's turn, and each turn's last request size (`context`).

### Changed
- The contributor "large Bash output" is now "large tool output", since other harnesses' tools aren't Bash. Its `kind` in `--json` is still `tool_output`.
- Its fix names what the large results came from. Each kind of tool with at least a fifth of the large tokens gets its own fix, largest first: shell commands (`| tail -n 40` or `| head`, plus RTK in Claude Code only), file reads (read by line range), and fetched pages (fetch in a subagent so only its summary stays).

### Fixed
- Docs said Copilot logs per-request records from 1.0.105; they start at 1.0.95.
- A subagent turn on a non-Claude model was priced with Anthropic's 5-minute cache-write rate (1.25× input). Other vendors charge a subagent's writes the same as the main loop's, so Gemini, GPT-5 and DeepSeek subagent turns were slightly overpriced. None of the author's imported turns were affected; OpenAI's GPT-6 write rates already equal 1.25× input.

## [0.13.0] - 2026-11-07

### Added
- **Copilot spend per prompt, before the session ends.** Copilot CLI 1.0.105+ logs every request (`session.usage_record`). `girth import --from copilot` now reads those: one turn per prompt (per model), recorded as soon as the prompt's work is done, so a running session counts without waiting for its shutdown. Stretches without records (older Copilot versions) are still read from their shutdown totals, and a stretch is never counted both ways. Sessions already imported by 0.12 are not counted again: the new record cursor sits beside the old one (`<file>#records` in `import.json`).
- **`girth advise` flags large Bash output.** The mod now measures each Bash result; one of 2,000 estimated tokens or more (4 characters per token) is recorded with its turn (`tool_output`). `advise` prices it as written to the cache once and re-read by every later request until the conversation shrinks, and suggests `| tail -n 40`, `| head` or a shell-output filter such as RTK. Only Bash is measured: other tools' results aren't documented to mods, and many are images. Turns also keep their last request's context size (`context`), which marks where a conversation was compacted or cleared.

### Fixed
- The plugin marketplace (`.claude-plugin/marketplace.json`) now has its own description, which `claude plugin validate` asked for.

## [0.12.0] - 2026-11-06

### Added
- **`girth stats --harness <id>`** limits skill uses and turns to one harness (`claude-code`, `codex`, `copilot`, `opencode`, `gemini`), like `advise --harness`. By default, `stats` counts skill uses from every harness Girth has imported.
- **One project cache across harnesses.** `girth validate` links each valid crafted skill in `<project>/.claude/skills` into `<project>/.agents/skills` with a relative symlink, and the reverse. Claude Code reads the first folder and most other harnesses the second, so a skill crafted in one now loads in both. An existing entry on the other side is never replaced. A link that can't be made (for example on Windows without symlink rights) is noted on stderr, and validation still passes. Discovery lists a linked skill once.
- **`girth import --from copilot`** (and `all`, the default) appends GitHub Copilot CLI spend from `$COPILOT_HOME/session-state` (default `~/.copilot`). There is one turn per model each time a session shuts down, with `harness: "copilot"`, the request count, the premium requests used, and the token counts. Skills the session ran (`skill.invoked`) are recorded as uses. A re-run adds only new shutdowns. Copilot resumes either restart their totals or carry them over, depending on the version; Girth tells the two apart so nothing is counted twice (see `src/import/copilot.rs`).
- **`girth import --from opencode`** appends OpenCode turns, one per prompt, read through OpenCode's own `opencode db --pure` (so Girth links no SQLite code). It runs only when `$XDG_DATA_HOME/opencode/opencode.db` (default `~/.local/share/opencode`) already exists; `GIRTH_OPENCODE_BIN` names another binary. Each turn keeps OpenCode's own per-request cost (`cost_micros`), which `advise` uses in place of list prices. A turn is recorded as aborted when it logged an error or was cut off by the next prompt. `skill` tool calls are recorded as uses. An OpenCode that can't be run imports nothing and says so on stderr.
- **`girth import --from gemini`** appends Gemini CLI turns from `$GEMINI_CLI_HOME/.gemini/tmp/*/chats` (default `~/.gemini`), one per prompt. It reads both session formats: whole JSON files, which Gemini rewrites, and JSON lines, where a message written again replaces the earlier copy. Each project's folder comes from its `.project_root`. `activate_skill` calls are recorded as uses.
- **Google list prices** for Gemini 3 Flash, 3.1 Flash-Lite, 2.5 Pro and 2.5 Flash (standard tier, prompts up to 200K tokens). `gemini-3-pro-preview` is no longer on Google's price page, so it stays unpriced.
- **`girth advise` reports Copilot premium requests** beside the list-price estimate (JSON: `premium_requests`), because Copilot bills by premium request rather than by token. Each `--harness` names that harness's own fresh-session command and instructions file: `/new` and `AGENTS.md` for Codex, `/clear` and `AGENTS.md` for Copilot and OpenCode, `/clear` and `GEMINI.md` for Gemini CLI.
- **List prices for Claude Sonnet 4.6, Opus 4.6 and Haiku 4.5**, the models in older Copilot sessions.

### Fixed
- The Codex and Cursor plugin icon (`assets/logo.svg`) was rejected as unusable because it declared an external DTD; it no longer does (`claude plugin validate` flags it).
- CLI tests no longer read the machine's own Codex, Copilot or OpenCode data.
- The keeper's warm-return count no longer counts another harness's imported turn in the same folder as Claude Code coming back.

## [0.11.0] - 2026-11-06

### Added
- **Every harness's skill folders.** Default roots now add each skill folder that exists for the 79 harnesses in the `skills` CLI's table (`src/harness_table.rs`, regenerated by `sh scripts/sync-harnesses.sh`). They also add folders the harnesses' own docs name: `~/.codex/skills`, `.cursor/skills`, `.github/skills`, `.opencode/skills` and `.gemini/skills`. A folder reached through a symlink is searched once. Project folders that aren't dot-dirs (such as `skills/`) are never added. Explicit roots are unchanged.
- **`girth import [--from codex|all]`** appends Codex turns from `$CODEX_HOME/sessions` (default `~/.codex`) to the ledger, at their own times. Each turn carries `harness: "codex"`, the model, the project, the session and the token counts; no content is copied. A re-run adds only new turns. A turn that never logged its end is recorded as aborted once a later turn ends.
- **`girth advise --harness <id>`** keeps one harness's turns. `--json` gains `harnesses` (spend per harness) and `unpriced` (turns on models without known prices, left out of spend). Codex advice names `/new` and `AGENTS.md`.
- **Plugin manifests for other harnesses:** `.codex-plugin/plugin.json` plus a repo marketplace (`.agents/plugins/marketplace.json`), so `codex plugin marketplace add mohessaid/girth` works; `.cursor-plugin/plugin.json`; and `gemini-extension.json`. A test keeps all four manifests at the crate's version.
- **OpenAI GPT-6 list prices** (standard tier, short context) for imported Codex turns.

### Changed
- **Crafted skills go to `.agents/skills` outside Claude Code.** Codex, Cursor, Copilot, Gemini CLI, Antigravity, OpenCode and most other harnesses read it. Under Claude Code (`CLAUDECODE=1`) the default is still `.claude/skills`, the only project folder Claude Code reads. `--output`, `GIRTH_OUTPUT_DIR` and config `output` still win.
- `girth advise --brief`, the line shown at Claude Code's session start, counts Claude Code turns only.

### Fixed
- `girth --help` showed `validate` with no summary and `advise` with validate's.

## [0.10.0] - 2026-10-26

### Added
- **`girth advise [--since 24h|7d] [--brief] [--json]`** ranks what's costing you, from the local ledger, at list prices (shares overlap). It covers skills, large conversations, turns more than 8 h into a session, cache rewrites after idle and subagents on an expensive model. A skill stays in context once loaded, so it is charged for every later turn of its session (a subagent's skill, for that subagent's next turn); ledgers from before sessions were recorded fall back to the turn it ran in. Each contributor comes with a fix that names a Girth lever or a Claude Code command, and the guard that shows the fix keeps things working. Text output hides contributors under 1%; `--json` lists them all.
- **The ledger records session ids:** the mod adds the Claude Code session to each turn, and the Skill hook adds it, and whether a subagent ran the skill, to each use. Older lines read as before.
- **Always in scope:** after the first main answer of each session, the mod shows `girth advise --brief`. That is one line, at most 120 characters, addressed to you, and only when one contributor is at least 15% of spend. It is never an instruction to Claude. The shipped skill gains an intent, *cut usage or costs* (`references/advise.md`): Claude applies one fix at a time, the one you pick, with its guard.
- **`girth drift <skill-dir>…`** reports whether a crafted skill's sources have moved on since it was validated. It re-hashes each source section named by a `<!-- src: skill#slug -->` marker and compares the result with the hash recorded at craft time. Each finding is one line: `changed`, `missing` (the heading was renamed or removed) or `unavailable` (the source is on no configured root). It exits 1 when there's drift. Hashes use normalised text, so reflowed text and emphasis changes don't count. The shipped skill runs it on every cache hit. Revalidating an unchanged crafted skill keeps the old baseline; only an extend, which changes the body, re-baselines it. Without that, any `girth validate` after a source update would have silenced the drift.

### Changed
- A crafted skill's `## Record` step now begins "If Girth is installed". Crafted skills load on any Agent Skills platform, and only Girth's hosts have the `made` command.
- **Crafted skills follow the Agent Skills spec's `metadata` rule** (agentskills.io: a map from string keys to string values). That standard moved to the Linux Foundation's Agentic AI Foundation on 2026-10-22, and more than 70 agent platforms read it. The template now writes flat keys: `girth-sources` (comma-separated), `girth-crafted-for`, `girth-created` and `girth-last-used`. Section provenance stays in the body's `<!-- src: … -->` markers.
- `girth validate` accepts both the flat form and the nested `metadata.girth` block written before this release, so existing crafted skills stay valid and are still recognised as crafted. In the flat form, a `girth-*` value that isn't a string is an error; other tools' keys, such as the skills CLI's boolean `internal`, are left alone.

### Fixed
- The launcher installs its cleanup traps before it creates the download's temp file. An interrupt in that window used to leave `.girth-download.*` in `bin/`.
- The repo's `girth-video` skill is now marked `internal`, so the skills CLI doesn't offer it for install.

## [0.9.0] - 2026-10-16

### Added
- **Cost per outcome:** Meter labels each turn's Bash calls as test runs (passed, failed or no verdict), commits and pull requests.
  - `/girth-meter` and `girth stats --meter` add a line with the cost per commit, the cost since the last commit and the test verdicts. `girth stats --json` has the same counts under `meter`.
  - Verdicts come from the test runner's summary, not the exit status, which piped output hides.
  - Only the labels are stored; command text and output never reach the ledger, and `share` never reads them.
- **Research:** `examples/outcomes/analyze-outcomes.py` and §6 of `docs/research/2026-10-15-where-the-money-goes.md`. On the author's history a commit cost $3.28, and 35% of test runs failed where exit status said 1%.

## [0.8.0] - 2026-10-16

### Added
- **Idle compaction (opt-in):** after 55 minutes idle, Girth compacts a conversation of at least 150,000 tokens while its cache is still warm. This replaces the keeper's pings for that conversation.
  - The next answer carries a line saying so, and `girth stats --meter` counts compactions.
  - On the author's history it would have saved about $1.8K at API prices.
  - **Turn it on** with `{"compact": {"idle": true}}`. It works in interactive sessions only.
- **Subagent model (opt-in):** `{"agents": {"model": "sonnet"}}` runs built-in subagents on that model. Explicit models, plugins' agents and forks are kept.
- **New commands:** `girth compact record` and `girth settings`.
- **Research:** `examples/compact/` (the spike and `analyze-idle-compaction.py`).

### Fixed
- A line under the answer (the doctor's, and now the compaction's) repeated the whole answer above it. Claude Code's own text for `turn.complete` is the answer, which Girth had treated as another mod's line.
- Lines under the answer no longer start with their own `girth:`, because Claude Code already names the plugin.

## [0.7.1] - 2026-10-15

### Fixed
- `/girth-meter` and `/girth-xray` now say "unavailable right now" when the binary times out or can't run. Before, the command was skipped without a word.
- Meter priced subagent turns at the 1-hour cache-write rate. Subagents write the 5-minute cache, at 1.25× input, so their estimates were too high.

### Added
- **Research:** `docs/research/2026-10-15-where-the-money-goes.md`, with `examples/subagents/analyze-subagents.py` and `examples/context/analyze-context.py`. On the author's transcripts, subagents are 6% of spend, and cache reads of a large context are 62%.

## [0.7.0] - 2026-10-15

### Added
- **Keeper:** while a conversation sits idle, Girth pings it (a one-character fork) every 55 minutes, at most 3 times. Claude Code's prompt cache lives one hour, and a return after that writes the whole conversation again at twice the input price.
  - On the author's machine, 62% of all cache writes were such returns. Simulated on that history, the keeper saves about $470 of rewrites for about $56 of pings (`examples/cache/analyze-breaks.py`).
  - A ping that finds the cache already gone stops the pings. Pings only happen for conversations of 20,000 tokens or more.
  - **Controls:** `keeper.enabled`, `keeper.max_pings` and `keeper.min_context` in `config.json`; `GIRTH_KEEPER=off`.
- **Doctor:** a turn that re-wrote at least 20,000 cached tokens gets one line under the answer, saying whether the cache had expired (with the idle time) or the prompt had changed. `girth stats --meter` totals rewrites, pings and warm returns.
- **Ledger:** `ping` events and `breaks` on turns, numbers only. `share` ignores both.
- **New commands:** `girth keeper record`; `girth meter record` now prints the keeper plan and the doctor line as JSON for the mod.

### Fixed
- Meter priced cache writes at the 5-minute rate; Claude Code writes the 1-hour cache, at twice the input rate. Estimates now reproduce Claude Code's own cost figure (they were about 25% low).

### Changed
- The planned loop breaker was dropped on evidence. In 32,787 real tool calls, an identical failing call reached a third attempt twice (`examples/cache/analyze-loops.py`).

## [0.6.0] - 2026-10-15

### Added
- **Lens for MCP and tools.** The mod also rewrites the MCP server instructions (`mcp_instructions_delta`) and the deferred-tools list (`deferred_tools_delta`).
  - **Kept in full:** servers this project used, servers used anywhere in the last 30 days, new servers, and pins (`lens.keep` accepts server ids).
  - **Shortened:** other servers' instructions collapse to one line naming them, and their tools to one line per server (`mcp__<id>__* (<n> tools; ToolSearch finds them)`). No server is disconnected, and no server name is lost.
  - **Just in time:** the first call to a collapsed server's tool in a session returns that server's held-back instructions. Claude reads them and calls again.
  - **Measured on the author's setup:** MCP instructions went from 14,258 B to 287 B and the tools list from 7,234 B to 2,797 B. Cost per run fell 29% against v0.5 and 42% against Lens off, at equal quality. Hidden-server tasks passed 6 of 6: Atlassian with held-back instructions, Excalidraw with collapsed tools (`examples/lens/RESULTS.md`).
  - **Controls:** `GIRTH_LENS_KINDS=skills,mcp,tools` limits Lens to some texts; `girth lens --kind skills|mcp|tools`.
- **X-ray:** `/girth-xray` (and `girth xray [--json]`) shows what the last request was made of: each attachment, system section and tool description, with bytes, share, tokens and cost, plus what Lens removed.
- **Meter:** turn records list the MCP servers used (ids only), which feeds the MCP Lens.
- **SkillsBench result** (`examples/skillsbench/RESULTS.md`): on 6 tasks, Girth-distilled skills (2–4 KB) matched the curated ones (8–82 KB) task for task, with 23% fewer output tokens. Curated skills gave no net lift over no skills on this subset.

## [0.5.0] - 2026-10-15

### Added
- **Lens:** a per-project skill listing, delivered by a small mod (`hooks/girth.js`, Claude Code 2.1.287+) that rewrites the `skill_listing` Claude reads on every request.
  - **Kept in full:** skills this project uses, skills used anywhere in the last 30 days, skills new in the last 14 days, crafted `girth-*` skills, project skills and pins.
  - **Shortened:** everything else collapses to its name plus a ≤ 60-character hint. No name is ever lost, and nothing is uninstalled.
  - **Measured on the author's 74-skill setup:** the listing went from 30,027 B to 7,869 B (−74%), and cost per run fell 19% at equal quality. A collapsed matching skill was still invoked in 3 of 3 runs (`examples/lens/`).
  - **Controls:** `lens.keep` and `lens.enabled` in `.girth/config.json` or `~/.girth/config.json`; `GIRTH_LENS=off`.
- **Meter:** per-turn token usage (numbers and a model id only) is written to the local ledger as `turn` events, with the request count and the Lens byte counts.
  - `girth stats` shows totals, an estimated cost (`src/price.rs`) and Lens savings. `girth stats --meter [--project DIR]` shows the meter alone, and `/girth-meter` shows it inside Claude Code.
  - `share` never reads `turn` or `seen` events.
- **New commands:** `girth lens [--project DIR] [--json]` (reads a listing from stdin) and `girth meter record`.
- **Search reach:** the default skill roots now include the `skills` folders of installed Claude Code plugins, so `girth search` reaches plugin skills.
- **SkillsBench proof harness** (`examples/skillsbench/`): three arms in local Docker (no skills, curated, Girth-distilled). The Girth arm is crafted and every crafted skill validates; the agent runs wait on a credential choice.
- **Tests:** mod tests (`claude plugin test`) are part of `make check`.

### Fixed
- `make dist` no longer copies the mod's tests into the dist plugin.

## [0.4.1] - 2026-10-15

### Fixed
- `npx skills add mohessaid/girth` no longer installs the repo-internal `girth-skill-authoring` skill. It is now marked `metadata.internal`.
- The launcher's one-time binary download is bounded (10 s connect, 120 s total, 2 retries) and announced on stderr. A stalled network used to hang the session silently, and an interrupt left a temp file in `bin/`.
- The README's install section now shows the two-step `/plugin marketplace add mohessaid/girth` then `/plugin install girth@girth`, which works the same in a session and in the shell. (The earlier one-line `/plugin install girth --marketplace mohessaid/girth` was also valid in a session from 2.1.275; its shell form needs 2.1.292.)

### Added
- The README compares plugin and skills-CLI installs: the usage ledger and automatic taste delivery need the plugin's hook.
- `examples/demo/demo.sh` and `make demo`: a recorded walkthrough of real `girth` commands on public skills, with no model calls.

## [0.4.0] - 2026-10-15

### Added
- `girth fetch owner/repo[/path]` shallow-clones a public GitHub skill repo into an untrusted scratch dir (`~/.girth/fetch/`).
  - Only plain `owner/repo` specs are accepted, so no URL, ssh, file transport or flag can reach git.
  - Git hooks are disabled, submodules are skipped, and only the https protocol is allowed.
- `girth show <skill-dir> [--heading H]…` prints a skill's frontmatter and table of contents, or only the named sections (with their subsections). Crafting now reads sources through it, so it needs no reads outside the project and never loads whole files.
- Remote fallback (`references/remote.md`): when no installed skill covers a need, Girth finds candidates with `npx skills find`, fetches at most two and distils from them.
- M2 evals: `auto-captions` (Girth fires unprompted on a specialised task: 3 of 3), `no-fire-generic` (it stays silent on a generic task: 0 of 3 fired) and `extend-captions` (a second need extends the existing skill).
- `examples/compounding/`: the 5-session compounding benchmark. Girth's persistent cache cost $0.599 over five HyperFrames sessions, against $0.909 for bulk-installed skills (−34%) and $1.061 for per-session distillation (−44%). Sessions 2–5 cost about half of a bulk session, and every session passed the HyperFrames check.

### Fixed
- The `girth show` overview is labelled "headings only", so it can't be mistaken for the whole skill.

## [0.3.0] - 2026-10-14

### Added
- **Taste memory**: house rules and a variation history per domain, for recurring deliverables (launch videos, decks, landing pages).
  - New commands: `girth taste`, `girth taste show|add|drop`, `girth made`.
  - Two tiers: personal (`~/.girth/taste.jsonl`, follows you across projects) and project (`.girth/taste.jsonl`, can be committed). Local only; `share` never reads it.
  - The `PostToolUse` hook now returns the taste as `additionalContext` when a crafted `girth-<domain>` skill, or one of its source skills, is used. Lines are pre-filtered, so a 1,000-event taste file adds about 1 ms.
  - `validate` links a crafted skill's source skills to its domain.
  - `stats` shows taste counts, never rule text.
- The skill gains a taste section and `references/taste.md`, and triggers when the user states a lasting preference. Always-on cost: 74 → 92 tokens.
- New eval `taste-launch`: a house rule is applied, recent choices are varied, and the deliverable is recorded (3 of 3 runs at 1.00).
- The launcher exports `GIRTH_LAUNCHER`, so taste context names the exact command to run.

### Fixed
- A third-party skill whose name happens to start with `girth-` is no longer counted as crafted. A crafted skill must also carry a `metadata.girth` block.
- `search` printed the warning for an invalid `.girth/config.json` twice. It now prints it once.

## [0.2.0] - 2026-10-14

### Added
- **Local skill ledger** (`~/.girth/ledger.jsonl`, overridable with `GIRTH_HOME`): an append-only record of skill uses, crafted-skill versions and feedback. A truncated line is skipped, never fatal.
- **Automatic use tracking:** a `PostToolUse` plugin hook runs `girth record use`. It takes ~1.5 ms and never fails the session.
- **Section provenance:** crafted sections carry `<!-- src: skill#slug -->` markers. `girth validate` records source coverage (`used/total`) and diffs each version, showing which sections were added and removed.
- `girth feedback <skill> up|down [--note]`. Notes stay local.
- `girth stats [--json]`: crafted skills, uses, removals, ratings, source coverage and most-used skills.
- `girth share status|allow|deny|preview`: consent is asked once by the skill. The preview shows the exact anonymous payload (source-section hashes and counts only). **Nothing is uploaded in 0.2.**
- Skill flows: review (remove sections through a multi-select), feedback, stats and sharing, loaded on demand from `references/`.

### Changed
- The `girth validate` VALID line now reports `(recorded: +A −R sections)`. Use `--no-record` to skip recording.
- Search headings ignore HTML comments, so provenance markers don't affect ranking.

## [0.1.1] - 2026-10-14

### Added
- Signed releases: Sigstore keyless bundles (`*.sigstore.json`) and GitHub build-provenance attestations for every binary and `SHA256SUMS`.
- Continuous fuzzing of the frontmatter parser and validator (`cargo fuzz`, 6.8M local executions, no crashes).
- CI quality signals: Codecov coverage, an MSRV (1.85) check, cargo-deny, CodeQL, OpenSSF Scorecard and Dependabot.
- Community files: CONTRIBUTING, Code of Conduct, SECURITY, issue and PR templates.

## [0.1.0] - 2026-10-14

### Added
- The `girth` CLI (Rust): `search`, `measure` and `validate`.
- The `girth` skill and Claude Code plugin, plus a marketplace manifest.
- A launcher that downloads checksum-verified release binaries (Linux musl x86_64/ARM64, macOS Intel/Apple Silicon).
- A behavioural eval suite (3 cases, all 1.00) and the hyperframes acceptance demo (38× lower always-on skill cost).

[Unreleased]: https://github.com/mohessaid/girth/compare/v0.19.0...HEAD
[0.19.0]: https://github.com/mohessaid/girth/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/mohessaid/girth/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/mohessaid/girth/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/mohessaid/girth/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/mohessaid/girth/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/mohessaid/girth/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/mohessaid/girth/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/mohessaid/girth/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/mohessaid/girth/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/mohessaid/girth/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/mohessaid/girth/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/mohessaid/girth/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/mohessaid/girth/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/mohessaid/girth/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/mohessaid/girth/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/mohessaid/girth/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/mohessaid/girth/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/mohessaid/girth/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/mohessaid/girth/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/mohessaid/girth/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/mohessaid/girth/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mohessaid/girth/releases/tag/v0.1.0
