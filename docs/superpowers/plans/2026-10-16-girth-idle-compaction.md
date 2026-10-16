# Girth v0.8 Idle compaction and subagent model: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** opt-in idle compaction of large conversations, and an opt-in model for built-in subagents.
**Spec:** `docs/superpowers/specs/2026-10-16-girth-idle-compaction-design.md`

## Global Constraints
- **Both features are off without config:** `compact.idle` defaults to false, and `agents.model` has no default. With no config, the keeper behaves exactly as in v0.7.
- **Compaction:**
  - `compact.min_context` defaults to 150,000, and the project config wins for it.
  - `"idle": false` in either config file wins over `true` in the other.
  - `GIRTH_COMPACT=off` turns it off.
  - Subagent turns never compact.
- **Every mod hook fails open.** Tests never touch the real `~/.girth`.
- **`share` never reads `compact` events.**
- **Version:** 0.8.0.

## Review Focus
1. **An earlier turn's compaction timer is cancelled** by a new main turn.
2. **No config means no `compact` key in the plan:** the v0.7 JSON stays byte-identical.
3. **A refused compaction** records nothing and leaves no note.
4. **`agent.spawn` never overrides** an explicit model, a plugin's agent or a fork.
5. **An old ledger without `compact` events** parses unchanged.

---

### Task 1: keeper settings and plan
- **`Settings`** gains `compact_idle` (false) and `compact_min_context` (150,000).
- **`settings()`** reads the `compact` key.
- **`plan(settings, keeper_off, compact_off, interval, context)`** returns `Plan { pings, interval_ms, compact }`, where `compact` is skipped from the JSON when false. A compact plan has `pings: 0`.
- **Tests:**
  - the precedence rules;
  - compact replaces pings only at or above `min_context`;
  - `compact_off` leaves the pings alone;
  - the JSON has no `compact` key when false.

### Task 2: ledger `compact` event and stats
- **The event:** `Event::Compact(Compact { project, model, before, after })`.
- **Summary:** `keeper::Summary` gains `compactions`, `compact_before` and `compact_after`.
- **Stats line:** `render` adds `meter: idle compaction: N times · 583K → 21K tokens on average`.
- **Tests:** a round trip; an old line parses; a summary filtered by project; render; the share sentinel.

### Task 3: CLI
- **`girth compact record`:** appends a `compact` event from stdin. Bad input is ignored.
- **`girth meter record`:** passes `compact_off = agent || GIRTH_COMPACT=off`.
- **`girth settings --project <dir>`:** prints `{"agents":{"model":…}}`, or `{}`.
- **Stats JSON:** shows `keeper` when there are compactions.
- **Tests:** CLI tests for each.

### Task 4: Mod (compactor)
- **The exported `compactor(deps)`:** `start(plan, context)` arms one timer; `stop()` cancels it.
  - When it fires, it calls `deps.compact()`. On success it records the event and keeps a note `{before, after}`.
  - `takeNote()` returns the note and clears it.
- **Wiring:**
  - `turn.start` (main) stops the compactor;
  - `turn.complete` (main) appends the note's line, then arms the compactor if the plan says compact.
- **Tests:** arm, cancel, success, refusal, throw; the line on the next turn; agent turns ignored.

### Task 5: Mod (subagent model)
- **`session.start`:** runs `girth settings --project <cwd>` once.
- **`agent.spawn`:** applies the spec's rules.
- **Tests:** set, explicit model kept, plugin agent kept, fork kept, no config, settings failure.

### Task 6: Live checks, docs, gates, release
- **Live:**
  - a tmux run of the dist build with `GIRTH_KEEPER_INTERVAL_MS=20000` and `compact.idle`, checking the event and the line;
  - a `claude -p` run with `agents.model=haiku` and an Explore subagent, checking the subagent's model.
- **Docs:** README and CHANGELOG.
- **Gates:** `make check`, mutants, evals.
- **Release:** 0.8.0.
