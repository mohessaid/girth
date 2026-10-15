# Girth v0.5: Lens, Meter, and a SkillsBench Proof

**Date:** 2026-10-15
**Status:** Approved in conversation ("go both"), with review at the author's convenience, as for v0.3 and v0.4.
**Research:** `docs/research/2026-10-15-landscape-and-directions.md` (sections 3, 5 and 6).
**Out of scope:** the hosted index, model routing, Haiku distillation, and Shield (all later).

## 1. Problem
- **Measured on the author's machine:** Claude Code sends a `skill_listing` of **74 skills, 29.3 KB (about 7.3K tokens), on every request**. That is about 21% of a minimal prompt. The tail of the list is already cut to bare names because it is over budget.
- **What the research says:** focused context wins. SkillsBench: 2–3 skills per task gain +20 pp, while 4 or more gain +5.2 pp.
- **The current tooling falls short:**
  - `/skill-doctor` and `skillOverrides` are per skill and global. They don't know a skill's age, and they skip plugin skills.
  - Girth today can only add skills (crafted ones). It can't make the rest quieter.
- **Girth's numbers are one-off benchmarks.** Nothing measures real sessions.

## 2. Goals
1. **Lens:** per project, the model reads full descriptions only for the skills that project needs. Every other skill stays invocable by name. Nothing is uninstalled, and no settings are edited.
2. **Meter:** every turn's real token usage lands in the local ledger, tagged with what Lens did. `girth stats` reports totals and Lens's measured listing savings.
3. **Proof:** run a capped SkillsBench subset (no skills / curated skills / Girth-distilled) and publish the result whatever it is.

Success criteria are in §9.

## 3. Mechanism: a mod inside the existing plugin
- **Why a mod:** Claude Code mods (v2.1.287+) can rewrite the `skill_listing` attachment (`prompt.attachment`, `type: "skill_listing"`) and observe `turn.complete` usage (verified by the probe in research §6).
- **The mod is thin.** `hooks/hooks.json` gains `"modules": ["./girth.js"]` alongside its existing `PostToolUse` settings hook. Every decision comes from the Rust binary through the launcher (`$.process.run([launcher, …], { stdin })`).
- **Failure policy: fail open.**
  - If the binary is missing, fails, is slow or prints bad output, the listing passes through unchanged and nothing is recorded.
  - Every hook has a `.catch` that calls `next(e)`.
- **Older Claude Code** (< 2.1.287) ignores `modules`. The settings hook keeps working, so v0.4 behaviour is unchanged.
- **Organization-managed machines** skip `prompt.attachment` hooks for user-installed plugins. Lens is inert there; Meter still works. Documented.
- **Skills-CLI installs** get neither Lens nor Meter. The README table says so.

## 4. Lens
### 4.1 `girth lens` (Rust)
`girth lens [--project DIR] [--json]` reads the listing text on stdin and prints the rewritten listing (or JSON).

- **Parse.**
  - The first line is the header.
  - Each entry starts with `- ` at column 0: `- name: description` (the description may continue on following lines until the next entry) or `- name` (name only).
  - Unknown lines are kept, attached to the preceding entry.
  - Unparseable input (no entries) passes through unchanged.
