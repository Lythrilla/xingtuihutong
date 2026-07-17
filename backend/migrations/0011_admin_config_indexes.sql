CREATE INDEX IF NOT EXISTS idx_partners_active_type_score
  ON partners(active, partner_type, match_score);
CREATE INDEX IF NOT EXISTS idx_match_requests_status
  ON match_requests(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_users_onboarding_status
  ON users(onboarding_status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_settlements_status
  ON settlements(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_songs_active_source
  ON songs(active, source_user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_plans_active_score
  ON plans(active, score DESC);
