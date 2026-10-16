# Girth v0.9 Cost per outcome: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** record test, commit and PR outcomes per turn, locally, and report cost per commit in Meter.
**Spec:** `docs/superpowers/specs/2026-10-16-girth-cost-per-outcome-design.md`

## Global Constraints
- **No command text or output in the ledger.** Only the outcome labels are stored.
- **Test verdicts come from the runner's summary,** never from the exit status alone.
- **Every mod hook fails open,** and a Bash result is returned unchanged.
- **At most 64 calls per turn,** and a 4,000-character output tail.
- **Tests never touch the real `~/.girth`.**
- **Version:** 0.9.0.

## Review Focus
1. **Bash result pass-through:** the result is unchanged, errors included.
2. **No command text leaks** into the ledger.
3. **A piped failing run** is judged a failure.
4. **A subagent's calls** land on the subagent's turn.
5. **An old ledger** parses unchanged.

---

### Task 1: `src/outcome.rs`
- **Code:** `Outcome`, `classify`, `verdict`, `outcomes`.
- **Tests:** unit tables (runners, wrappers, heredoc, `git -C`, dry run, `cd`) and summary formats (N = 0 vs N ≥ 1); property tests (no panic, piped failure).

### Task 2: ledger and meter summary
- **Ledger:** `Turn.outcomes` (skipped when empty).
- **Summary:** `meter::Summary` gains `commits`, `prs`, `tests_passed`, `tests_failed`, `tests_untold`, `since_commit`.
- **Render:** the outcomes line, which leaves out empty parts.
- **Tests:** a round trip, an old line, the summary, render, `since_commit`, and the share sentinel.

### Task 3: CLI
- **`meter record`:** reads `bash`, caps it at 64 calls, sets `outcomes`.
- **Tests:** CLI tests for stored outcomes, no command text in the ledger, the cap, and `stats --meter` showing the line.

### Task 4: Mod
- **`tool.call` (Bash):** records each call per loop key.
- **`turn.complete`:** attaches the list for its loop.
- **Tests:** recorded and attached, result unchanged, agent separation, non-Bash ignored.

### Task 5: Live check, docs, gates, release
- **Live:** a `claude -p` run in a scratch repo with an isolated `GIRTH_HOME`.
- **Docs:** README, CHANGELOG, `hooks.json` description, mutation-notes, eval-log.
- **Gates:** `make check`, in-diff mutants, evals.
- **Release:** 0.9.0.
