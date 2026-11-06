<div align="center">

<img src="assets/logo.svg" alt="Girth logo" width="160">

# Girth

**Just-in-time, project-scoped agent skills.**<br>
Distil only the slice your project needs from the skills you already have — 38× lower always-on skill cost.

<p>
  <a href="https://github.com/mohessaid/girth/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/mohessaid/girth/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://codecov.io/gh/mohessaid/girth"><img alt="codecov" src="https://codecov.io/gh/mohessaid/girth/graph/badge.svg"></a>
  <a href="https://github.com/mohessaid/girth/actions/workflows/codeql.yml"><img alt="CodeQL" src="https://github.com/mohessaid/girth/actions/workflows/codeql.yml/badge.svg"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/mohessaid/girth"><img alt="OpenSSF Scorecard" src="https://api.scorecard.dev/projects/github.com/mohessaid/girth/badge"></a>
  <a href="https://github.com/mohessaid/girth/releases"><img alt="Release" src="https://img.shields.io/github/v/release/mohessaid/girth?sort=semver"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/github/license/mohessaid/girth"></a>
  <a href="Cargo.toml"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85-orange?logo=rust"></a>
  <a href="docs/mutation-notes.md"><img alt="Mutation score" src="https://img.shields.io/badge/mutation-99.5%25-brightgreen"></a>
  <a href="docs/eval-log.md"><img alt="Evals" src="https://img.shields.io/badge/evals-7%2F7%20passing-brightgreen"></a>
  <a href="#install"><img alt="Claude Code plugin" src="https://img.shields.io/badge/Claude%20Code-plugin-d97757"></a>
  <br>
  <a href="https://agentskills.io/specification"><img alt="Agent Skills format" src="https://img.shields.io/badge/Agent%20Skills-spec-0a7ea4"></a>
  <a href="examples/portability/RESULTS.md"><img alt="Works with Codex CLI" src="https://img.shields.io/badge/works%20with-Codex%20CLI-412991?logo=openai"></a>
  <a href="examples/portability/RESULTS.md"><img alt="Works with GitHub Copilot CLI" src="https://img.shields.io/badge/works%20with-Copilot%20CLI-24292f?logo=githubcopilot"></a>
  <a href="examples/portability/RESULTS.md"><img alt="Works with OpenCode" src="https://img.shields.io/badge/works%20with-OpenCode-111111"></a>
  <a href="examples/portability/RESULTS.md"><img alt="Works with Antigravity CLI" src="https://img.shields.io/badge/works%20with-Antigravity%20CLI-4285f4?logo=google"></a>
  <a href="#every-harness"><img alt="Finds skills for 79 harnesses" src="https://img.shields.io/badge/skill%20folders-79%20harnesses-6e7781"></a>
</p>

<a href="#install">Install</a> · <a href="#how-it-works">How it works</a> · <a href="examples/studio/RESULTS.md">Results</a> · <a href="CONTRIBUTING.md">Contributing</a>

</div>

Instead of installing dozens of large skills and paying for all of them every turn, Girth distils only the slice your project needs into a small `girth-<domain>` skill, at the moment you need it. Later sessions reuse it from the project's cache.

<p align="center"><img src="assets/demo.gif" alt="Girth demo: measure 21 public skills, find the one slice a task needs, validate the small crafted skill, and remember taste" width="760"></p>

Status: v0.12, early and measured: every headline result below is re-runnable from `examples/`. Design: `docs/superpowers/specs/`.

