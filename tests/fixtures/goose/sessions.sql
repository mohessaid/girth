-- The columns of Goose's sessions.db (schema 18, block/goose 2026-11) that girth import reads.
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    session_type TEXT NOT NULL DEFAULT 'user',
    working_dir TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    accumulated_input_tokens INTEGER,
    accumulated_output_tokens INTEGER,
    accumulated_cache_read_tokens INTEGER,
    accumulated_cache_write_tokens INTEGER,
    accumulated_cost REAL,
    provider_name TEXT,
    model_config_json TEXT
);
-- Input includes the cache: 2,038,113 in = 1,900,000 read + 100,000 written + 38,113 uncached.
INSERT INTO sessions VALUES ('20261114_1', 'user', '/work/goose-app', '2026-11-14 09:00:00', '2026-11-14 10:30:00',
  2038113, 23803, 1900000, 100000, 1.25, 'anthropic', '{"model_name":"claude-sonnet-5-5","context_limit":200000}');
INSERT INTO sessions VALUES ('20261114_2', 'sub_agent', '/work/goose-app', '2026-11-14 10:00:00', '2026-11-14 10:05:00',
  5000, 300, 0, 0, NULL, 'anthropic', '{"model_name":"claude-haiku-5-5"}');
INSERT INTO sessions VALUES ('20261114_3', 'user', '/work/other', '2026-11-14 11:00:00', '2026-11-14 11:00:00',
  NULL, NULL, NULL, NULL, NULL, NULL, NULL);