- **Keep described** (in the listing's original order) when any of these holds:
  1. The name is Girth's own (`girth`, `*:girth`), or its base name starts with `girth-` (crafted skills).
  2. A project skill exists: `<project>/.claude/skills/<base>/SKILL.md`.
  3. The ledger has a `use` of this name in this project.
  4. The ledger has a `use` of this name in any project within the last 30 days.
  5. The name is **new**: first recorded by Lens fewer than 14 days ago **and** not part of the baseline (the names present at Lens's first run on this machine).
  6. The name is pinned. `lens.keep` (a list of names) in the project's `.girth/config.json` or in `GIRTH_HOME/config.json` (new, optional).
- **Collapse** every other entry into one final line, keeping every name:

  `- Also installed (names only; ask \`girth search "<need>"\` which fits): a, b, c`

  A collapsed skill stays invocable by name, and Girth's search finds it by full text.
- **Never loses a name.** The set of names in the output equals the set in the input (property test).
- **Off switches:**
  - `GIRTH_LENS=off`, or `"lens": {"enabled": false}` in either config, passes the listing through unchanged.
  - If fewer than 2 entries would be collapsed, the listing passes through unchanged (nothing to gain).
- **Seen:** names not yet recorded are appended to the ledger as one `seen` event `{names: [...]}`. The first `seen` event on a machine is the baseline.
- **JSON output:** `{text, described, collapsed, bytes_before, bytes_after}`.

### 4.2 Cache stability
- The mod memoises `lens` output by input text for the life of the session. The same listing therefore always gets the same rewrite, which keeps the prompt cache warm.
- Ledger changes mid-session apply from the next session.

### 4.3 Skill text
`skills/girth/SKILL.md` gains one line: when a needed skill appears only under "Also installed", run `girth search` and invoke the named skill, or craft from it. Always-on cost: the description is unchanged.

## 5. Meter
### 5.1 Recording
- **Source:** on `turn.complete` with `e.usage`, the mod runs `girth meter record` with stdin JSON:

  `{project, model, input, output, cache_read, cache_write, agent: bool, aborted: bool, lens: {before, after} | null}`

  `lens` carries the byte counts from the session's listing rewrite, if any.
- **Ledger event:** `turn` holds these fields as numbers and the model id. The project path is kept, as `use` events already keep it. No text, prompt or answer is recorded.
- **Sharing:** `share` ignores `turn` and `seen` events (exhaustive match plus a test).

### 5.2 Reporting
- **`girth stats`** adds a meter section, computed from `turn` events:
  - turns, tokens by kind, and an estimated cost (`src/price.rs`, per-model $/Mtok for input, output, cache read and cache write; an unknown model shows tokens only);
  - **Lens savings:** `(before − after) bytes ÷ 4` tokens per request × requests, priced at the cache-read rate. It is labelled an estimate; the request count is the turns recorded.
  - With `--json`, a `meter` object.
- **Command:** the mod registers a `/girth-meter` command that prints the current project's meter lines (from `girth stats --project . --meter`).

## 6. SkillsBench proof (`examples/skillsbench/`)
- **Harness:** Harbor (`uv tool install harbor`) with its `claude-code` agent on Docker, on SkillsBench `main`.
- **Tasks:** 6 that have curated skills and light environments. The final picks are recorded in RESULTS.md with the reason for each.
- **Arms:**
  - **none:** the task's skills removed from its environment;
  - **curated:** the task as shipped;
  - **girth:** the task's skills replaced by one `girth-<domain>` skill, crafted on the host.
- **Crafting the girth arm:** a headless `claude -p` session with the Girth plugin. It uses the task's `instruction.md` as the need and the task's `environment/skills` as the only roots. The craft cost is reported separately.
- **Model and budget:** `claude-sonnet-5-5`, 1 run per arm per task (18 runs), plus 6 crafts. A hard cap of **$10** for the whole proof; stop and report what was finished.
- **Report:** pass rate per arm, tokens and cost per arm, and the craft cost. **Whatever the result**, it goes in RESULTS.md and the README. If the girth arm is worse than curated, the README says so.

## 7. Error handling summary
| Failure | Behaviour |
|---|---|
| Binary missing, slow (> 5 s) or failing during `lens` | The listing passes through unchanged; the mod logs once with `$.ui.log` |
| Malformed listing | Passes through unchanged |
| Ledger unreadable | Treated as empty: no uses, so only rules 1, 2, 5 and 6 keep descriptions |
| `meter record` failure | Ignored; the turn is unaffected |
| Org-managed machine | Lens inert, Meter on |

## 8. Testing
- **Rust unit tests:**
  - parsing (wrapped descriptions, name-only lines, unknown lines, header);
  - each keep rule, the baseline-versus-new rule, the 30-day window, pins in both configs, the off switches and the "fewer than 2 collapsed" passthrough;
  - JSON;
  - `seen` dedupe;
  - `turn` parsing;
  - the price table and the savings maths.
- **Property tests:**
  - the output name set equals the input name set;
  - described entries are byte-identical to the input;
  - a rewrite is never larger than its input plus 200 bytes;
  - parsing never panics.
- **CLI tests:**
  - `lens` from stdin (text and JSON);
  - `GIRTH_LENS=off`;
  - `meter record` appends a `turn` event;
  - the `stats` meter section;
  - `share preview` shows nothing from `turn` or `seen` (a sentinel project path).
- **Mod tests (`claude plugin test`, added to `make check`):**
  - the listing is rewritten via a stubbed `process.run`;
  - the listing passes through on a process error and on a timeout;
  - output is memoised;
  - `turn.complete` sends the right stdin;
  - `/girth-meter` returns the text.
- **`claude plugin validate`:** clean, and the gating hooks have a `.catch`.
- **Evals:** `lens-hidden-skill`. The scaffold installs a specialised project-unrelated skill under user-level roots, with no ledger use, so Lens collapses it. A task needs it. Pass if `Skill` is called with that skill or with `girth`.
  - If `claude plugin eval` doesn't load mods, this runs as a headless A/B script instead (`examples/lens/`) and the reason is recorded.
- **Live A/B (`examples/lens/`):**
  - one real task in this repo, with the author's real configuration and `GIRTH_HOME` isolated;
  - with and without the mod;
  - compare `input + cache_write` tokens per request and the task check.
- **Mutation testing:** at least 80% caught on `lens.rs`, `meter.rs` and `price.rs`.

## 9. Success criteria
1. On the author's 74-skill machine, Lens cuts the listing by ≥ 70% of bytes in a project with typical usage, with every name preserved.
2. `lens-hidden-skill` passes 3 of 3, and the existing 7 evals stay at 1.00.
3. The live A/B shows lower prompt tokens with the task check still passing.
4. Meter records every main-loop turn of a live session. `girth stats` shows totals and the Lens estimate.
5. The SkillsBench proof runs within $10 and is published as it came out.
6. `make check` is green, coverage ≥ 90%, mutants ≥ 80%, and v0.5.0 is released with the full ritual.
