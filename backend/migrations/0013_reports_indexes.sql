CREATE INDEX IF NOT EXISTS idx_users_created_at ON users(created_at);
CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations(updated_at);
CREATE INDEX IF NOT EXISTS idx_agent_sessions_created_at ON agent_sessions(created_at);
CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_created_at ON agent_tool_calls(created_at);
CREATE INDEX IF NOT EXISTS idx_settlements_created_status ON settlements(created_at, status);
CREATE INDEX IF NOT EXISTS idx_demand_proposals_created ON demand_proposals(created_at);
