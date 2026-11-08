# Girth: import Antigravity CLI spend

Status: design, 2026-11-08.

## Why

Girth's meter now reads Claude Code, Codex, Copilot, OpenCode and Gemini CLI. Antigravity has a "works with" badge and a skill-folder entry, but no meter. ccusage, tokscale and RTK all read it (`docs/radar/2026-11-08.md`).

## Evidence (agy 1.5.2, this machine)

- **Where:** conversations are `~/.gemini/antigravity-cli/conversations/<uuid>.db`, one SQLite database each. The IDE's `~/.gemini/antigravity/conversations/*.pb` files are a different format and out of scope.
- **`gen_metadata(idx, data)`:** one row per model call. `data` is protobuf. Field `#1` is the chat-model message:
  - `#19`: model id (`gemini-3.8-flash-n`, `gemini-3.6-flash`)
  - `#4`: usage
    - `#2`: uncached input
    - `#5`: cache reads; absent until a cached prefix exists
    - `#3`: output, thinking included. When `#3` is absent, output is `#9 + #10`.
    - `#11`: response id
    - `#1`: a fixed count (1,318 now, 1,071 in September). agy's own usage leaves it out, so Girth does too.
  - Check against `agy -p --output-format json` usage on one conversation, after both runs (4 generations):
    - input 35,102 = Σ`#2`
    - output 475 = Σ`#3`
    - thinking 403 = Σ`#9`
  - tokscale (MIT; `crates/tokscale-core/src/sessions/antigravity_cli.rs`) adds `#1` to input. agy's report says not to.
- **`steps(idx, step_type, metadata)`:**
  - Type 14 is the user's prompt, 15 a model response and 132 a tool call.
  - `metadata` `#1` is `{seconds, nanos}`, and `#9.#11` is the response id of the generation it carries.
- **`trajectory_metadata_blob.data`:**
  - `#1.#1` is the workspace URI (`file:///…`).
  - `#2` is `{seconds, nanos}`, the time the conversation was created.

## What

- **Read through the `sqlite3` command,** as OpenCode is read through `opencode db`, so Girth carries no SQLite code. Open read-only (`file:…?mode=ro`), and select blobs as `hex()`. If `sqlite3` is missing or fails, print a note on stderr and import nothing.
- **A turn** is one user prompt (a type-14 step) and the generations after it, matched to steps by response id.
  - Each generation is one request.
  - A turn's time is its prompt step's time. If a conversation has no steps, the time is its creation time.
  - The project is the workspace path.
  - The harness is `antigravity`.
  - Every turn is a main-loop turn.
- **Done:** a turn is done when a later prompt exists, or when its conversation file hasn't changed for 30 minutes. A turn that isn't done is left for a later run.
- **Cursor:** `import.json` stores how many turns of each conversation were imported.
- **Prices:** no `gemini-3.6-flash` or `gemini-3.8-flash` rows yet, so those turns are unpriced, and `advise` already reports unpriced turns.
- `girth import antigravity` reads `$GEMINI_CLI_HOME/.gemini/antigravity-cli/conversations`, falling back to `~/.gemini/…`. `advise --harness antigravity` names its commands.

## Tests

- **Unit, on synthetic protobuf rows built in the test:**
  - fields and turns
  - the `#9 + #10` fallback
  - the cursor
  - an open last turn
  - a malformed blob: skipped, never a panic
- **CLI:** an import from a fixture database made by `sqlite3` in the test, skipped when `sqlite3` is absent, then a re-import that adds nothing.
- **End to end:** `girth import antigravity` on this machine. The turns of conversation `6959b26d` must sum to agy's own usage: input 35,102, output 475.
