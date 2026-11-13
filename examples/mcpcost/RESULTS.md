# MCP tool schemas: what they add to every request (2026-11-13)

Every MCP tool's JSON schema goes into the prompt unless Claude Code defers it behind ToolSearch. Papers 2610.30151 and 2610.22732, and OpenRouter's measurement of GitHub's server (19.4k tokens per request), say this is a large fixed cost. This probe checks what a mod can see of it.

## The probe (`probe/`)

A mod whose `turn.complete` hook calls `$.session.usage({ breakdown })` with `summary` (estimated locally, free) and `full` (the token-count API), and copies the breakdown's categories and its `mcpTools` rows (server, tool, tokens, `isLoaded`). A `telemetry.log` hook copies the real request's tokens.

Load it with `claude --plugin-dir examples/mcpcost/probe`. It ran on Claude Code 2.1.323 with `claude-haiku-5-5`, with 11 connected servers (228 MCP tools), prompt `Reply with the word ok.`

## Findings

1. **The breakdown names each MCP tool's server and schema tokens.** `serverName` is the same id as the tool prefix (`mcp__claude_ai_github__…` → `claude_ai_github`), so it joins the ledger's per-turn `mcp` list. GitHub's server came to 19,427 tokens (`full`) and 17,089 (`summary`): the 19.4k figure.
2. **With tool search on (the default), every schema is deferred.** The request was 23,740 tokens and cost $0.0026. The breakdown has an `MCP tools (deferred)` row of kind `deferred`.
3. **With `ENABLE_TOOL_SEARCH=false`, every schema is sent.** The same prompt became one request of **148,717** tokens: **6.3×.** It cost $0.149, 57× the dollars, because the request was all cache writes and crossed Haiku 5.5's 100k-token line, above which the whole request is billed at $0.50 input instead of $0.10. The row became `MCP tools`, kind `used`, 131,821 tokens.
4. **The per-tool `isLoaded` flag is wrong when tool search is off.** It said 541 of 131,821 tokens were loaded. Only the category is right. So Girth counts a server's schemas as sent when `isLoaded` is true, or when the breakdown has no `deferred` row at all.
5. **The figures are estimates.** `summary` overstates: its `used` rows sum to 168,817 tokens against the real 148,717 (+13.5%). `full` sums to 188,843 (+27%). Girth records `summary`, which costs nothing, and labels what it shows as an estimate.
6. **With tool search off, the cache often misses on the second request** (2026-11-13; `orderprobe/` logs each request's cache tokens, the tool list and every tool description lookup).
   - The task was `Use the Bash tool to run: echo hi. Then reply done.`, which takes two requests. In 7 of 14 runs (Haiku 5.5 and Sonnet 5.5), the second request read **0** tokens from the cache and wrote the whole prompt again, about 190k tokens. One Sonnet run cost **$1.47**.
   - With tool search on, the same task's second request read the cache every time (23,791 tokens on Haiku, 22,853 on Sonnet).
   - The second request was always about 190,390 tokens. The first varied between runs (174,670 and 178,704), and when it was 178,704 a later run read it back from the cache. So something enters the start of the prompt between the requests.
   - Tool descriptions were not the cause: `Skill`, `Agent` and `ListAgents` change only after the second request.
   - The likely cause, not confirmed: MCP servers that finish connecting after the first request. With tool search off, their schemas join the tools block at the front of the prompt. With tool search on, a late server only appends a deferred-tools message, after the cached prefix.
   - Girth's doctor already names such requests as cache rewrites. The fix is the same as for the schemas: keep tool search on.

## For Girth

Record each session's MCP schema tokens per server, sent and deferred. In the meter and `advise`, multiply a sent server's tokens by the session's requests, price them, and set that against how often the server's tools were called. The fix for a sent server that is never called is to turn tool search back on (unset `ENABLE_TOOL_SEARCH`) or disable the server in `/mcp`.

**Timing (2.1.323, 228 MCP tools):** `$.session.usage({ breakdown: 'summary' })` took 50 ms; `'full'` took 587 ms. The mod uses `summary` after the turn is recorded, bounded at 5 s.
