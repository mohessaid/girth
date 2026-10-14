# Girth v0.2: Local Skill Ledger

**Date:** 2026-10-14
**Status:** Approved in conversation ("go ahead"), with a review on GitHub at the author's convenience
**Builds on:** `2026-10-14-girth-jit-skills-design.md`
**Out of scope:** the hosted index, crawler and upload (sub-projects 3a/3b, pending the author's decision), and taste/variation memory (sub-project 2).

## 1. Problem
The skill ecosystem has no outcome measurements. Stars are bookmarks, installs are gameable, and users don't write reviews. Girth sits at the moment of use, so it can measure what nobody else can: which skills get **used**, which **sections** survive in crafted skills, what gets **removed**, and how users **rate** the results. v0.2 records this **locally**, shows it back to the user, and asks consent once for future anonymous sharing.

## 2. Principles
1. **Invisible when working.** No new steps for the user. The model does at most one extra command per flow.
2. **Fast.** Recording a use is a single append (target < 10 ms). `stats` and `feedback` take < 50 ms. No network calls.
3. **Private by construction.** Nothing leaves the machine in v0.2. The future share payload contains **only source-section content hashes plus counts and percentages**: no names, paths, text or notes.
4. **Inspectable.** The ledger is a plain JSONL file the user can read or delete.

## 3. Storage
- `GIRTH_HOME` (default `~/.girth`) holds two files:
  - `ledger.jsonl`: append-only, one event per line.
  - `config.json`: `{"sharing": "allowed" | "denied"}`. If the key is absent, the user hasn't been asked yet.
- Every event has `{"v": 1, "ts": <unix seconds>, "kind": ...}`.
- Unknown or malformed lines are skipped when reading, so the ledger never breaks the tool.

### Events
| kind | fields | written by |
|---|---|---|
| `use` | `skill`, `project` | the plugin hook → `girth record use` |
| `craft` | `skill`, `project`, `sections[] {heading, hash, src[]}`, `sources[] {skill, total, used[] (slugs), hashes{slug→hash}}`, `added[]`, `removed[] {hash, src[]}`, `body_bytes` | `girth validate` on success (`--no-record` to skip) |
| `feedback` | `skill`, `project`, `rating` (`up`/`down`), `note` (optional, **local only**) | `girth feedback` |

`project` is the absolute working directory. It is stored locally only and never shared.

## 4. Sections, hashes and provenance
- **Section:** a markdown body splits at every ATX heading. A section is its heading line plus the content up to the next heading. Text before the first heading is ignored.
- **Slug:** the heading text lowercased, with non-alphanumeric runs replaced by `-` and trimmed.
- **Normalised content:** lowercase; HTML comments removed; markdown punctuation `#*_`>|` and list markers removed; whitespace collapsed.
- **Hash:** the first 16 hex characters of SHA-256 over the normalised section content (heading included). This is stable across machines, so a future server can match it against the public skills it has crawled.
- **Provenance marker:** a crafted section declares its origin on the heading line:
  `## Caption timing <!-- src: video-mega#caption-timing, hyperframes-core#captions -->`
  - Markers are optional for validity.
  - `extract_headings` and slugs ignore HTML comments.
  - On a `craft` event, each `src` skill is resolved through discovery (the configured roots) to record `total` sections, the `used` slugs and their content hashes.
- **Diff on validate:** compare against the previous `craft` event for the same (project, skill).
  - `added`: new section hashes.
  - `removed`: section hashes that disappeared, with their `src`.
  - Edits show up as a removal plus an addition.

## 5. Commands
| Command | Behaviour |
|---|---|
| `girth record use [--skill NAME]` | Without `--skill`, reads the hook JSON from stdin (`tool_input.skill`, `cwd`). Appends a `use` event. **Always exits 0**; it never disturbs the session. |
| `girth validate DIR…` | As in v0.1. For each VALID `girth-*` dir it also appends a `craft` event, unless `--no-record` is given. Prints `recorded: +A −R sections` next to `VALID`. |
| `girth feedback SKILL up\|down [--note TEXT]` | Appends a `feedback` event. |
| `girth stats [--json]` | Local dashboard: crafted skills with uses, sections, removed and ratings; source usage (`used/total` and %); top used skills; sharing status. |
| `girth share status\|allow\|deny\|preview` | Reads or sets consent. `preview` prints the exact payload that *would* be shared (§6). v0.2 never uploads. |

## 6. The share payload (preview only in v0.2)
```json
{
  "girth": "0.2.0",
  "sections": [{"h": "<hash>", "used": 3, "kept": 2, "removed": 1, "up": 1, "down": 0}],
  "skills": [{"h": "<hash of sorted section hashes>", "total": 52, "used": 4}]
}
```
- Only **source** sections appear: the ones referenced by `src` markers.
- Crafted-skill text and hashes are never included, since a crafted skill may contain project specifics.
- Notes, names, paths and the project are never included. A test asserts this.

## 7. UX in Claude Code
- **Consent:** the first time Girth crafts, `girth share status` returns `unset`, and the skill asks once with AskUserQuestion. The three options are **Allow**, **Don't allow** and **Show me exactly what would be shared** (which runs `preview`).
  - The prompt states plainly that nothing is uploaded until the Girth index exists.
  - The answer is stored, and the question is never asked again.
- **Intents** (sub-flows live in `skills/girth/references/` and are loaded only when used):
  - `review <skill>`: a multi-select of sections to remove → edit → `validate` (which records the removal).
  - `feedback`: up or down, plus an optional local note.
  - `stats`: run and summarise.
  - `sharing`: status, change, preview.
- **Hook:** `hooks/hooks.json` adds `PostToolUse` with matcher `Skill`, running `girth record use`. Errors are swallowed.

## 8. Testing
- **Unit:** sectioning, slugs, normalisation, hashing, marker parsing, ledger read/write (with malformed lines), diffing, aggregation, consent config.
- **Property:**
  - Sectioning never panics.
  - A hash is invariant to whitespace and case changes.
  - **The share payload never contains any skill name, path, heading text or note.**
- **CLI:** record use from stdin JSON, validate recording with added/removed counts, feedback, stats text and JSON, share commands.
- **Plugin:** `claude plugin validate` accepts the hook.
- **Eval:** the existing 3 cases still pass. Recording goes to the sandbox's home.
- **Performance:** a release-build measurement of `record use` and `stats` over a 10K-event ledger, documented in `docs/perf.md` (not a CI assertion, which would be flaky).

## 9. Success criteria
1. A real session on the author's machine produces `use` and `craft` events with no visible extra steps.
2. `girth stats` explains in one screen which source skills earn their place (`used/total`, kept and removed).
3. `girth share preview` contains nothing identifying (the property test plus manual review).
4. `record use` takes < 10 ms and `stats` < 50 ms on a 10K-event ledger (release build).
5. Coverage ≥ 90%, mutants ≥ 80% caught, evals 3/3 passing.
