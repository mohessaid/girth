# Mutation testing notes

`cargo mutants` on 2026-10-14:
- v0.1 core: 253 mutants, 6 unviable, 243 of 245 viable caught.
- v0.2 ledger modules (`sections`, `ledger`, `record`, `stats`, `share`): 144 mutants, 14 unviable, **130 of 130 viable caught**.
- v0.3 taste memory (`taste`, `main`, `config`, `skill`, `ledger`): 208 mutants, 12 unviable, 190 of 196 caught on the first run. Three survivors exposed real gaps: duplicate rule adds, the hook's per-domain context budget, and the feedback 👍/👎 label. Tests were added, and a targeted re-run caught all of them. **193 of 196 caught (98.5%).**

- `fetch` (M2): 25 mutants, 1 unviable, **24 of 24 caught**.
- `show` (M2): 20 mutants, 19 caught at first. The survivor ("SKILL.md first" ordering) got a test with a file that sorts before `SKILL.md`, and it is now killed: **20 of 20**.

Equivalent or untestable survivors:
- `src/skill.rs` `discover_skills`: `!real.is_dir() || seen_paths.contains(&real)` → `&&`. A non-directory never yields a skill (`load_skill` needs `<dir>/SKILL.md`), and a duplicate real path is still dropped by the name-dedupe set. The observable behaviour is identical.
- `src/main.rs` `emit`: `kind() != BrokenPipe` → `==`. This only changes whether a stderr message is printed when writing to stdout fails for a reason *other than* a broken pipe (e.g. a full disk). That isn't reproducible in a portable test.
- `src/main.rs` `record_use`: the match guard `!stdin().is_terminal()` → `true`. The guard only matters when stdin is a real TTY (a person typing `girth record use` with no `--skill`); tests always pipe stdin.

v0.5 (2026-10-15):
- **The mod (`hooks/girth.js`, mutated by hand: cargo-mutants doesn't cover JS).** Four mutants:
  - Dropping the memo: **caught**.
  - Recording Lens bytes on subagent turns: **caught**.
  - Dropping the exit-code check: survived at first, then **caught** after a test was added in which the binary fails but prints valid JSON.
  - Dropping the `.catch` handlers: **equivalent.** Claude Code already skips a hook that throws (before or after `next`) and carries on unchanged, so `.catch(… next(e))` restates the engine's default. It stays, so the fail-open intent is visible and `claude plugin validate` reports gating hooks as handled.
- **Rust (`lens`, `meter`, `price`, `config`):** 185 mutants, 3 unviable, **175 of 182 caught (96%)** on the first run. Five survivors were real gaps, each now covered by a test:
  - a blank line after a listing entry;
  - the 30-day and 14-day window edges;
  - exactly two collapsed entries;
  - the "unpriced" note when there are none.

  A targeted re-run catches all five.
- **Equivalent (2 mutants):** `lens::hint`'s word-cut guard (`i > 0` → `true`, and `>` → `>=`). The description is trimmed before it is cut, so a space can never sit at position 0. The guard can't change the result.

## v0.6 (MCP Lens, X-ray)
- **Rust (`mcp_lens`, `xray`, `lens`, `meter`):** 193 mutants, 3 unviable, **183 of 190 caught (96%)**. Five survivors were real gaps, each now covered by a test that fails under the hand-applied mutant:
  - an empty server id (`mcp____tool`) is no server;
  - `described` is reported when nothing collapses (instructions and tools);
  - the 24-hour baseline window: its length and its exact edge.
- **Equivalent (2 mutants):** the `lens::hint` guard noted above.

## v0.7 (cache keeper and doctor), the mod
- **Doctor (hand-mutation pass, 9 mutants):** 8 caught. Equivalent: dropping `breaks.delete(e.turnId)`. Turn ids are unique, so only memory is kept.
- **Keeper (9 mutants):** 7 caught.
  - Fixed: the `isAnswered` guard was a real bug. An aborted fork still spends tokens, so a ping is now recorded whenever usage comes back.
  - Not observable in `claude plugin test`: `turn.start` → `keep.stop()`. A timer's `cancel()` runs client-side and emits no event. It is covered by the live run (`examples/cache/RESULTS.md`): a turn inside the interval cancels the pending ping.
- **Rust (`keeper`, `price`, `ledger`):** 107 mutants, 6 unviable, **100 of 101 caught (99%)**. The survivor (another project's turn passing the project filter) is now covered by a test.

## v0.8 (idle compaction, subagent model)
- **Rust (lines changed on the branch, `--in-diff`):** 58 mutants, 3 unviable, **50 of 55 caught**.
  - The five survivors were all on one line: `stats --json`'s condition for showing `keeper`.
  - `stats_json_shows_keeper_only_when_there_is_something_to_show` now covers it. It fails under the hand-applied `>=` mutant.
- **Mod (hand-mutation pass, 17 mutants):** 12 caught. The five survivors:
  - **`squeeze?.stop()` in `turn.start`:** not observable in `claude plugin test`, because a timer's `cancel()` runs client-side.
    - Covered live, on the dist build with a 20 s interval: turns at 881 s and 895 s; turn 1's timer would have fired at 901 s, and nothing compacted; one compaction at 917 s.
  - **The `agentId` guard on `takeNote()`:** needs a fired timer inside the wiring, which the harness can't do.
    - It matters because a subagent spawned in the first answer after a return completes before the main turn, and would otherwise take the note.
  - **Equivalent:**
    - `timer = null` after the timer fired: a later `stop()` cancels an already-fired timer harmlessly.
    - `model !== ''`: `girth settings` never prints an empty model.
    - `start(plan, context)` → `0`: `context` is only a fallback when the engine reports no `tokensBefore`, and Claude Code 2.1.300 always reports it.

## v0.9 (cost per outcome)
- **Rust (lines changed on the branch, `--in-diff`):** 157 mutants, 2 unviable, **143 of 155 caught**. All 12 survivors were edge cases in the `outcome` classifier:
  - a heredoc word with `_`;
  - the assignment rule, which now accepts any shell identifier, so `foo=1 cargo test` counts;
  - the 4,000-character tail's exact edge;
  - `make: ***` without `Error`;
  - `Ran 1 test across`;
  - `0 passed`;
  - `FAIL_FAST` not being `FAIL`.
  - Tests now pin each one. A rerun on `src/outcome.rs`: **111 of 111 caught** (2 unviable).
- **Mod (hand-mutation pass, 10 mutants):** 8 caught, and the cap's off-by-one now has a test (`at most 64 Bash calls a turn go to the meter`). One survivor:
  - **Keying the calls by `e.agentId`:** a test's own `$.tool.call` carries no `agentId`, so a subagent's call can't be faked.
    - Covered live, on the dist build: a general-purpose subagent ran `cargo test 2>&1 | tail -3`. Its turn recorded `["test_pass"]`, and the main turn recorded none.
- **After the branch review's fixes** (filter pipes, key-value summaries, quote-aware splitting), `--in-diff` again: 187 mutants, 2 unviable, **182 of 185 caught**.
  - The three survivors were the splitter's backslash escape and two edges of the filter rule (`tee` is not a filter; a grep with no pipe before it is not one).
  - Tests now pin all three. Each fails under its hand-applied mutant.
