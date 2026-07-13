ALTER TABLE users ADD COLUMN onboarding_status TEXT NOT NULL DEFAULT 'draft';
ALTER TABLE users ADD COLUMN review_note TEXT NOT NULL DEFAULT '';

UPDATE users
SET onboarding_status = CASE WHEN verified = 1 THEN 'approved' ELSE 'draft' END;

ALTER TABLE partners ADD COLUMN source_user_id TEXT REFERENCES users(id) ON DELETE SET NULL;
CREATE UNIQUE INDEX idx_partners_source_user ON partners(source_user_id);

ALTER TABLE songs ADD COLUMN source_user_id TEXT REFERENCES users(id) ON DELETE SET NULL;
CREATE UNIQUE INDEX idx_songs_source_user ON songs(source_user_id);

CREATE TABLE onboarding_applications (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('provider', 'client')),
  entity_name TEXT NOT NULL,
  contact_name TEXT NOT NULL,
  contact_method TEXT NOT NULL,
  description TEXT NOT NULL,
  tags TEXT NOT NULL DEFAULT '[]',
  work_title TEXT NOT NULL DEFAULT '',
  work_url TEXT NOT NULL DEFAULT '',
  audience_size TEXT NOT NULL DEFAULT '',
  cooperation_budget TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'approved', 'rejected')),
  review_note TEXT NOT NULL DEFAULT '',
  submitted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  reviewed_at TEXT,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_onboarding_status ON onboarding_applications(status, submitted_at DESC);

CREATE TRIGGER enforce_unreviewed_users_after_insert
AFTER INSERT ON users
WHEN NEW.onboarding_status != 'approved'
BEGIN
  UPDATE users SET verified = 0 WHERE id = NEW.id;
END;
