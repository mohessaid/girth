# Portability: do other agents load a Girth-crafted skill?

**Date:** 2026-10-23 · **Question:** the Agent Skills standard is now read by 70+ platforms. Is a skill Girth crafts discovered outside Claude Code?

**Setup:** a fresh folder containing two skills in `.agents/skills/`. Each agent was asked headless, read-only, to name every skill it has that starts with `girth-`.
- `girth-captions`, crafted live by the current template: flat, string-only `metadata` (`girth-sources`, `girth-crafted-for`, `girth-created`, `girth-last-used`).
- `girth-hyperframes`, crafted on 2026-10-14: the legacy nested `metadata.girth` block with lists.

| Agent | Version | Flat (spec) form | Legacy nested form | Notes |
|---|---|---|---|---|
| Codex CLI (`codex exec --sandbox read-only`) | 0.160.1 | **listed** | **listed** | Codex listed both in its skill inventory from `.agents/skills` and reported no load failure. It never read a body: Its shell couldn't run here: the host blocks bubblewrap (see `docs/eval-log.md`). |
| Gemini CLI | 0.61.0 | not tested | not tested | The account's tier no longer serves Gemini CLI ("migrate to Antigravity"). |
| Cursor agent | 2026.04.17 | not tested | not tested | Needs `agent login`. |

**Reading it:**
- On the one agent tested, both forms are discovered, so skills crafted before the change keep working.
- The flat form is the one the spec defines (`metadata`: string keys to string values). It is what new crafts write, and what a stricter reader will expect.
- The `## Record` step in a crafted skill calls Girth's `made` command, which only Claude Code with Girth has. Since 2026-10-23 the template says "If Girth is installed", so other agents skip it instead of failing.

## Every harness's folders and Codex spend (2026-11-06)

**Discovery.** Same machine, a folder with no `.girth/config.json`, `girth measure`:

| Build | Default roots | Skills found |
|---|---|---|
| v0.9.0 (installed plugin) | `~/.claude/skills`, `~/.agents/skills`, `.claude/skills`, plugin skill dirs | 124 |
| this change | the above, plus each harness folder that exists | **146** |

The 22 skills that were missing live only in other harnesses' folders. They are counted once even where `npx skills add` linked them from `~/.agents/skills`. The folders that exist here: `~/.codex/skills` (8 entries), `~/.gemini/skills` (11), `~/.cursor/skills` (8), `~/.copilot/skills` (8), `~/.config/opencode/skills` (14), `~/.grok/skills` (2) and `~/.config/goose/skills` (15).

**Codex spend.** `girth import` on the author's `~/.codex` (into a scratch copy of `~/.girth`):

```
$ girth import
imported 140 turns from codex
$ girth import
imported 0 turns from codex
$ girth advise --since 60d --harness codex
girth advise (60d): $173.00 estimated at list prices; shares overlap
  81%  turns 8h+ into a session
       fix: /new when you switch tasks: a fresh session re-reads only what the new task needs
  44%  large conversations
       fix: /compact <what to keep> at a natural break, or /new when you switch tasks
```

- An independent Python sum over all 4,544 `token_usage_record` lines, at the same OpenAI list prices, gives **$173.00**.
- The first build gave $170.14. The $2.87 gap was 4 turns that never logged an end event; they are now recorded as aborted once a later turn ends.
- Across both harnesses over 60 days, `advise --json` splits spend as `claude-code` $3,549 and `codex` $173.

**Codex large tool output (2026-11-07).** `girth import` now records Codex tool results of 8,000 characters or more. Same `~/.codex`, into a scratch `GIRTH_HOME`:

```
$ girth import
imported 142 turns from codex
$ girth advise --since 100000d --harness codex
girth advise (100000d): $173.45 estimated at list prices; shares overlap
  81%  turns 8h+ into a session  ($140.05)
  44%  large conversations  ($76.90)
  28%  large tool output  ($49.25)
       fix: end long commands with | tail -n 40 or | head
```

