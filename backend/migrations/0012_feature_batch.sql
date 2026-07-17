CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO app_settings (key, value) VALUES
  ('contact_unlock_price', '1000'),
  ('membership_monthly_price', '9900'),
  ('membership_quarterly_price', '24900');

CREATE TABLE IF NOT EXISTS memberships (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  plan TEXT NOT NULL,
  active_until TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS contact_unlocks (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  partner_id TEXT NOT NULL REFERENCES partners(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id, partner_id)
);

CREATE INDEX IF NOT EXISTS idx_contact_unlocks_user ON contact_unlocks(user_id);
CREATE INDEX IF NOT EXISTS idx_match_requests_status_created ON match_requests(status, created_at DESC);
