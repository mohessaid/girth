# Girth v1 — Just-in-Time Project Skills

**Date:** 2026-10-14
**Status:** Draft for review
**Layer:** Harness (first layer of the Girth stack)

## 1. Problem

Published agent skills bundle a whole domain into one install (e.g. 13 `hyperframes-*` skills, several overlapping video pipelines). A real project needs a small slice from many domains: engineering, marketing, content, video. Users therefore install dozens of large skills, use a fraction of each, and pay for the rest:

- **Always-on cost:** every installed skill's description sits in context on every turn. In the author's setup, 52 skills mean ~29 KB (~7–8K tokens) of descriptions per turn, and near-duplicate descriptions confuse routing.
- **Invocation cost:** `SKILL.md` bodies average ~13 KB; some skills pull references up to ~870 KB.
- **Discovery tools** (find-skills, skills.sh) help locate skills but still install them whole.

## 2. Goal

The harness grows its **own lean, project-specific skills on demand**, at the moment it falls short, by distilling only the needed slice from existing skills. This works the way a model reaches for web search when it lacks a fact.

The mental model is a **cache**: crafted project skills are the cache, the skill ecosystem (local plus remote) is the backing store, and a capability gap is a miss.

**Non-negotiable:** output quality must not drop. Compression applies to the *skill*, never to the *style of the work the skill produces*.

## 2a. Competitive landscape

- **Skillselion MCP** (MIT, free, ~86k-item catalog) is the closest prior art. `load_skill` copies the best-matching whole skill into a temp folder (`$TMPDIR/skillselion/...`) for the current session. `synthesize_skills` merges the rules of the top 2–8 skills into a provenance-tagged consensus digest. A `SessionStart` hook primes sessions.
- **find-skills / skills.sh (`npx skills find|add`)** discovers and installs whole skills.
- **Research:** "task-derived acquisition" and skill distillation are active topics (2026 agent-skills surveys, COLLEAGUE.SKILL, TRUSS on safe skill generation). No widely adopted tool persists project-scoped distillations.

**Girth's differentiation:**
1. **Persistent, project-scoped cache that compounds.** Skillselion re-pays the full load every session. Girth pays once, then pays only for a ≤3 KB slice.
2. **Local-first.** It distils from what the user already has installed before touching the network.
3. **Task-slice distillation with quality-preserving rules**, rather than a consensus merge of whole skills.
4. **Measured savings and eval-gated quality** shipped with the tool.

Skillselion can be an optional remote source alongside skills.sh.

## 3. Scope

**In v1:**
- A Claude Code skill named `girth`, plus small helper scripts.
- Two triggers: model-initiated and user-initiated (`/girth <need>`).
- Local-first sourcing, with remote search as a fallback.
- The crafted-skill format, distillation rules, growth/merge policy.
- Helpers for search, measure and contract validation (Python, stdlib only).

**Out of v1 (later):**
- **Hooks (plugin form):** a `UserPromptSubmit` hook that detects correction phrases ("that's wrong", "not like that", "too generic") and nudges the model to call Girth. A `Stop`/failure hook could do the same. This makes the trigger automatic rather than relying on self-awareness.
- **MCP server:** expose `girth_craft(need)` as a true tool for cross-harness use (Codex, Cursor, others), reusing the same scripts.
- **Usage-driven pruning:** evict or merge crafted skills based on `last_used` and observed usage across sessions.

## 4. Architecture

```
 task in progress
       │  gap detected (model self-check OR user runs /girth <need>)
       ▼
 ┌─────────────────────────┐
 │ girth SKILL.md          │  tiny always-on description (~40 words)
 │ (procedure for model)   │
 └──────────┬──────────────┘
            │ 1. check cache
            ▼
   .claude/skills/girth-*/        ── hit ──▶ extend if needed, use it
            │ miss
            │ 2. search sources (scripts/girth search)
            ▼
   ~/.claude/skills, ~/.agents/skills   (local, no network)
            │ nothing relevant
            ▼
   remote: skills.sh / find-skills search, GitHub fetch into scratch dir
            │
            │ 3. model reads candidates, extracts needed slice
            ▼
   write/extend .claude/skills/girth-<domain>/SKILL.md
            │ 4. scripts/girth validate / measure
            ▼
   continue the original task using the crafted skill
```

