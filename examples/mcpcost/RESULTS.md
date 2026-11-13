# MCP tool schemas: what they add to every request (2026-11-13)

Every MCP tool's JSON schema goes into the prompt unless Claude Code defers it behind ToolSearch. Papers 2610.30151 and 2610.22732, and OpenRouter's measurement of GitHub's server (19.4k tokens per request), say this is a large fixed cost. This probe checks what a mod can see of it.

## The probe (`probe/`)

A mod whose `turn.complete` hook calls `$.session.usage({ breakdown })` with `summary` (estimated locally, free) and `full` (the token-count API), and copies the breakdown's categories and its `mcpTools` rows (server, tool, tokens, `isLoaded`). A `telemetry.log` hook copies the real request's tokens.

Load it with `claude --plugin-dir examples/mcpcost/probe`. It ran on Claude Code 2.1.323 with `claude-haiku-5-5`, with 11 connected servers (228 MCP tools), prompt `Reply with the word ok.`

## Findings

1. **The breakdown names each MCP tool's server and schema tokens.** `serverName` is the same id as the tool prefix (`mcp__claude_ai_github__…` → `claude_ai_github`), so it joins the ledger's per-turn `mcp` list. GitHub's server came to 19,427 tokens (`full`) and 17,089 (`summary`): the 19.4k figure.
2. **With tool search on (the default), every schema is deferred.** The request was 23,740 tokens and cost $0.0026. The breakdown has an `MCP tools (deferred)` row of kind `deferred`.
3. **With `ENABLE_TOOL_SEARCH=false`, every schema is sent.** The same prompt became one request of **148,717** tokens, all cache-written, costing **$0.149: 57×.** The row became `MCP tools`, kind `used`, 131,821 tokens.
4. **The per-tool `isLoaded` flag is wrong when tool search is off.** It said 541 of 131,821 tokens were loaded. Only the category is right. So Girth counts a server's schemas as sent when `isLoaded` is true, or when the breakdown has no `deferred` row at all.
5. **The figures are estimates.** `summary` overstates: its `used` rows sum to 168,817 tokens against the real 148,717 (+13.5%). `full` sums to 188,843 (+27%). Girth records `summary`, which costs nothing, and labels what it shows as an estimate.

## For Girth

Record each session's MCP schema tokens per server, sent and deferred. In the meter and `advise`, multiply a sent server's tokens by the session's requests, price them, and set that against how often the server's tools were called. The fix for a sent server that is never called is to turn tool search back on (unset `ENABLE_TOOL_SEARCH`) or disable the server in `/mcp`.