Independent research points the same way: in [SkillsBench](https://arxiv.org/abs/2602.12670), focused skills improved task success by 18.9 points against 5.7 for comprehensive documentation, and 2–3 skills per task beat 4 or more.

Crafted skills follow the open [Agent Skills spec](https://agentskills.io/specification), the standard Anthropic [donated to the Agentic AI Foundation](https://www.anthropic.com/news/donating-agent-skills-to-the-agentic-ai-foundation) on 2026-10-22. More than 70 agent platforms, including ChatGPT, Cursor, GitHub Copilot and VS Code, read it, so any of them can pick up a skill Girth distils (Codex CLI 0.160 lists both the current and the legacy form: `examples/portability/RESULTS.md`). (Its `## Record` step runs only where Girth is installed.)

## Measured so far (one machine)

**Studio: a six-step video workflow, 8 runs per arm** (2026-10-17, claude-sonnet-5-5). The task is a 9-second portrait HyperFrames video that walks through reference → data → story → motion → direct → ship, writing each step's file and then passing check and render. It is graded by a script, not a model:

| Arm | Pass | Cost per run (mean ± 95% CI) | Fresh prompt tokens |
|---|---|---|---|
| 39 skills installed (today's default) | 8/8 | $0.206 ± 0.014 | 31.6K ± 2.0K |
| **Girth (one crafted skill, 3.8 KB)** | **8/8** | **$0.146 ± 0.017 (−29%)** | **18.5K ± 2.0K (−42%)** |

The quality checks found nothing in any of the 16 runs: 0 errors and 0 warnings. Welch's t = 6.4 (p < 0.001), and the one-time craft ($0.19) pays for itself after about 3 sessions. See `examples/studio/RESULTS.md`, with all 16 renders side by side.

<p align="center"><img src="examples/studio/sheet.png" alt="All 16 studio renders at the Motion step: every installed skill on top, Girth below" width="760"></p>

**Five sessions in one domain** (2026-10-15, HyperFrames, claude-sonnet-5-5, the tasks never mention Girth):

| Arm | Session 1 | Sessions 2–5 (each) | 5-session total |
|---|---|---|---|
| Every skill installed (today's default) | $0.18 | $0.16–0.21 | $0.909 |
| Fresh distillation every session | $0.25 | $0.18–0.22 | $1.061 |
| **Girth (crafts once, then reuses)** | $0.22 | **$0.09–0.10** | **$0.599 (−34%)** |

Replicated three more times on 2026-10-23, with every composition checked by a script: the mean five-session totals were **$0.877 bulk, $1.025 JIT and $0.564 Girth (−36% vs bulk, p = 0.03)**, and all 45 compositions passed. See `examples/compounding/RESULTS.md`.

**OpenMontage, a public 49-skill video pack** (2026-10-18, Remotion, 8 runs per arm): all 16 runs passed. Girth's median cost was 19% lower ($0.107 → $0.087), and 7 of 8 girth runs cost less than every bulk run (Mann–Whitney p = 0.007). One girth run was slowed by a concurrent render, so the means differ by only 7%. The saving is smaller than in the studio test because this pack costs a session less. See `examples/openmontage/RESULTS.md`.

**Lens: the same task with and without Lens** (2026-10-15, the author's real setup of 74 skills and 6 MCP servers, claude-sonnet-5-5, 3 runs per arm):

| | Lens off | Skills Lens (v0.5) | Full Lens (v0.6) |
|---|---|---|---|
| Cost per run (mean) | $0.166 | $0.135 (−19%) | **$0.096 (−42%)** |
| Fresh prompt tokens (input + cache write) | 32.0K | 24.7K | **15.9K (−50%)** |
| Matching skill still invoked (collapsed by Lens) | 3/3 | 3/3 | 3/3 |
| Metadata checks passed | 18/18 | 18/18 | 18/18 |

With v0.6, tasks that need a collapsed MCP server still work, 6 of 6 across two servers: the server's instructions arrive on first use and Claude follows them.

See `examples/lens/RESULTS.md`, including the pilot that showed names alone lose a needed skill.

**SkillsBench, 6 tasks × 3 runs** (2026-10-15 and 2026-10-23, claude-sonnet-5-5): Girth's one distilled skill per task **matched the task's curated skills on every task in every run** (18 of 18 task-runs; 3/6 passed in each arm, each time), with 14% fewer output tokens. Self-generated skills tend not to help (SkillsBench); skills distilled from curated sources kept their effect. See `examples/skillsbench/RESULTS.md`.

**HyperFrames captions, before and after crafting** (first run 2026-10-14, replicated with 8 runs per arm on 2026-10-23):

| | Before | With Girth (bulk skills removed) |
|---|---|---|
| Always-on skill cost (39 user skills vs crafted) | 17,022 B ≈ 4.3K tok | 448 B ≈ 112 tok (**38× less**) |
| Fresh prompt tokens per session (mean of 8) | 31.2K | 17.3K (**−45%**) |
| Task cost per session (mean of 8) | $0.195 | $0.128 (**−34%**; every girth run cheaper than every bulk run) |
| HyperFrames check warnings | in 3 of 8 runs | in 0 of 8 runs |

The studio, OpenMontage and HyperFrames results above use 8 runs per arm, compounding uses 4 full repeats, and SkillsBench and Lens use 3. Every headline result here has now been run more than once. See `examples/hyperframes/RESULTS.md` for the full numbers and caveats, and `docs/eval-log.md` for the behavioural eval suite (7 cases, 21 runs, all 1.00).

## Install

**As a Claude Code plugin (recommended).** In Claude Code (2.1.275 or later):

```
/plugin marketplace add mohessaid/girth
/plugin install girth@girth
```

**As a Codex plugin** (Codex CLI 0.164 or later; the install was checked from a local copy of this repo):

```bash
codex plugin marketplace add mohessaid/girth
codex plugin add girth@girth
```

**As a Gemini CLI or Antigravity extension.** The repo root holds `gemini-extension.json`, and the skill is in `skills/`:

```bash
gemini extensions install https://github.com/mohessaid/girth
```

The repo also carries a `.cursor-plugin/plugin.json` for the Cursor Marketplace. Girth isn't listed there yet.

**As a plain skill, for any agent the [skills CLI](https://skills.sh) supports:**

```bash
npx skills add mohessaid/girth
```

| | Plugin | Skills CLI |
|---|---|---|
| Craft, reuse, `show`, `fetch`, `taste`, `made` | ✓ | ✓ |
| Usage ledger (`girth stats` knows which skills you actually use) | ✓ | crafts only |
| Taste delivered automatically when a domain skill loads | ✓ | ask *"girth taste &lt;domain&gt;"* |
| Lens and Meter (Claude Code 2.1.287 or later; MCP Lens and X-ray tested on 2.1.300) | ✓ | — |

The plugin registers a `PostToolUse` hook on skill loads (about 1 ms) and, on Claude Code 2.1.287 or later, a small mod (`hooks/girth.js`) for Lens and Meter. A skills-CLI install registers neither, so those parts are plugin-only.

### Every harness

Girth's core is a plain binary and an [Agent Skills](https://agentskills.io/specification) skill, so it is not tied to Claude Code:

- **Finds skills wherever your harnesses keep them.** It searches each skill folder that exists, for all 79 harnesses in the [`skills` CLI's table](https://github.com/vercel-labs/skills/blob/main/docs/agents.md), plus folders their own docs name (`~/.codex/skills`, `.cursor/skills`, `.github/skills`, `.opencode/skills`, `.gemini/skills`).
- **Writes crafted skills where your harness reads them:** `.agents/skills`, which Codex, Cursor, Copilot, Gemini CLI, Antigravity, OpenCode, Cline, Amp and most others read. Under Claude Code it writes `.claude/skills` instead. `girth validate` then links each crafted skill into the other folder, so a skill crafted in one harness loads in all of them, and the project cache keeps compounding when you switch harnesses.
- **Counts what other harnesses spend.** `girth import` reads the local session logs of Codex, GitHub Copilot CLI, OpenCode and Gemini CLI. `girth advise --harness codex` (or `copilot`, `opencode`, `gemini`) ranks them with that harness's own fixes. On the author's machine, each total matched an independent sum to the cent:
  - 140 Codex turns, $173.00 at OpenAI list prices;
  - 103 Copilot turns, $610.09 at Anthropic list prices (one per prompt where Copilot 1.0.105+ logs each request, otherwise one per session stretch);
  - 11 OpenCode turns, $0.28 by OpenCode's own per-request cost;
  - 45 Gemini CLI turns, $1.55 at Google list prices (13 turns on `gemini-3-pro-preview`, which Google no longer prices, are labelled unpriced).

  Copilot bills premium requests, not tokens, so `advise` reports those beside the list-price figure (611 here).

| Harness | Finds its skills | Crafts into a dir it reads | Spend import | [Checked with a real run](examples/portability/RESULTS.md) |
|---|---|---|---|---|
| Claude Code | ✓ | `.claude/skills` | native (the mod) | ✓ |
| Codex CLI 0.164 | ✓ | `.agents/skills` | ✓ `girth import` | ✓ |
| GitHub Copilot CLI 1.0.106 | ✓ | `.agents/skills` | ✓ `girth import` (per prompt on 1.0.105+, else per session stretch; with premium requests) | ✓ |
| OpenCode 1.17 | ✓ | `.agents/skills` | ✓ `girth import` (through `opencode db`, with its own costs) | ✓ |
| Antigravity CLI 1.5 | ✓ | `.agents/skills` | — (logs are opaque) | ✓ |
| Gemini CLI 0.65 | ✓ | `.agents/skills` | ✓ `girth import` | not yet (its free tier moved to Antigravity) |
| Cursor, Amp, Goose | ✓ | `.agents/skills` | — (no usable token data on disk) | not yet |
| The other 70 or so harnesses in the `skills` table | ✓ | per their table row | — | not yet |

Lens, Meter, the keeper and idle compaction use Claude Code's mods API, so they work only in Claude Code.

**Updating.** Claude Code auto-updates only its official marketplaces. To keep Girth current, open `/plugin` → **Marketplaces** → `girth` → **Enable auto-update**, or run `claude plugin update girth@girth`. A session that's already open keeps the version it loaded until you run `/reload-plugins`. Your ledger and taste live in `~/.girth`, shared by every session and version.

You don't need Rust. The first time the skill runs, it downloads (one time, about 2 MB, with a timeout) the prebuilt `girth` binary for your platform from [Releases](https://github.com/mohessaid/girth/releases) and verifies it against `SHA256SUMS` before running it. Builds exist for Linux x86_64 and ARM64 (static musl) and macOS (Intel and Apple Silicon). To use your own build, set `GIRTH_BIN`.

## How it works

1. During a task, the model (or you: "use girth for …") names a need.
2. `girth search` ranks the skills already on disk and the project's crafted-skill cache, and `girth show` prints only the sections that matter, so whole files needn't be read. (In practice models still open the big source files in most crafting sessions; see `examples/trigger/RESULTS.md`.)
3. If nothing local fits, `girth fetch owner/repo` makes a shallow, read-only copy of a public skill repo to search. Fetched content is treated as data and never executed.
4. The model distils only that slice into `<output_dir>/girth-<domain>/SKILL.md`: `.claude/skills` under Claude Code, `.agents/skills` under any other harness. Rules are kept verbatim, tone guidance is never compressed, and the body is ≤ 3 KB.
5. `girth validate` enforces the contract. The next time the domain comes up, the crafted skill is a cache hit.
6. On a cache hit, `girth drift` re-hashes the exact source sections the skill was distilled from. It reports any that `changed`, went `missing` (renamed or removed), or whose source is `unavailable` (uninstalled), so a cached slice doesn't silently go stale. Reflowed text and emphasis changes don't count.

**Headless and CI:** Claude Code protects `.claude/`, so a `claude -p` session can't write a new crafted skill there unless it runs with bypass permissions. Interactive sessions just ask you. In CI, either craft in a throwaway checkout with bypass, or commit the crafted skills and only reuse them. With that permission, Girth fired by itself in 10 of 10 fresh sessions and crafted in 9 (`examples/trigger/RESULTS.md`).

**What's costing you, always in view:** `girth advise` ranks your spend from Girth's local ledger. After a session's first answer it shows one line when a single cause dominates, for example `girth: 90% of your spend (7d) is large conversations. Ask me to cut it.` Each fix comes with its guard. A distilled skill is checked by `validate` and `drift`, compaction keeps what you name, and the subagent model only moves subagents. On the author's machine (2026-10-26), it matched the top causes in Claude Code's `/usage`: large conversations at 90–98% of spend, and subagents on the main model at 30% over 7 days.

**Next to a memory plugin** (such as claude-mem): a memory plugin records *what happened* (tool calls, edits, decisions) and brings back what's relevant. Girth keeps *the know-how a project needs*: the slice of your installed skills that the work calls for, plus a ledger of what it costs. They solve different problems, and they work together.

**Next to `/skill-doctor`** (Claude Code 2.1.285+): it shows which loaded skills go unused and what they cost, so you can prune them. That still leaves you a choice between keeping a large skill and paying for it, or deleting it and losing what it knows. Girth removes that choice: it keeps the slice your project uses and leaves the rest out of context.

## Lens, X-ray & Meter

**Lens** trims what Claude Code adds to every request: the skill list, MCP server instructions and the deferred-tools list.
- **Kept in full:** the skills this project uses, skills you used anywhere in the last 30 days, skills installed in the last 14 days, your crafted `girth-*` skills, the project's own skills, and anything you pin.
- **Shortened:** every other skill collapses into one line, as its name plus a short hint, so it can still be found and invoked. Nothing is uninstalled and no settings change.
- **MCP servers** this project doesn't use (going by your tool calls) collapse to one line each. Their tools stay findable through ToolSearch. The first time Claude calls a collapsed server's tool, Girth hands it that server's instructions before the call runs, so nothing the server asks for is lost.
- **Settings:** pin skills (or server ids such as `claude_ai_Figma`) with `{"lens": {"keep": ["name"]}}` in `.girth/config.json` or `~/.girth/config.json`. Turn Lens off with `{"lens": {"enabled": false}}`, or with `GIRTH_LENS=off` for one session. `GIRTH_LENS_KINDS=skills` limits it to the skill list.
- **Limit:** on machines where an organization manages Claude Code, a plugin you install yourself can't rewrite the list, so Lens stays off there.

**X-ray:** run `/girth-xray` to see what your last request was made of, part by part, with each part's share, what Lens removed and what it costs. Tool parameter schemas aren't visible to plugins, so they aren't counted.

**Meter** records each turn's real token usage in your local ledger (numbers and a model id only). Run `/girth-meter` or `girth stats --meter` to see:
- your usage for the project;
- an estimated cost;
- how much Lens kept out of the prompt.

**Cost per outcome:** Meter also counts what the money bought. For each Bash call Claude runs, Girth works out whether it was a test run (and whether it passed), a commit or a pull request. `/girth-meter` then adds a line such as `meter: outcomes: 12 commits · ≈ $3.40 per commit · ≈ $1.20 since the last commit · test runs: 20 passed, 8 failed, 2 no verdict`.
- **Test verdicts come from the runner's own summary** (`test result: FAILED`, `3 failed`, `12 tests, 1 failure`, …), not the exit status. Claude almost always pipes test output through `tail` or `grep`, so the exit status is the filter's. On the author's history, exit status called 99% of runs green; the summaries said 35% failed.
- **Only the labels are stored** (`test_fail`, `commit`, …). Command text and output never reach the ledger.
- **Use it to compare:** turn a lever on or off for a week and see whether a commit got cheaper. `examples/outcomes/analyze-outcomes.py` measures your history, locally; on the author's it was $3.28 per commit, from $0.91 to $6.41 per project.

The estimate prices the main conversation's cache writes at the 1-hour rate Claude Code uses, and subagents' at the 5-minute rate they use. This reproduces Claude Code's own cost figure for the main conversation. It leaves out background requests, such as session titles. Meter data is never shared.

**Keeper** stops you paying twice for the same conversation.
- **The problem:** Claude Code caches your conversation for an hour. Come back later and the whole thing is written to the cache again, at twice the input price. On the author's machine, **62% of all cache writes** were these returns (`examples/cache/analyze-breaks.py` measures yours, locally).
- **What Girth does:** while a conversation sits idle, Girth sends a one-character ping (a fork of the conversation) every 55 minutes, at most 3 times (about 3¾ hours of cover). A ping reads the cache instead of writing it, at about 1/40 of the cost of a rewrite at API prices. If a ping finds the cache already gone, the pings stop.
  - On the author's history, this would have saved about $470 of rewrites for about $56 of pings.
  - On a subscription the saving is in plan usage rather than dollars. Girth can't verify that plans weigh cache reads and writes the way the API prices them.
- **Settings:** pings only happen for conversations of 20,000 tokens or more. Change that with `{"keeper": {"max_pings": 3, "min_context": 20000}}`. Turn it off with `{"keeper": {"enabled": false}}`, or `GIRTH_KEEPER=off` for one session.

**Doctor:** when a turn had to write the cache again, Girth says why under the answer, for example: `girth: the prompt cache had expired (idle 3h 12m); this turn re-wrote 450K tokens (≈ $3.60)`. `girth stats --meter` totals these rewrites and the keeper's pings.

**Idle compaction (opt-in)** makes the most expensive conversations cheaper to come back to.
- **The problem:** every request re-reads the whole conversation. On the author's machine, those re-reads were **62% of spend**, and the median request carried 387K tokens.
- **What Girth does:** when a conversation of at least 150,000 tokens has sat idle for 55 minutes, Girth compacts it (the same as `/compact`) while its cache is still warm. For that conversation, this replaces the keeper's pings.
  - You come back to a ~50K conversation instead of a ~580K one.
  - The first answer after your return says so, for example: `compacted this conversation while you were away (583K → 21K tokens)`.
  - On the author's history, this would have saved about $1.8K at API prices, including compactions of sessions never returned to (`examples/compact/analyze-idle-compaction.py` measures yours, locally).
- **The trade-off:** compaction keeps a summary, not every detail. That's why it stays off until you turn it on.
- **Settings:** turn it on with `{"compact": {"idle": true}}`, and change the threshold with `"min_context"`. `"idle": false` in any config file, or `GIRTH_COMPACT=off`, turns it off.
- **Interactive sessions only.** Claude Code doesn't let plugins compact `claude -p` or SDK sessions.

**Subagent model (opt-in):** `{"agents": {"model": "sonnet"}}` runs built-in subagents (general-purpose, Explore and the like) on that model. A model Claude or the agent's definition chose is kept, as are plugins' agents and forks. On the author's machine subagents were 6% of spend, so expect a modest saving. `/girth-meter` shows each turn's model.

## Ledger & feedback

Girth keeps a local, append-only ledger (`~/.girth/ledger.jsonl`) of which skills you use, which sections of which source skills your crafted skills keep or drop, and your 👍/👎. Ask *"girth stats"* to see which installed skills actually earn their place, or *"girth review captions"* to remove sections you don't want. Removals become signal too.

**Sharing is opt-in and anonymous.** Girth asks once. If you allow it, only **content hashes of public skill sections plus counts** (used / kept / removed / up / down) would be shared, never names, paths, text, notes or anything from your project. Nothing is uploaded until the Girth index launches. Run `girth share preview` to see exactly what would be sent.

## Taste memory

For the things you make again and again (launch videos, decks, landing pages), Girth remembers two things per domain:

- **House rules** you stated once: *"always end on the logo with the URL"*. They're applied every time, in every project, and a new or extra request never drops them.
- **What recent deliverables chose**: palette, music, opening and so on. The next one varies them unless a rule or your request pins them, and Girth tells you what's fresh.

With the plugin, taste arrives on its own. When a skill linked to the domain loads, Girth's hook adds the taste to context (about 1 ms; nothing is added for unrelated skills). Say *"from now on, always…"* to add a rule, or *"girth taste launch-video"* to see what's remembered.

Personal taste lives in `~/.girth/taste.jsonl` and follows you across projects. Project rules (`--project`) live in `.girth/taste.jsonl` and can be committed for a team; trust them as you trust that repo's `CLAUDE.md`. Taste is local only: `girth share` never reads it.

## CLI

```bash
cargo build --release
target/release/girth measure                      # what your skills cost in context
target/release/girth search "caption timing" --json
target/release/girth validate .claude/skills/girth-captions   # also records to the ledger
target/release/girth drift .claude/skills/girth-captions      # source sections changed since then? exit 1 if so
target/release/girth stats                                      # what the ledger says
target/release/girth share preview                              # exactly what would be shared
target/release/girth taste add launch-video "End on the logo with the URL."
target/release/girth made launch-video palette=paper-ink music=lo-fi
target/release/girth taste show launch-video                    # rules + recent choices
target/release/girth fetch obra/superpowers/skills              # shallow, untrusted, never executed
target/release/girth lens --project . < listing.txt             # what Lens would send for a skill listing
target/release/girth stats --meter --project .                  # token usage and Lens savings
```

Skill roots resolve in this order:
1. `--roots`
2. `GIRTH_SKILL_ROOTS` (colon-separated, POSIX style)
3. `.girth/config.json` (`{"roots": [...], "output": "..."}`)
4. The defaults: `~/.claude/skills`, `~/.agents/skills`, `.claude/skills`, and the `skills` folders of installed Claude Code plugins, plus every other harness's skill folders that exist (79 harnesses from the [`skills` CLI's table](https://github.com/vercel-labs/skills/blob/main/docs/agents.md), resynced with `sh scripts/sync-harnesses.sh`). A folder reached through a symlink is searched once.

Crafted skills go to `--output`, then `GIRTH_OUTPUT_DIR`, then config `output`, then `.claude/skills` under Claude Code (`CLAUDECODE=1`) or `.agents/skills` elsewhere.

## Development

```bash
make check    # fmt, clippy, tests (unit, property, CLI, launcher), coverage ≥ 90%, plugin validate, mod tests
make mutate   # cargo-mutants
make evals    # claude plugin eval on a staged dist copy (spends model credits, capped)
cargo deny check   # advisories, licenses, sources
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [CHANGELOG](CHANGELOG.md). Report security issues privately ([SECURITY.md](SECURITY.md)).

Girth is built with Girth: see `examples/dogfood/RESULTS.md`.

## License

MIT