**Division of labour:** the running model does the distillation. The scripts only search, fetch, measure and validate. That keeps v1 to one `SKILL.md` and a few helpers, with no LLM calls of their own.

**Helper language (revised 2026-10-14): Rust.** There is one `girth` binary with subcommands `search`, `measure` and `validate`. The skill calls it through a POSIX launcher, `skills/girth/scripts/girth`, which tries in order: `$GIRTH_BIN`, a bundled `skills/girth/bin/girth`, the repo's `target/release/girth`, `girth` on `PATH`, and finally `cargo build --release`. Prebuilt per-platform binaries are an M3 release task. YAML is parsed **strictly** (a real YAML parser) when validating crafted skills, and **leniently** during discovery: if strict parsing fails, a fallback scan recovers `name` and `description`, and the number of fallbacks is reported on stderr. That way discovery never drops a real-world skill. The verified Python reference implementation in the M0–M1 plan defines the behaviour; its tests are ported one-to-one.

**Configurable roots and output:** helpers resolve skill roots in this order: `--roots`, then `GIRTH_SKILL_ROOTS` (`os.pathsep`-separated), then `roots` in `./.girth/config.json`, then the defaults `~/.claude/skills`, `~/.agents/skills`, `./.claude/skills`. The crafted-skill output dir resolves the same way (`--output`, `GIRTH_OUTPUT_DIR`, config `output`, default `./.claude/skills`) and is always searched, because it is the cache.

**Findings from the 2026-10-14 eval spike** (these drove the rules above):
- Eval child sessions receive only `EVAL_*` env vars, so the config file is how evals point Girth at fixture roots.
- Writes into `.claude/` are denied during eval runs even with `Write` granted. Evals set `output` to a non-`.claude` dir. In interactive use, that write is a natural consent prompt.
- A plugin's scripts are readable from the sandbox even when the plugin lives under `~`.
- A pyenv `python3` shim breaks inside the sandbox, so evals run with `/usr/bin` first on `PATH`.
- `claude -p --setting-sources project,local` hides user-level skills and plugins. In an empty project the author's user-level skills and plugins added **~13.3K input tokens per session** (35.8K vs 22.5K). This is the baseline for the acceptance demo.

### Components

| Unit | Purpose | Interface |
|---|---|---|
| `skills/girth/SKILL.md` | Trigger description plus the craft procedure | Loaded by the harness; invoked by the model or `/girth` |
| `girth search <query>` | Ranks candidate skills by matching the query against name, description and headings across roots; optional remote fallback via `npx skills find` | Prints ranked list (text or `--json`): path, bytes, score, matched headings |
| `girth fetch <owner/repo[/path]>` (M2) | Shallow-fetches a remote skill into a fresh untrusted scratch dir; never into a skills dir; never executes fetched content | Prints the local path |
| `girth measure [--roots …]` | Reports always-on description bytes/≈tokens and body sizes. Default roots are *search sources*; pass `--roots ~/.claude/skills` (plus project skills) to measure what the harness actually loads | Table or `--json`, plus totals |
| `girth validate <skill-dir>…` | Checks a crafted skill against the contract (section 7) | Exit 0 if valid, else lists violations |
| `.claude-plugin/plugin.json` | Makes the repo a plugin, enabling `claude plugin validate` / `eval` | It still installs via `npx skills add` |
| Crafted skills `.claude/skills/girth-<domain>/` | The cache | Standard skill format (below) |

## 5. Triggers

1. **Model-initiated.** The `girth` description tells the model to invoke it when the current task needs domain know-how it lacks or is unsure of (conventions, library APIs, craft rules), and no project skill covers it. The `SKILL.md` adds a short self-check: "Am I about to produce generic output for a specialised domain?"
2. **User-initiated.** `/girth <need>`, e.g. `/girth caption animation in hyperframes`. This covers the case the model can't detect: the user sees the output is falling short.

## 6. Source precedence

