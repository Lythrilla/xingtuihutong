ALTER TABLE agent_settings ADD COLUMN model TEXT NOT NULL DEFAULT 'default';
ALTER TABLE agent_settings ADD COLUMN max_history INTEGER NOT NULL DEFAULT 30;
ALTER TABLE agent_settings ADD COLUMN fallback_reply TEXT NOT NULL DEFAULT '我暂时无法处理这个请求，请换一种方式描述或联系管理员。';
ALTER TABLE agent_settings ADD COLUMN suggestion_count INTEGER NOT NULL DEFAULT 3;
