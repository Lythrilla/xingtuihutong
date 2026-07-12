CREATE TABLE analytics_events (
  id TEXT PRIMARY KEY,
  user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  event_name TEXT NOT NULL,
  entity_type TEXT,
  entity_id TEXT,
  properties TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE agent_sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE agent_messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
  content TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE agent_tool_calls (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
  message_id TEXT REFERENCES agent_messages(id) ON DELETE SET NULL,
  tool_name TEXT NOT NULL,
  input_json TEXT NOT NULL DEFAULT '{}',
  output_json TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL DEFAULT 'completed',
  duration_ms INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE agent_actions (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  action_type TEXT NOT NULL,
  title TEXT NOT NULL,
  payload TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL DEFAULT 'completed',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_analytics_events_created ON analytics_events(created_at DESC);
CREATE INDEX idx_analytics_events_user ON analytics_events(user_id, created_at DESC);
CREATE INDEX idx_agent_sessions_user ON agent_sessions(user_id, updated_at DESC);
CREATE INDEX idx_agent_messages_session ON agent_messages(session_id, created_at);
CREATE INDEX idx_agent_tool_calls_session ON agent_tool_calls(session_id, created_at);
CREATE INDEX idx_agent_tool_calls_name ON agent_tool_calls(tool_name, created_at DESC);
