# `girth advise`: usage insights always in scope

**Date:** 2026-10-25 · **Status:** draft

## Why

On 2026-10-25 the author hit 98% of a weekly plan limit. Claude Code's `/usage` explained it ("start fresh when you switch tasks 95%", "`/playwright-cli` 16% of usage", sessions running 8 h+). It is an approximate snapshot you have to open, and its fixes are generic.

**Target numbers** (the author's `/usage`, 2026-10-25, last 24 h, sessions on this machine only, shares overlap). These are the acceptance target for the dogfood run:

| `/usage` tip | Share |
|---|---|
| Start fresh when you switch tasks (`/clear`) | 95% |
| Give simple subagents a cheaper model | 61% |
| Stop sessions you no longer need (some ran 8 h+) | 61% |
| Run `/playwright-cli` on a cheaper model | 16% |
| Run fewer sessions at once (up to 5) | 13% |

Per-skill share: `/playwright-cli` 16%, `/kestrelle:kestrelle-workflow` 3%, `/writing-ics-ansible-roles` 1%.

The author's ask: Girth should hold these insights **always**, and help optimise **while keeping things working**. Girth already records every turn (tokens, cache breaks, subagent turns, MCP servers, outcomes) and owns the levers: Lens, distillation, keeper, idle compaction and the subagent model. What it lacks is the bridge from "what's costing you" to "the Girth lever that fixes it, and the check that proves nothing broke."

## What

### 1. `girth advise [--since 24h|7d] [--json] [--brief]`

It reads the ledger and ranks **contributors**: each has a share of spend (`price.rs` at list prices, labelled approximate), a one-line fix that names a Girth lever or a Claude Code command, and a **guard** that says why the fix keeps things working.

| Contributor (from the ledger) | Fix | Guard |
|---|---|---|
| One skill's turns dominate (per-skill attribution, new) | `girth` distils the slice this project uses | `girth validate` + `girth drift`; the source stays installed |
| Always-on listing bytes × requests (`lens_before`) | turn Lens on | Lens keeps every skill used here or recently, plus all `girth-*` |
| Cache rewrites after idle (`breaks`) | keeper or idle compaction | keeper only pings; compaction is opt-in and keeps the named items |
| Context per turn grows past N (cache_read per turn) | `/compact <what to keep>` at a natural break, or `/clear` on a task switch | the summary keeps the named details |
| Subagent turns on the main model (`agent`) | `{"agents":{"model":"sonnet"}}` | code edits stay on the main model |
| Turns 8 h+ into a session (`Turn.session`, new) | `/clear` on a task switch | CLAUDE.md, project skills and the ledger carry over; `/compact` if the work goes on |
| MCP servers connected but unused (`mcp`) | Lens collapses them | servers that are used are kept |
| Large tool output (`tool_output`, added 2026-11-06; Codex 2026-11-07) | end the command with `\| tail -n 40` or `\| head`, or use a shell-output filter such as RTK (Claude Code only) | `tail` keeps the summary and errors at the end; a filter keeps errors and summaries |

- **`--brief`** prints only the top contributor above a threshold (≥ 15% of spend in the window) whose fix isn't already on. If there is none, it prints nothing.
- **Frozen-interface rule:** a new command and new JSON. Nothing existing changes.

### 2. Always in scope

- **`session.start` (mod):** run `girth advise --brief` once per session.
  - The line is addressed to the **user** and states a fact, at most 120 characters.
  - Example: `girth: 16% of your spend this week followed /playwright-cli; distil it next time you need it.`
  - It is never an instruction to Claude. A tip injected into context must not derail the task in hand into unprompted crafting or compaction, and a long line would make Girth the always-on cost it exists to remove.
  - It is silent when no contributor clears the threshold.
- **Shipped skill:** a new row in "Other intents", *cut usage / why is this costing so much*, points to `references/advise.md`. Claude acts on advice only when asked there. It runs `girth advise` and applies one fix at a time, with its guard.
- **Shares overlap**, as in `/usage`: a turn in which two skills fire counts toward both.

### 3. New ledger fields (additive, `#[serde(default)]`)

- `Turn.session`, and `Use.session` plus `Use.agent` from the Skill hook's `session_id` and `agent_id`. `Turn.skills` was dropped: the hook already records every Skill call, in subagents too (checked against transcripts, Kestrelle #18).
- **Residency:** a loaded skill stays in context, and is re-injected after compaction, for the rest of its session. A main-loop use charges every later main turn of that session, a subagent's use its next subagent turn. This is how `/usage` can show a skill at 16% that ran once.
- Old lines parse unchanged and fall back to charging the next turn of the same project.

## Addendum (2026-11-06): large tool output

Evidence: `examples/context/RESULTS.md`. Tool traffic is most of what a conversation re-reads even with a shell filter on, so the share of *all* tool output says nothing a user can act on. What a filter or `| tail` fixes is single large results.

- **Measured:** in the mod's `tool.call`, each Bash result's full text (stdout and stderr, before the outcome tail is cut) at 4 characters per token, an estimate. A result of 2,000 estimated tokens or more counts as large. A turn records the sum as `tool_output: {"Bash": tokens}` (a map, so other tools can be added). Non-Bash results are not measured: their shape isn't documented to mods, and half of the author's `Read` results are images, which a length can't price.
- **Also recorded:** `context`, the last request's input tokens (with cache), on main-loop turns. The meter already received it; it is now kept.
- **Cost model:** a large result is written to the cache once and re-read by every later request until the conversation shrinks. Inside its own turn, half the turn's requests are counted (where in the turn it came isn't recorded). Then every later turn of the same session and loop (main or subagent), up to the first main turn whose `context` is smaller than the turn before's: a `/compact`, an idle compaction or a `/clear` that kept the session id. Subagent results are counted within their turn only. Without a session id, only the turn itself. This is the weighting `examples/context/analyze-tails.py` used.
- **Fix text covers both setups.** Whether a mod sees a Bash command before or after another hook (such as RTK's) rewrites it wasn't verified, so the fix names `| tail`, `| head` and a filter rather than detecting one.
- **Calibration, once real data exists:** with RTK on, the author's large results are about 4% of tool re-reads, so the contributor should stay under the 15% `--brief` threshold there. If it fires on that machine, the model overcounts.

## Addendum (2026-11-07): large tool output from Codex

Evidence: `examples/portability/RESULTS.md` ("Codex large tool output").

- **Measured:** `girth import` reads each tool result in a Codex rollout (`function_call_output` or `custom_tool_call_output`, a string or a list of text parts), named by its call (`exec`, `js`, `shell`). A result of `LARGE_OUTPUT_CHARS` (8,000) characters or more goes with the turn its line names, or the open turn, at 4 characters per token. The log holds what the model was sent: the largest of the author's 548 large results is 48K characters, and 162 carry Codex's "tokens truncated" marker.
- **Also recorded:** `context`, the input of the turn's last request, cached part included.
- **Label:** "large tool output" for every harness, since Codex's tools aren't Bash.
- **Fix text per harness:** RTK filters Claude Code's Bash tool, so other harnesses' fix and guard name only `| tail` and `| head`.
- **Bound check:** in each Codex session, the tokens the cost model says were re-read stay under that session's logged cache reads (at most 48%). A cost model that broke this bound would be overcounting.
- **Not handled:** Codex subagent threads are recorded as main-loop turns, as before.
- **Copilot (same day):** only turns read from per-request records carry `tool_output`. See the all-harnesses spec.
- **OpenCode (same day):** `part.state.output` lengths, capped by OpenCode at 50 KB, go with their prompt's turn. `context` is the last request's input plus its cache reads and writes.
- **Claude Code, every tool (same day):** a probe mod logged the shape of each `tool.call` result in a headless run. Read, Grep, Glob, WebFetch and ToolSearch all return `text`, a string holding what the model reads: Read's has line numbers, an image Read's is empty. The mod measures `text` for every tool, MCP tools included, and Bash keeps its stdout and stderr fallback. This replaces the earlier "non-Bash results are not measured".
- **Gemini CLI (same day):** each tool call's `functionResponse.response.output` length goes with its turn; `resultDisplay`, the user's copy, is not counted. `context` is the last request's `input` plus `tool`.
- **The fix follows the tools.** On OpenCode, 77% of large output was `webfetch`, which `| tail` can't trim. Tool names are grouped into shell (the default), file reads (`read`, `view`) and fetches (any name containing `fetch`). Each group with at least a fifth of the window's large tokens gets its fix and guard, largest first.

## Not in scope

- Auto-applying fixes. Girth suggests, and the user or Claude applies.
- Plan-limit percentages. Claude Code doesn't expose them to plugins. Girth reports its own measured spend and says it is approximate.
- Sharing. Everything stays local.
- Overlapping sessions. Running sessions one after another spends the same tokens as running them at once, so "fewer at once" cuts no spend and has no guard. Not a contributor.

## Tests

- **Unit:** each contributor detector on synthetic ledgers, including threshold edges and "fix already on" suppression.
- **CLI:** `--brief` is silent on an empty ledger and prints one line on a dominated one.
- **Mod:** `hooks/tests` for the session-start attach and the once-per-session rule.
- **Dogfood:** run `girth advise --since 7d` on the author's real ledger and compare it with the `/usage` screenshot above (record both).