1. Project `.claude/skills/girth-*` (already crafted: cache hit)
2. Local installed skills: `~/.claude/skills`, `~/.agents/skills`, project `.claude/skills` (non-girth)
3. Remote: find-skills / skills.sh search, then GitHub fetch into scratch

Girth may *use* find-skills to locate a skill. It **never installs whole skills**. Remote content is treated as untrusted data: it is read and distilled, never executed.

## 7. Crafted-skill contract

```markdown
---
name: girth-<domain>
description: <≤30 words, project-specific: what this project does in this domain>
metadata:
  girth:
    sources:
      - path-or-repo: <path or owner/repo>
        ref: <commit or version, if known>
        sections: [<headings used>]
    crafted_for: <the task/need that triggered crafting>
    created: <YYYY-MM-DD>
    last_used: <YYYY-MM-DD>
---
<body ≤ 3 KB>
```

**Distillation rules** (these protect quality):
- Keep **verbatim**: "never/always/must" constraints, gotchas, exact API signatures, required file structures, version-specific behaviour.
- Keep only what the triggering need (and clearly adjacent needs) touches. Drop examples, motivation prose and unrelated sub-topics.
- **Never compress tone-of-output guidance into terse style rules.** If a source skill governs how output should read (copy, docs, narration), preserve those instructions in full fidelity. The skill is compressed; the work it produces is not.
- Record which source sections were used so the skill can be re-distilled or extended later.
- If the needed slice can't fit in 3 KB, put the overflow in `references/<topic>.md` inside the crafted skill. Loading it stays on demand.

## 8. Growth policy

- On a gap in a domain that already has a `girth-<domain>` skill, **extend it** rather than create a new one, and update `sources` and `crafted_for`.
- At most **one crafted skill per domain**. When the body passes 3 KB, move detail into `references/` instead of splitting.
- If crafted skills in a project exceed **8**, Girth proposes merges to the user before creating another.
- Every use updates `last_used`.

## 9. Error handling

- **No relevant source found:** say so and continue the task unaided. Never fabricate a skill from the model's own guesses and label it as distilled.
- **Fetch fails or no network:** use local sources only and report it.
- **Conflicting sources:** prefer the most specific and most recent source, and note the conflict in the crafted skill.
- **User declines** (in user-visible flows): Girth writes nothing.

## 10. Success criteria

1. **Token cost:** for a demo project, `girth measure` shows always-on description cost after Girth (girth plus crafted skills only, with bulk installs removed) is **≥10× lower** than with the bulk installs.
2. **Quality demo:** a real captioning or animation task, done with the author's 13-skill hyperframes stack versus one crafted `girth-hyperframes` skill, produces output judged **no worse** by the author. This is a side-by-side and becomes the launch artifact.
3. **Incrementality:** a second, different need in the same domain extends the existing crafted skill rather than creating a new one.
4. **Install:** a single `npx skills add <repo>` (or copy into `.claude/skills`) makes Girth usable.
5. **Compounding (the metric only Girth can hit):** total tokens spent on skill knowledge across **5 sessions** on the same domain. Girth must beat both bulk-install and per-session loading (Skillselion-style), because sessions 2–5 hit the cache.

## 11. Repository layout

```
girth/
  .claude-plugin/plugin.json
  skills/girth/SKILL.md
  skills/girth/scripts/girth   # POSIX launcher for the girth binary
  Cargo.toml  src/*.rs        # girth CLI: frontmatter, config, skill, measure, search, validate
  tests/{cli,launcher,skill_md}.rs   # integration tests (unit + proptest live in src/)
  evals/                    # claude plugin eval suite
  examples/                 # before/after demo artefacts
  docs/superpowers/{specs,plans}/
  CLAUDE.md  README.md
```

## 12. Test strategy (the whole spectrum)

Development is test-driven: a failing test comes before every behaviour.

