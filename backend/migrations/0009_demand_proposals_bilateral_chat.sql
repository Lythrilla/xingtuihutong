ALTER TABLE conversations ADD COLUMN participant_user_id TEXT REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE conversations ADD COLUMN context_type TEXT NOT NULL DEFAULT '';
ALTER TABLE conversations ADD COLUMN context_id TEXT NOT NULL DEFAULT '';

ALTER TABLE conversation_messages ADD COLUMN sender_user_id TEXT REFERENCES users(id) ON DELETE SET NULL;

CREATE TABLE conversation_read_states (
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  unread_count INTEGER NOT NULL DEFAULT 0,
  last_read_at TEXT,
  PRIMARY KEY (conversation_id, user_id)
);

INSERT INTO conversation_read_states (conversation_id, user_id, unread_count)
SELECT id, user_id, unread_count FROM conversations;

CREATE TABLE demand_proposals (
  id TEXT PRIMARY KEY,
  match_request_id TEXT NOT NULL REFERENCES match_requests(id) ON DELETE CASCADE,
  provider_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  amount INTEGER NOT NULL CHECK (amount > 0),
  cycle TEXT NOT NULL,
  deliverables TEXT NOT NULL,
  message TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'accepted', 'rejected', 'withdrawn')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(match_request_id, provider_user_id)
);

CREATE INDEX idx_demand_proposals_request
  ON demand_proposals(match_request_id, status, created_at DESC);
CREATE INDEX idx_demand_proposals_provider
  ON demand_proposals(provider_user_id, status, updated_at DESC);
CREATE INDEX idx_conversation_participant
  ON conversations(participant_user_id, updated_at DESC);
