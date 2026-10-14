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