- 548 of 4,386 tool results are large: 515 from `exec` and 33 from `js`, about 2.4M estimated tokens in all.
- These are what the model was sent: the largest is 48K characters, and 162 carry Codex's "tokens truncated" marker.
- An independent Python pass with the same carry rule gives **$49.25**.
- **Bound:** in each session, the tokens the rule says were re-read stay under the session's logged cache reads (ratios 0.07, 0.08, 0.29 and 0.48), so the estimate is possible, though the true share may be lower.
- The 2 turns more than the 140 above have happened since.

**Copilot CLI spend.** `girth import --from copilot` on the author's `~/.copilot` (58 sessions, 96 shutdowns; into a scratch `GIRTH_HOME`):

```
$ girth import --from copilot
imported 97 turns from copilot
$ girth import --from copilot
imported 0 turns from copilot
$ girth advise --since 100000d --harness copilot
girth advise (100000d): $610.09 estimated at list prices; shares overlap
  43%  turns 8h+ into a session  ($261.10)
  14%  webapp-testing  ($85.47)
       (19 turns on models without known prices are not counted)
       (611 Copilot premium requests; Copilot's share is at list prices, not what the subscription bills)
```

- The 43% "turns 8h+ into a session" share is partly how the import works. Each turn is stamped at its shutdown, so every turn after a resume counts as hours or weeks into the session. `/clear` is still the right fix for a session resumed over weeks, but don't quote the share as a measured finding.
- Copilot logs per-model totals at each shutdown. A resumed session's next shutdown holds either a fresh count or the running totals, depending on the CLI version; one session here has both. The first draft of the rule read one fresh count as running totals (about $70 too little). Model-tagged assistant messages settled that case: 312 Sonnet and 48 gpt-5.4 messages in that stretch.
- 16 shutdowns are empty. Of the other 80, rules settle 67 structurally (39 first, 24 where a model went down, 4 carried over unchanged), message tags settle 6, and turn counts settle 7.
- An independent Python pass with the same rule gives **$610.09**. The bounds are $278.20 if every shutdown were running totals and $646.27 if every one were fresh.
- Unpriced: agnes-3.0-flash (BYOK, logged cost 0), deepseek-v4-pro, gpt-5.4, gpt-5.3-codex.

**Copilot per-request records (0.13.0).** Copilot 1.0.95+ logs each request as a `session.usage_record`. 3 sessions here carry them (96 records, all BYOK `agnes-3.0-flash` with logged cost 0), and in each the records sum exactly to the shutdown totals. Read from records, those 3 stretches become 9 turns, one per prompt, instead of 3 shutdown turns:

```
$ girth import --from copilot
imported 103 turns from copilot
$ girth import --from copilot
imported 0 turns from copilot
```

- Spend and premium requests are unchanged: **$610.09** and **611**, as the records sum to their shutdowns. 25 turns are now on models without known prices (19 before), as the BYOK stretches split into more turns.
- One prompt was answered by agent-initiated requests only, after a request that had stopped. A first draft grouped turns by initiator alone, so that prompt joined the turn before it, and a live import would have dropped those requests.
- 4 of the 9 turns are recorded as aborted: the next prompt cut them off mid-tool-call.

