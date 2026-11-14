//! Goose spend as ledger turns, from its session database
//! (`$GOOSE_PATH_ROOT/data/sessions/sessions.db`, else `<XDG data>/goose/sessions/sessions.db`),
//! read through `sqlite3 -readonly -json`.
//!
//! Goose keeps running totals per session (`accumulated_*`), so each import appends what
//! grew since the last one, as [`super::hermes::parse_for`] does for Hermes.
//!
//! **Cache:** Goose's input total *includes* cache reads and writes. Its Bedrock mapping,
//! for one, adds them in (`from_bedrock_usage`, 2026-11). The uncached input is therefore
//! input − cache read − cache write. Only databases with the cache columns (schema 18;
//! `accumulated_cache_read_tokens`) can be split that way; an older one fails [`QUERY`]
//! and is skipped with a note, rather than priced as if nothing was cached.
//!
//! The cost is Goose's own `accumulated_cost` when above zero. A `sub_agent` or `hidden`
//! session is side work (`agent`).

/// One row per session in [`super::hermes::Row`]'s shape. Goose doesn't count requests.
pub const QUERY: &str = "SELECT 'g' AS src, id AS session, \
COALESCE(json_extract(model_config_json, '$.model_name'), '') AS model, '' AS task, \
COALESCE(provider_name, '') AS route, 0 AS calls, \
MAX(0, COALESCE(accumulated_input_tokens, 0) - COALESCE(accumulated_cache_read_tokens, 0) \
- COALESCE(accumulated_cache_write_tokens, 0)) AS input, \
COALESCE(accumulated_output_tokens, 0) AS output, \
COALESCE(accumulated_cache_read_tokens, 0) AS cache_read, \
COALESCE(accumulated_cache_write_tokens, 0) AS cache_write, \
COALESCE(accumulated_cost, 0) AS cost, \
CAST(strftime('%s', COALESCE(updated_at, created_at)) AS REAL) AS ts, \
working_dir AS project, \
CASE WHEN session_type IN ('sub_agent', 'hidden') THEN id END AS parent \
FROM sessions";
