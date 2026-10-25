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
| Long or overlapping sessions (session id + span, new) | stop idle ones; fewer at once | — |
| MCP servers connected but unused (`mcp`) | Lens collapses them | servers that are used are kept |

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

- `Turn.session` (id) and `Turn.skills` (skills invoked in the turn). These are needed for per-skill attribution and the session-span and overlap contributors.
- Old lines parse unchanged. Before `Turn.skills` has data, per-skill attribution is approximated by matching the existing `Use` events (recorded by the `PostToolUse: Skill` hook) to turns by timestamp. This lets the dogfood run use the author's existing 7-day ledger.

## Not in scope

- Auto-applying fixes. Girth suggests, and the user or Claude applies.
- Plan-limit percentages. Claude Code doesn't expose them to plugins. Girth reports its own measured spend and says it is approximate.
- Sharing. Everything stays local.

## Tests

- **Unit:** each contributor detector on synthetic ledgers, including threshold edges and "fix already on" suppression.
- **CLI:** `--brief` is silent on an empty ledger and prints one line on a dominated one.
- **Mod:** `hooks/tests` for the session-start attach and the once-per-session rule.
- **Dogfood:** run `girth advise --since 7d` on the author's real ledger and compare it with the `/usage` screenshot above (record both).
