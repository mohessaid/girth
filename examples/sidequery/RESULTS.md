# Side queries: the requests Claude Code makes outside turns (2026-11-08)

Claude Code also sends model requests outside the conversation. The radar sweep (`docs/radar/2026-11-08.md`) reported one kind, `agent_summary`, at 16–26% of spend. Girth's mod records turns from `turn.complete`, and it never saw these requests. This probe checks whether a mod can see them and what they cost.

## The probe (`probe/`)

The probe is a mod with two hooks.
- **`telemetry.log`** with `{ to: 'collector' }`. It copies each `api_request` record's `query_source`, model, token counts and `cost_usd_micros`.
- **`turn.complete`.** It copies each turn's `usage`.

Load it with `claude --plugin-dir examples/sidequery/probe`.

It ran on Claude Code 2.1.319 with `claude-haiku-5-5`.

## Findings

1. **Collector records reach a mod even with telemetry off.** Each `-p` run gave 21 records with no `CLAUDE_CODE_ENABLE_TELEMETRY` set: `mcp_server_connection`, hook events, `user_prompt`, `api_request` and `assistant_response`.
   - Each `api_request` carries:
     - `query_source` and `model`
     - the token counts: `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens`, `cache_creation_5m_tokens` and `cache_creation_1h_tokens`
     - Claude Code's own price, `cost_usd_micros`
     - `duration_ms`, `ttft_ms`, `speed` and `effort`
   - It also carries identity fields: `user.*`, `organization.id` and request ids. Girth must never copy those.
2. **Turns are exactly the main loop's and subagents' requests.** In an interactive session, all `turn.complete` usage summed to 263,485 tokens for the main loop and 192,705 for the subagent. Those equal, token for token, the sum of the `api_request` rows whose `query_source` is `repl_main_thread` or `agent:builtin:general-purpose`.
3. **Side queries are not in any turn.** The same session sent three other kinds. Girth's ledger had none of them before this change.

   | `query_source` | Requests | Tokens | Cost | Notes |
   |---|---|---|---|---|
   | `prompt_suggestion` | 1 | 43,476 | $0.000509 | read 42,899 tokens of the conversation's cache |
   | `agent_summary` | 4 | 3,188 | $0.000411 | about every 30 s while a background subagent ran; **no cache reads**, and input grew with each one (624 → 827) |
   | `generate_session_title` | 1 | 1,277 | $0.000135 | once per session |

   `agent_summary` sends its input with no cache, at the full input price. On Opus and with a long subagent transcript, each request costs more, which fits the reported 16–26%.
4. **`CLAUDE_CODE_DISABLE_AGENT_PROGRESS_SUMMARIES=1` stops it.** We repeated the background-agent task, four 40-second waits in one subagent, in a fresh session with the variable set. It sent **0** `agent_summary` requests; the first session sent 4. The variable is in the 2.1.319 binary.
5. **`claude -p` sends no `agent_summary`,** even with a background subagent. Only interactive sessions do, so the interactive runs drove a real session in `tmux`.

6. **Collector records carry no agent id** (2026-11-08, two subagents started in parallel under `claude -p`).
   - Every request had the same `prompt.id`, there was no trace span, and the two agents' requests interleaved (sequence numbers 34 and 47 for one agent, 38 and 43 for the other).
   - Each agent's `turn.complete` usage did equal the sum of its own requests: 31,121 = 15,018 + 16,103, and 31,275 = 15,011 + 16,264.
   - So the mod prices a subagent's turn at Claude Code's figure only when the subagent requests pending since the last match add up exactly to that turn. That is the case when one agent runs at a time. Otherwise the turn keeps list prices.

## The binary's own lists (2.1.319, `strings`)

- Turn sources: `repl_main_thread`, `sdk`, `agent:custom`, `agent:default`, `agent:builtin`.
- Sources the binary names besides those include `compact`, `away_summary`, `tool_use_summary_generation`, `resume_cache_warm`, `hook_prompt`, `side_question` and `extract_memories`.
