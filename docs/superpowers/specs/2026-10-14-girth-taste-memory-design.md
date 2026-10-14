# Girth v0.3: Taste and Variation Memory

**Date:** 2026-10-14
**Status:** Approved in conversation ("continue working on the project", "go none stop"), with a review on GitHub at the author's convenience
**Builds on:** `2026-10-14-girth-local-ledger-design.md`
**Out of scope:** the hosted index and community signals (sub-projects 3a/3b, pending the author's decision).

## 1. Problem
The author's words: *"in all my projects I ask for a launch video in motion graphic but I always want different things to be added… skills installed but now they feel generic… getting boring and when I ask for something else I don't know what gets to the model but looses everything."*

There are three pains:
1. **Lost house rules.** Lasting preferences ("always end on the logo") live only in chat. A changed or extra request drops them.
2. **Sameness.** Nothing remembers what earlier deliverables looked like, so every launch video converges on the skill's defaults.
3. **Generic skills.** An installed skill knows the domain, but not this user.

## 2. Prior art (learn and search)
- **HyperFrames / media-use memory** (`media-use/references/memory.md`):
  - *Preferences*: confirmed answers only, two tiers (project `.media/preferences.json` and personal `~/.media/preferences.json`, the latter promoted after 2 projects).
  - *Recipes*: frozen approved bundles.
  - Both push toward **reuse**: the next video is recommended to look like the last approved one.
  - Nothing records *variation*, and the store is HyperFrames-only.
- **Claude Code memory** (CLAUDE.md, auto memory): general-purpose, not keyed by domain, has no history of outputs, and is not surfaced at the moment a domain skill is used.
- **Gap Girth fills:** for *any* domain, a short list of house rules plus a history of what recent deliverables chose, delivered **at the moment the domain's skill is used**, with a rule to vary what isn't pinned.
- **Deference:** where a source skill keeps its own confirmed preferences (HyperFrames), those count as *pinned*. Girth does not copy or contradict them.

## 3. Principles
1. **Delivered, not fetched.** The existing `PostToolUse` hook already runs on every Skill call. When the skill maps to a domain with taste, the hook returns the taste as `additionalContext`. Verified on 2026-10-14: a headless `claude -p` run quoted hook-injected `additionalContext` verbatim. This costs no always-on tokens and doesn't depend on the model remembering to look.
2. **Confirmed only.** House rules come from explicit user statements ("always", "never", "every time", "from now on"), never from inference. This is the same rule HyperFrames uses.
3. **Additive.** A new or extra request adds to the house rules and the crafted skill. It never drops them.
4. **Local and private.** Taste is free text the user wrote. It is never part of the share payload, and it lives in its own file that `share` never reads.
5. **Fast.** The hook's taste lookup reads only the small taste files. Budget: < 10 ms.

## 4. Storage
`taste.jsonl` has two tiers, merged on read:
- **Project:** `<cwd>/.girth/taste.jsonl`. Can be committed, so a team shares house rules. Evals seed this tier.
- **Personal:** `GIRTH_HOME/taste.jsonl` (default `~/.girth`). Carries across projects; this is the launch-video case.

Events (`v: 1`, `ts`, `kind`). Malformed or unknown lines are skipped.

| kind | fields | written by |
|---|---|---|
| `rule` | `domain`, `text`, `op` (`add`/`drop`) | `girth taste add` / `girth taste drop` |
| `made` | `domain`, `project`, `choices {key: value}` | `girth made` |
| `link` | `domain`, `skill` | `girth validate` when it records a craft: one per source skill named in `src` markers. It goes to the personal tier. |

- **Domain:** the crafted-skill slug (`girth-<domain>` ↔ `<domain>`).
- **Choice keys:** lowercased; runs of non-alphanumerics become `-`.
- **Values:** trimmed free text, at most 80 characters.

## 5. Merged view (`girth taste show DOMAIN`)
- **Rules:** replay `add`/`drop` per tier in order. Personal rules come first, then project rules tagged `(project)`. Duplicate texts collapse. At most 12 are shown.
- **Recent choices:** from `made` events in both tiers, newest first.
  - Per key: the last 3 distinct values.
  - Keys are ordered by most recent use, at most 8.

Text form:
```
taste: launch-video
house rules (always apply):
  1. End on the logo lockup with the site URL.
  2. (project) Captions in sentence case.
recently made (newest first; vary these unless a rule or the user pins them):
  palette: neon-on-black, paper-ink
  music: synthwave
record this one when done: <launcher> made launch-video key=value ...
```
- When nothing is stored: `taste: launch-video (nothing remembered yet)`.
- `--json` gives `{domain, rules[{n, text, tier}], recent{key: [values]}}`.

## 6. Commands
| Command | Behaviour |
|---|---|
| `girth taste` | Lists the known domains with rule and made counts (both tiers), plus link-only domains. Prints `no taste remembered yet` when there are none. |
| `girth taste show DOMAIN [--json]` | §5. |
| `girth taste add DOMAIN TEXT [--project]` | Appends a rule (personal tier by default) and prints `remembered for DOMAIN: TEXT`. Empty text is a usage error (exit 2). |
| `girth taste drop DOMAIN N [--project]` | Drops rule N as numbered by `show`, but only from the chosen tier. Prints `dropped …`, or `no rule N in DOMAIN` with exit 1. |
| `girth made DOMAIN KEY=VALUE…` | Appends one `made` event to the personal tier and prints `remembered N choices for DOMAIN`. A malformed pair is a usage error (exit 2). |
| `girth record use` (hook) | As in v0.2. After recording, it resolves domains for the skill: `girth-<d>` → `d`, plus `link` events naming the skill (at most 2 domains). For each domain that has any rules or `made` events, it prints one hook JSON with `additionalContext` set to the show text (capped at 1,500 bytes). Otherwise it prints nothing. It always exits 0. |

- **Launcher path:** the launcher exports `GIRTH_LAUNCHER` before `exec`. The show text names it, or plain `girth` when it isn't set.
- **Domain validation:** `DOMAIN` must be a valid slug (`[a-z0-9]+(-[a-z0-9]+)*`). Otherwise it's a usage error (exit 2).

## 7. Skill text (`skills/girth/SKILL.md` and `references/taste.md`)
- **Intents table:** add *"remember or forget a preference, or see past choices"* → `references/taste.md`.
- **Step 1:** run `girth taste` and reuse an existing domain slug when one fits, so taste carries across projects.
- **New section, "Taste (recurring deliverables)":**
  1. When `girth taste show <domain>` (or hook context) lists house rules, apply every rule.
  2. For each recent key, choose something new unless a rule, the user's request or a source skill's confirmed preference pins it. Say in one line what is fresh this time.
  3. When the deliverable is done, run `girth made <domain> key=value…` for the choices that define it.
  4. When the user states a lasting preference, run `girth taste add` and confirm in one line.
  5. A new request extends the rules; it never drops them.
- **Always-on budget:** the description stays trigger-only. Measure always-on bytes before and after, and report the delta.

## 8. Privacy
- `share` never opens `taste.jsonl`.
- A CLI test seeds both tiers with sentinel strings, runs `share preview`, and asserts that no sentinel appears.
- `stats` shows only counts (domains, rules, made). Rule texts appear only in `taste show` and in the hook context.

## 9. Risks
- **Compliance on `made`:** history only grows if the model runs `girth made`.
  - Mitigations: the command is named in the hook context itself, at the moment of use; the eval asserts it is called.
  - If real use shows that it's skipped, a `Stop`-hook nudge is the fallback (deferred).
- **Domain drift:** the model may choose `video` in one project and `launch-video` in another. Mitigation: step 1 lists existing domains first.
- **Hook noise:** context is injected only when taste exists for the domain, capped at 1.5 KB.

## 10. Testing
- **Unit:** taste event parsing (malformed, unknown), rule replay (add/drop/duplicates/tiers), recent-value ordering and caps, key slugging, `KEY=VALUE` parsing, domain resolution (prefix and link, cap 2), show text and JSON.
- **Property:**
  - Parsing never panics.
  - `show` never lists a dropped rule.
  - The recent list never repeats a value within a key.
- **CLI:** taste list/show/add/drop (both tiers, exit codes); `made`; the hook prints `additionalContext` JSON only when taste exists, and nothing for unrelated skills or bad stdin; `validate` writes `link` events; `share preview` contains no taste sentinel; `stats` counts.
- **Launcher:** `GIRTH_LAUNCHER` is exported.
- **Skill text:** names `references/taste.md`, `girth made` and `girth taste add`; still ≤ 700 words; reference ≤ 1.5 KB.
- **Eval** `taste-launch`:
  - The scaffold seeds the project tier with a house rule (the end card shows `girth.dev`) and three `made` events using the palette `neon-on-black`.
  - The prompt asks for a scene-list plan in `plan.md` (no rendering).
  - Graders:
    - `plan.md` contains `girth.dev`.
    - `plan.md` does not contain `neon-on-black`.
    - Bash is called with `made launch-video`.
- **Performance:** the hook's taste path on a 1K-event taste file goes in `docs/perf.md`.

## 11. Success criteria
1. In a new project, a lasting preference stated once is applied in the next session's deliverable without being repeated by the user.
2. Consecutive deliverables in a domain vary the unpinned choices, and the user is told what is fresh.
3. No always-on growth beyond the description delta reported in the release notes, with hook latency < 10 ms.
4. Coverage ≥ 90%, mutants ≥ 80% caught, evals 4 of 4 passing.
