ALTER TABLE agent_actions ADD COLUMN dedupe_key TEXT;

CREATE UNIQUE INDEX idx_agent_actions_dedupe
ON agent_actions(dedupe_key)
WHERE dedupe_key IS NOT NULL;

CREATE INDEX idx_agent_actions_user
ON agent_actions(user_id, action_type, created_at DESC);

CREATE INDEX idx_conversation_messages_conversation_created
ON conversation_messages(conversation_id, created_at);
