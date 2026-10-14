# Mutation testing notes

`cargo mutants` on 2026-10-14:
- v0.1 core: 253 mutants, 6 unviable, 243 of 245 viable caught.
- v0.2 ledger modules (`sections`, `ledger`, `record`, `stats`, `share`): 144 mutants, 14 unviable, **130 of 130 viable caught**.
- **Overall: 373 of 375 viable caught (99.5%).**

The two survivors are equivalent or untestable mutants:
- `src/skill.rs` `discover_skills`: `!real.is_dir() || seen_paths.contains(&real)` → `&&`. A non-directory never yields a skill (`load_skill` needs `<dir>/SKILL.md`), and a duplicate real path is still dropped by the name-dedupe set. The observable behaviour is identical.
- `src/main.rs` `emit`: `kind() != BrokenPipe` → `==`. This only changes whether a stderr message is printed when writing to stdout fails for a reason *other than* a broken pipe (e.g. a full disk). That isn't reproducible in a portable test.
