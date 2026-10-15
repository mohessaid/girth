# Girth v0.6 Tools Lens and X-ray: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans.

**Goal:** collapse the MCP instructions and deferred-tool names of servers a project doesn't use, delivering the instructions just in time on first use, and show the prompt's composition with `/girth-xray`.

**Spec:** `docs/superpowers/specs/2026-10-15-girth-tools-lens-xray-design.md`

## Global Constraints
- **No server id is ever lost** from a rewritten text, and kept sections stay byte-identical.
- **`--kind skills` stays byte-for-byte v0.5.**
- **Every hook fails open.** JIT denies at most once per server per session.
- **`seen` server keys** are `mcp:<id>`. `Turn.mcp` holds ids only.
- **New frozen strings:**
  - `## Other connected servers (their instructions arrive on first use): `
  - `(<n> tools; ToolSearch finds them)`
  - `x-ray: nothing captured yet (send a message first)`
- **Version** 0.6.0.

## Review Focus
1. **A section containing `#` subheadings** (the Atlassian section has 3) stays one section.
2. **A server whose heading normalises to no tool prefix** (instructions present, no tools) is still kept or collapsed by its id, and is never lost.
3. **The JIT deny for an MCP tool reached via ToolSearch:** the first real call gets the note, and the retry passes.
4. **Deltas arriving more than once per session** (servers connecting late): the memo is per text, and the X-ray uses the latest size.
5. **The skill-listing evidence is unaffected** by `mcp:`-prefixed `seen` names.

### Task 1: Evidence refactor + `Turn.mcp`
Extract the keep logic into `Evidence` (skills and servers), keeping v0.5 behaviour (the existing tests stay green). Add `Turn.mcp`, the share sentinel and the round trip.

### Task 2: `lens --kind mcp|tools`
Section parser, id normalisation, tools grouping, JSON `deferred`, the property tests and the CLI tests.

### Task 3: `girth xray`
`xray` module: render and JSON; CLI tests.

### Task 4: The mod
Two more attachment types, JIT deny, `mcp` in turn records, the X-ray snapshot and `/girth-xray`. Mod tests, plus a hand mutation pass.

### Task 5: Live proof
Cost A/B and the hidden-server task; `examples/lens/RESULTS.md` gets a v0.6 section.

### Task 6: Gates and release
`make check`, mutants, evals, docs, CHANGELOG, 0.6.0, merge, CI, tag, verify the installed plugin.