**Copilot's other models priced (2026-11-07).** List prices for `gpt-5.4` and `gpt-5.3-codex` (OpenAI's pricing page) and `deepseek-v4-pro` (DeepSeek's, off-peak rates) price 9 more turns:

```
$ girth advise --since 100000d --harness copilot
girth advise (100000d): $618.16 estimated at list prices; shares overlap
       (16 turns on models without known prices are not counted)
```

- The 9 turns add $8.06 ($610.0929 → $618.1568): $4.17 on deepseek-v4-pro (7 turns), $3.15 on gpt-5.4 (1) and $0.74 on gpt-5.3-codex (1). An independent Python sum at the same rates gives the same $8.0639.
- The 16 turns still unpriced are all `agnes-3.0-flash`, a BYOK model with no public price.
- DeepSeek bills peak hours (weekdays 01:00–04:00 and 06:00–10:00 UTC) at twice these rates. A Copilot turn is stamped at its shutdown, not when it ran, so the off-peak figure may be low.

**Copilot large tool output (2026-11-07).** A survey of all 29,359 tool results in `~/.copilot` found:
- **706 are large** (8,000+ characters of `content`): `view` 334, `bash` 265, `web_fetch` 33.
- **`content` is what the model was sent.** 36 large results have a longer `detailedContent`, the user-facing text, and Bash results stop at about 26K characters. The largest results are whole-file views, up to 91K characters.
- **Only stretches read from per-request records get `tool_output`.** A shutdown stretch is one turn of hundreds of requests, with 291 compactions across the machine, so the cost model can't tell what a result was re-read by.
- **Records depend on the Copilot version, not on BYOK.** A one-prompt 1.0.106 run on GitHub's `gpt-6-luna` (0.22 AI credits; its session was deleted afterwards) wrote a `session.usage_record`.
- **The record stretches here** hold 2 large Bash results (3,736 and 2,320 estimated tokens), both found by Girth and by an independent pass. They are on the unpriced BYOK model, so `advise` shows no share yet. Spend is unchanged at $618.16.

**OpenCode spend.** `girth import --from opencode` reads OpenCode's database through `opencode db` (into a scratch `GIRTH_HOME`):

```
$ girth import --from opencode
imported 11 turns from opencode
$ girth import --from opencode
imported 0 turns from opencode
```

- An independent Python read of `opencode.db` gives $0.283301 over 135 requests and 11 prompts. Girth's ledger gives the same: 11 turns, 135 requests, $0.283301, 0 unpriced, 1 aborted, 4 in subagent sessions, plus 2 skill uses (`career-ops`, `girth`).

**Gemini CLI spend.** `girth import --from gemini` on the author's `~/.gemini` (23 sessions; into a scratch `GIRTH_HOME`):

```
$ girth import --from gemini
imported 45 turns from gemini
$ girth import --from gemini
imported 0 turns from gemini
```

- The ledger holds 260 of 270 requests, $1.5537 at Google list prices. 13 turns on `gemini-3-pro-preview` are unpriced, because Google's page no longer lists that model.
- An independent Python pass over every request gives $1.6449. The 10 requests Girth left out are exactly the open turns in the 3 sessions that end mid-tool-call ($0.0911), which leaves $1.5538, equal to Girth's figure within rounding.

## Headless check per harness (2026-11-06, `sh examples/portability/run.sh`)

- **Setup:** a throwaway git folder with the shipped `girth` skill (binary bundled) and a crafted `girth-hyperframes` in `.agents/skills`.
- **The task:** list every `girth*` skill, then use the girth skill to run `search` for "video captions" and print `output_dir`. One read-only prompt per harness, each on its own provider's credits.
- **Transcripts:** `examples/portability/runs/2026-11-06/`.

| Harness | Version | Lists `girth` and `girth-hyperframes` | Ran `girth search` through the skill | `output_dir` |
|---|---|---|---|---|
| Codex CLI | 0.164.0 | ✓ | ✓ | `.agents/skills` |
| GitHub Copilot CLI | 1.0.106 | ✓ (via its `skill` tool) | ✓ | `.agents/skills` |
| OpenCode | 1.17.11 | ✓ | ✓ | `.agents/skills` |
| Antigravity CLI (`agy`) | 1.5.2 | ✓ | ✓ (needed `--dangerously-skip-permissions`: headless `agy` cannot ask) | `.agents/skills` |
| Gemini CLI | 0.65.0 | not run: Google no longer serves it to individual accounts ("migrate to Antigravity") | | |
| Cursor agent | 2026.04.17 | not run: needs `agent login` | | |

**Two things found.**
- **A harness started from a Claude Code shell inherits `CLAUDECODE=1`,** so Girth wrote to `.claude/skills` there. The first Codex run did exactly that. A harness started from a normal terminal gets `.agents/skills`. The script now clears `CLAUDECODE`, as a normal terminal would.
- **Gemini CLI warned that `~/.agents/skills/figma` overrides `~/.gemini/skills/figma`.** The same skill installed for two harnesses is common, which is why Girth's discovery searches each folder only once.
