-- The columns of Hermes Agent's ~/.hermes/state.db (v0.19.0) that girth import reads.
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    model TEXT,
    parent_session_id TEXT,
    started_at REAL NOT NULL,
    ended_at REAL,
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    cache_read_tokens INTEGER DEFAULT 0,
    cache_write_tokens INTEGER DEFAULT 0,
    reasoning_tokens INTEGER DEFAULT 0,
    cwd TEXT,
    git_repo_root TEXT,
    estimated_cost_usd REAL,
    actual_cost_usd REAL,
    api_call_count INTEGER DEFAULT 0
);
CREATE TABLE session_model_usage (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    billing_provider TEXT NOT NULL DEFAULT '',
    billing_base_url TEXT NOT NULL DEFAULT '',
    billing_mode TEXT NOT NULL DEFAULT '',
    task TEXT NOT NULL DEFAULT '',
    api_call_count INTEGER NOT NULL DEFAULT 0,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens INTEGER NOT NULL DEFAULT 0,
    estimated_cost_usd REAL NOT NULL DEFAULT 0,
    actual_cost_usd REAL NOT NULL DEFAULT 0,
    first_seen REAL,
    last_seen REAL,
    PRIMARY KEY (session_id, model, billing_provider, billing_base_url, billing_mode, task)
);
-- A CLI session: per-model rows, one of them an auxiliary task (compression).
INSERT INTO sessions (id, source, model, started_at, cwd, git_repo_root, api_call_count, input_tokens)
  VALUES ('s1', 'cli', 'claude-sonnet-5-5', 1794599000, '/work/hermes-app/src', '/work/hermes-app', 4, 4200);
INSERT INTO session_model_usage VALUES
  ('s1', 'claude-sonnet-5-5', 'anthropic', '', 'api', '', 3, 1200, 800, 50000, 9000, 120, 0.0312, 0, 1794599000, 1794600000.5),
  ('s1', 'claude-haiku-5-5', 'anthropic', '', 'api', 'compression', 1, 3000, 400, 0, 0, 0, 0.0011, 0, 1794599500, 1794599600);
-- A gateway session: totals on the session row only.
INSERT INTO sessions (id, source, model, started_at, ended_at, cwd, api_call_count, input_tokens, output_tokens, actual_cost_usd, estimated_cost_usd)
  VALUES ('s2', 'telegram', 'gpt-6.1-terra', 1794600050, 1794600100, NULL, 2, 500, 100, 0.002, 0.003);
-- Nothing spent yet.
INSERT INTO sessions (id, source, model, started_at) VALUES ('s3', 'cli', 'claude-sonnet-5-5', 1794600200);
-- A delegated child of s1.
INSERT INTO sessions (id, source, model, parent_session_id, started_at, cwd) VALUES ('s4', 'cli', 'claude-haiku-5-5', 's1', 1794600300, '/work/hermes-app');
INSERT INTO session_model_usage VALUES
  ('s4', 'claude-haiku-5-5', 'anthropic', '', 'api', '', 2, 700, 90, 4000, 0, 0, 0.0009, 0, 1794600300, 1794600400);
