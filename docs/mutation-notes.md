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
