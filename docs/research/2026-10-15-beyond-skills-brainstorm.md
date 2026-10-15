# Beyond skills: brainstorm (2026-10-15)

## The reframe
Girth started as "just-in-time skills". With v0.5 it has three things no skills tool has:
- **Lens:** a lever on what Claude Code sends.
- **Meter:** real cost per turn, measured locally.
- **A harness** (`examples/lens/ab.sh`) that proves a lever on the user's real setup.

That makes Girth the **private cost-and-context layer of the harness**. Skills were the first lever; they aren't the only one.

**What a real request carries** (probe, the author's machine, Claude Code 2.1.300, text that Claude Code added, excluding tool schemas):

| Part | Bytes | Share | Girth lever |
|---|---|---|---|
| Skill listing | 30,027 | 44% | **Lens (v0.5)** |
| MCP server instructions | 14,258 | 21% | none yet |
| Deferred-tools list | 7,234 | 10.5% | none yet |
| Hook additional context | 4,648 | 7% | none yet |
| Memory, advisor, agents, env… | ~12,500 | 18% | — |

So 31% of the added text is MCP and tool listings, many of them for connectors a given project never touches.

## Directions, by layer of the stack

### Harness: what goes into the prompt
1. **Lens for tools and MCP.**
   - `prompt.attachment` can rewrite `mcp_instructions_delta` and `deferred_tools_delta`, and `tool.describe` can defer a tool or rewrite its description.
   - The evidence is the same as for skills: `tool.call` gives per-project usage, so servers never used here shrink to a hint.
   - The A/B harness is ready. **Prize:** up to about 31% more of the added text.
2. **Context X-ray (`/girth-xray`).** Show, per request, what the prompt is made of (the table above, live, with a cost per part). Nobody shows users this today. It is also the best demo Girth could have.
3. **Cache doctor.** A cache write on turn N > 1 means the prefix changed, and everything after it is re-billed. A `turn.step` observer can name the turn and the size ("turn 7 re-wrote 80K tokens"). This is diagnosis only, with no behaviour change.
4. **Memory distillation.** `CLAUDE.md` and nested memory are always on and only grow. Apply the craft discipline (verbatim rules, drop the rest, keep the source) to memory: suggest splits into path-scoped rules, and flag rules that never fire.

### Models and effort
5. **Cheap subagents.** `agent.spawn` can set a subagent's model, and subagents start fresh, so moving them to Haiku 5.5 (about 40× cheaper than Opus 5.5) costs no cache. Meter already tags subagent turns. **Step one is measurement:** what share of real spend is subagents?
6. **Effort per step.** `turn.step` can change effort. Whether that invalidates Claude Code's cache is unknown, so it starts as a spike: one observe-only run logging cache writes per request. Never switch *models* mid-turn: caches are per model, and thinking blocks are tied to the model.

### The loop
7. **Loop breaker.** Detect a runaway: the same failing command 3×, or edit-undo churn. Say so once, with the cost so far. A stuck agent is the most expensive agent.
8. **Cost per outcome.** Tie Meter's turns to outcomes the repo already has: a passing test run, a commit, a merged PR. Report "$ per green test run" and "$ per commit" per project. The industry is moving from cost per token to cost per task, and Girth can do it locally and privately.

### Beyond one harness
9. **One Girth, every agent.** Export crafted skills, house rules and taste into `AGENTS.md` and other harnesses' formats. The user's taste and distilled know-how follow them from Claude Code to Codex, Gemini CLI and others.
10. **Deferred work at half price.** Queue non-urgent agent jobs (evals, backfills, overnight refactors) to run as scheduled or batch work, where batch pricing is 50% off.

### Community (hosted, pending the author's decision)
11. **The Girth index, widened.** It ranks skills, **MCP servers and plugins** by what people keep versus collapse, and by measured lift. It uses opt-in hashes and percentages only. This replaces stars with evidence for the whole extension ecosystem, not just skills.
12. **Local Radar.** Lens already records names; it can record description hashes too. Then a session-start line becomes "3 skills and 1 MCP server changed since your last session". That is the author's original "what's new" ask, local-first and without a crawler.

## Recommendation
- **v0.6 = Lens for tools and MCP + Context X-ray.**
  - Same mechanism, same evidence, same A/B harness, and the prize is already measured: 31% of the added text.
  - X-ray makes every lever visible to the user, and it makes the demo.
- **Alongside, at no cost:** let Meter fill on the author's real sessions for a few days. That decides between (5) subagent routing and (8) cost per outcome on data, not guesses.
