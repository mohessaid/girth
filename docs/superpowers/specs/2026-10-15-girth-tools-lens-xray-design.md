# Girth v0.6: Lens for Tools and MCP, and Context X-ray

**Date:** 2026-10-15
**Status:** Approved in conversation (the author picked all four beyond-skills directions; this is the first, as recommended). Review at the author's convenience.
**Research:** `docs/research/2026-10-15-beyond-skills-brainstorm.md`
**Builds on:** `2026-10-15-girth-lens-meter-design.md` (v0.5)
**Later releases (own specs):** v0.7 cache doctor + loop breaker; v0.8 cheap subagents; v0.9 cost per outcome.

## 1. Problem
Measured on the author's machine, Claude Code adds 68.7 KB of text to a request:
- 44% is the skill listing, which v0.5 Lens now handles;
- **21% is MCP server instructions** (5 servers, 14.3 KB);
- **10.5% is the deferred-tools list** (157 lines; Figma alone lists 41 tools).

Most projects touch none of these servers. Nobody can see this composition today.

## 2. Goals
1. **MCP Lens:** per project, keep in full the MCP instructions and deferred-tool names of servers this project uses. Collapse the rest to one line per server, with every server name kept. Nothing is disconnected.
2. **Just-in-time instructions:** when the model first calls a tool of a collapsed server in a session, it gets that server's instructions before the call runs.
3. **X-ray:** `/girth-xray` shows what the last request was made of (each attachment, system section and tool description), with bytes, share and cost, and what Lens removed.

## 3. Evidence: which servers a project uses
- **Server id:** the tool prefix `mcp__<id>__`. The instructions heading "## claude.ai Figma" maps to the id by replacing each run of characters other than letters and digits with `_` (`claude_ai_Figma`). A match is case-sensitive on the normalised forms.
- **Turn records:** the mod notes the server ids of MCP `tool.call`s during a turn. `turn.complete` sends them as `mcp: [ids]` in the Meter record, so `Turn` gains `mcp: Vec<String>` (default empty, skipped when empty). No arguments or results are recorded.
- **Keep rules** match skills Lens exactly, with servers in place of skills:
  - used in this project, or anywhere in the last 30 days;
  - new (first seen fewer than 14 days ago, and not in the baseline);
  - pinned in `lens.keep`, which accepts server ids too.

  `seen` events record server ids under the prefix `mcp:` (`mcp:claude_ai_Figma`), so the baselines of skills and servers stay separate.

## 4. `girth lens --kind mcp|tools`
`--kind` defaults to `skills` (v0.5 behaviour, unchanged).

**`mcp`** (the `mcp_instructions_delta` text):
- **Parse:** the preamble runs up to the first `## ` heading. Each section runs from one `## ` heading to the next.
- **Rewrite:** kept sections stay byte-identical. The others are removed, and one line is appended:

  `## Other connected servers (their instructions arrive on first use): Name A, Name B`

  The text is unchanged when fewer than 1 section would be removed.
- **JSON:** `{text, described, collapsed, bytes_before, bytes_after, deferred: {id: section_text}}`. The mod keeps `deferred` for JIT delivery.

**`tools`** (the `deferred_tools_delta` text):
- **Rewrite:** lines `mcp__<id>__<tool>` of a collapsed server are replaced, at the position of the server's first line, by one line:

  `mcp__<id>__* (<n> tools; ToolSearch finds them)`

  Every other line is kept verbatim, and the text is unchanged when no server collapses.
- **Same JSON keys,** with `deferred` empty.

**Invariant (property test):** every server id present in the input is present in the output.

## 5. The mod
- `prompt.attachment` with `type` `mcp_instructions_delta` and `deferred_tools_delta` is handled like `skill_listing`: memoised per text, failing open, with byte counts kept for the meter.
- **JIT:** on `tool.call` whose tool is `mcp__<id>__*`, the mod checks `deferred[id]`. On the first such call in the session it answers `{ deny: "<id> instructions (Girth held them back until first use; read them, then call the tool again):\n\n" + text }` and marks the id delivered. Every later call passes through. The deny costs one extra call per server per session.
- **Usage:** `tool.call` for `mcp__<id>__*` adds `id` to the turn's set, and `turn.complete` sends `mcp` (a sorted list).
- **X-ray:** the mod observes every `prompt.attachment`, `prompt.section` and `tool.describe`, recording the sizes in a snapshot keyed by part:
  - `{kind: "attachment" | "section" | "tool", name, bytes, before?}`;
  - `before` is set where Lens rewrote the part;
  - the latest value per part wins.

  `/girth-xray` runs `girth xray` with the snapshot as stdin JSON `{model, parts}` and prints the result.

## 6. `girth xray`
- **Input:** stdin `{model, parts:[{kind, name, bytes, before?}]}`.
- **Output:** a table sorted by bytes:
  - each part's share of the total;
  - tokens (bytes ÷ 4);
  - cost per request at the model's cache-read rate, plus the once-per-session cache-write cost;
  - a final "Lens removed" line summing `before − bytes`.
- **`--json`** gives the same data.
- **Empty input:** `x-ray: nothing captured yet (send a message first)`.

## 7. Error handling
Every hook fails open (`.catch` → `next(e)`):
- A failed `lens --kind …` leaves the text unchanged.
- JIT never blocks twice for the same server in a session. If the mod has no `deferred` entry, the call passes through.
- X-ray only observes; it never changes an event.

## 8. Testing
- **Rust unit and property tests:**
  - section parsing (preamble, `#` subheadings inside a section, no sections);
  - id normalisation;
  - the keep rules for servers;
  - the tools-line grouping (non-MCP lines are kept, and the first-line position is kept);
  - **no server id is ever lost**;
  - `Turn.mcp` round trip;
  - `share` ignores it;
  - `xray` rendering, sorting, shares, the Lens line, the empty case and unknown-model pricing.
- **CLI tests:** `lens --kind mcp|tools` (text and JSON), `GIRTH_LENS=off`, `xray`.
- **Mod tests:**
  - rewrites for both attachment types;
  - the JIT deny happens exactly once per server, and unrelated tools pass;
  - turn records carry `mcp`;
  - X-ray snapshot and command;
  - fail-open paths.
- **Live A/B (`examples/lens/`):**
  1. **Cost:** the bakery task with skills Lens on in both arms, and MCP Lens off versus on.
  2. **Hidden server:** a task that needs a collapsed server, e.g. an Excalidraw diagram through the connected claude.ai Excalidraw server. Pass when that server's tool is called and succeeds after the JIT note.
- **Mutation testing:** ≥ 80% on the new Rust code.
- **Gates:** `make check` and `make evals` must stay green.

## 9. Success criteria
1. On the author's machine, `mcp_instructions_delta` plus `deferred_tools_delta` shrink by ≥ 60% in a project that uses no MCP server, with every server name present.
2. The hidden-server task passes 3 of 3, with the JIT note delivered once per run.
3. The cost A/B shows lower prompt tokens at equal task quality.
4. `/girth-xray` matches the probe's composition within rounding.
5. Every gate is green and v0.6.0 is released.