| Level | Tool | What it covers | Gate |
|---|---|---|---|
| Unit | `cargo test` (module tests, assert_cmd CLI and launcher tests) | Each helper's functions: frontmatter parsing, scoring, sizing, contract checks | All pass |
| Property | proptest | Search ranking invariants (exact name match outranks partial; ranking is deterministic); measure is additive across roots; the validator never crashes on arbitrary frontmatter | All pass |
| Coverage | cargo-llvm-cov | Line coverage of `src/` | ≥ 90% |
| Mutation | cargo-mutants | Strength of the unit and property suites against `src/` | ≥ 80% killed; survivors reviewed |
| Static | rustfmt, clippy `-D warnings`, `claude plugin validate` | Lint plus plugin/skill schema | Clean |
| Behavioural | `claude plugin eval` | Skill trigger and output behaviour in isolated headless sessions, with/without baseline (Δ) | Threshold per case; pinned `--model`; `--max-cost-usd` |
| E2E / acceptance | `claude -p` in fixture projects | Hyperframes side-by-side (13 skills vs one crafted); 5-session compounding measurement | Author judgment plus measured numbers |

**Constraints from the eval harness** (these shape the interfaces):
- Runs are isolated, and with Bash granted, `~` and the Claude config are unreadable. Cases therefore seed fixture skill roots with `context.scaffold_script` and point `GIRTH_SKILL_ROOTS` at them.
- There are no custom-code graders. Contract conformance is checked by having the prompt run `girth validate` and grading its `VALID <dir>` output in the trace, then grading it with `regex` and `file_exists`.
- Remote fetch needs `WebFetch(domain:…)` grants. Behavioural cases stay local-only; remote is covered by unit tests with a stubbed network.

**Planned eval cases:**
1. Model-initiated trigger on a specialised task with a matching local source: `tool_used: Skill`, crafted file exists.
2. **Must not fire** on a generic task: `tool_used` Skill with `min: 0, max: 0`, `arm: both`.
3. `/girth <need>` produces a contract-valid skill: validator output passes, size ≤ 3 KB.
4. A second need in the same domain extends rather than duplicates: exactly one `girth-<domain>` dir.
5. Tone fidelity: crafting from a copywriting source keeps tone guidance verbatim (regex on the preserved lines).
6. No source found: no skill written, and the reply says so.

## 13. Milestones and goals

| Milestone | Goal | Exit criteria |
|---|---|---|
| **M0 Skeleton** | Project set up the TDD way | Plugin layout, Cargo crate, failing tests and scaffolded eval cases; `make check` runs fmt, clippy, tests, coverage and plugin validate; the red suite runs end to end |
| **M1 Local-first `/girth`** *(traction artefact)* | User-triggered crafting from local skills | `search`, `measure` and `validate` implemented to the gates in section 12; eval cases 3, 5 and 6 pass; **hyperframes 13→1 side-by-side** judged no worse; measured description-cost drop recorded in `examples/` |
| **M2 Self-trigger and growth** | The model calls Girth on its own; the cache compounds | Description tuned until eval Δ > 0; cases 1, 2 and 4 pass; `girth fetch` remote fallback; 5-session compounding numbers recorded |
| **M3 Launch** | Traction | README with before/after numbers and demo; published to skills.sh; announcement post |
| Later | Next layers | Hooks (correction-phrase trigger), MCP server, usage-based pruning (including **stale plugin-cache versions**: the author's `~/.claude/plugins/cache` keeps several old versions of the same plugin skills), **Radar** (below) |

## 13a. Dogfooding (bootstrapping)

Girth is built with Girth, the way a self-hosting compiler is built with itself.
- **Stage 0:** the core (`search`, `measure`, `validate`) is written without Girth.
- **Stage 1:** right after that, the stage-0 binary crafts this repo's own project skills, e.g. `girth-skill-authoring` from the superpowers `writing-skills` and Anthropic `skill-creator` skills. Girth's own `SKILL.md` is then written following that crafted skill.
- Domains with no local source are recorded as "no source found", which exercises spec case 6.
- Results (sizes, sources, what was reused) are recorded in `examples/dogfood/`.

## 14. Roadmap: Radar layer (separate spec)

This is a separate Girth layer and stays out of v1. It is a scheduled "what's new" digest covering the Claude Code / claude.dev changelog and docs, Addy Osmani, field newsletters, and big-tech engineering blogs. It produces a short, deduplicated brief of changes relevant to the user's stack, and can propose updates to crafted skills when a source changes. It is a natural fit for `/schedule` routines. It gets its own brainstorm and spec after M3.
