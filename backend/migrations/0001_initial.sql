PRAGMA foreign_keys = ON;

CREATE TABLE users (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  organization TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('provider', 'client')),
  avatar TEXT NOT NULL,
  description TEXT NOT NULL,
  verified INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE user_sessions (
  token TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE partners (
  id TEXT PRIMARY KEY,
  partner_type TEXT NOT NULL CHECK (partner_type IN ('provider', 'client')),
  avatar TEXT NOT NULL,
  avatar_class TEXT NOT NULL,
  name TEXT NOT NULL,
  identity TEXT NOT NULL,
  description TEXT NOT NULL,
  tags TEXT NOT NULL DEFAULT '[]',
  match_score INTEGER NOT NULL DEFAULT 0,
  result_text TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE songs (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  artist TEXT NOT NULL,
  cover_class TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE target_types (
  key TEXT PRIMARY KEY,
  icon_class TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  sort_order INTEGER NOT NULL
);

CREATE TABLE budget_options (
  id TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  min_amount INTEGER,
  max_amount INTEGER,
  sort_order INTEGER NOT NULL
);

CREATE TABLE plans (
  id TEXT PRIMARY KEY,
  icon_class TEXT NOT NULL,
  color_class TEXT NOT NULL,
  title TEXT NOT NULL,
  plan_type TEXT NOT NULL,
  description TEXT NOT NULL,
  tags TEXT NOT NULL DEFAULT '[]',
  budget_amount INTEGER NOT NULL,
  score INTEGER NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE match_requests (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  song_id TEXT NOT NULL REFERENCES songs(id),
  target_keys TEXT NOT NULL,
  budget_id TEXT NOT NULL REFERENCES budget_options(id),
  status TEXT NOT NULL DEFAULT 'completed',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE match_request_plans (
  match_request_id TEXT NOT NULL REFERENCES match_requests(id) ON DELETE CASCADE,
  plan_id TEXT NOT NULL REFERENCES plans(id),
  rank INTEGER NOT NULL,
  PRIMARY KEY (match_request_id, plan_id)
);

CREATE TABLE saved_plans (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  plan_id TEXT NOT NULL REFERENCES plans(id),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id, plan_id)
);

CREATE TABLE notifications (
  id TEXT PRIMARY KEY,
  user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  is_read INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  partner_id TEXT NOT NULL REFERENCES partners(id),
  last_message TEXT NOT NULL,
  unread_count INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(user_id, partner_id)
);

CREATE TABLE conversation_messages (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  sender TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE user_tags (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  tag TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  PRIMARY KEY (user_id, tag)
);

CREATE TABLE certifications (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  icon_class TEXT NOT NULL,
  color_class TEXT NOT NULL,
  status TEXT NOT NULL
);

CREATE TABLE portfolio_cases (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  case_type TEXT NOT NULL,
  name TEXT NOT NULL,
  result_text TEXT NOT NULL,
  color_class TEXT NOT NULL,
  sort_order INTEGER NOT NULL
);

CREATE TABLE wallets (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  balance INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE settlements (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  amount INTEGER NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE admin_sessions (
  token TEXT PRIMARY KEY,
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_notifications_user ON notifications(user_id, created_at DESC);
CREATE INDEX idx_conversations_user ON conversations(user_id, updated_at DESC);
CREATE INDEX idx_match_requests_user ON match_requests(user_id, created_at DESC);

INSERT INTO target_types (key, icon_class, title, description, sort_order) VALUES
('creator', 'video', '短视频创作者', '内容种草与矩阵传播', 1),
('campus', 'campus', '校园音乐人', '年轻圈层与线下活动', 2),
('brand', 'briefcase', '品牌营销机构', '商业联名与场景曝光', 3),
('media', 'audio', '音乐媒体', '榜单、乐评与媒体传播', 4);

INSERT INTO budget_options (id, label, min_amount, max_amount, sort_order) VALUES
('budget-1', '¥5,000 以下', NULL, 5000, 1),
('budget-2', '¥5,000 - 20,000', 5000, 20000, 2),
('budget-3', '¥20,000 - 50,000', 20000, 50000, 3),
('budget-4', '¥50,000 以上', 50000, NULL, 4);
