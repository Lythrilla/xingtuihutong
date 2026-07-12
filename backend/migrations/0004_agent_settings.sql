CREATE TABLE agent_settings (
  id TEXT PRIMARY KEY CHECK (id = 'default'),
  enabled INTEGER NOT NULL DEFAULT 1,
  engine TEXT NOT NULL DEFAULT 'StarConnect Agent Runtime · Data Grounded',
  welcome_message TEXT NOT NULL DEFAULT '你好，我可以直接查询业务数据、检索合作伙伴、推荐方案，并执行收藏或创建跟进任务。',
  system_prompt TEXT NOT NULL DEFAULT '你是星推互通智能运营助手，只能调用当前已启用的工具，并基于实时数据库回答。',
  max_tokens INTEGER NOT NULL DEFAULT 1000,
  temperature REAL NOT NULL DEFAULT 0.7,
  max_tool_calls INTEGER NOT NULL DEFAULT 8,
  default_suggestions TEXT NOT NULL DEFAULT '["查询我最近 7 天的合作数据","帮我找短视频推广服务方","推荐 2 万元内的推广方案","分析当前合作转化漏斗"]',
  follow_up_suggestions TEXT NOT NULL DEFAULT '["找出最值得提升的转化环节","推荐适合当前数据表现的方案","创建本周跟进任务"]',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE agent_tools (
  name TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 1,
  label TEXT NOT NULL,
  description TEXT NOT NULL,
  mode TEXT NOT NULL CHECK (mode IN ('read', 'write')),
  keywords TEXT NOT NULL DEFAULT '[]',
  blocked_keywords TEXT NOT NULL DEFAULT '[]',
  keyword_groups TEXT NOT NULL DEFAULT '[]',
  required_tools TEXT NOT NULL DEFAULT '[]',
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO agent_settings (id) VALUES ('default');

INSERT INTO agent_tools (name, enabled, label, description, mode, keywords, blocked_keywords, keyword_groups, required_tools, sort_order) VALUES
('query_business_metrics', 1, '业务数据查询', '读取实时匹配、会话、收藏与收益', 'read', '["数据","统计","趋势","表现","多少","收益"]', '[]', '[]', '[]', 1),
('inspect_collaboration_pipeline', 1, '合作漏斗分析', '分析发现、匹配、沟通与结算转化', 'read', '["漏斗","转化","pipeline"]', '[]', '[]', '[]', 2),
('search_partners', 1, '合作伙伴检索', '按身份、能力和匹配度查询伙伴', 'read', '["找","伙伴","服务","需求","达人","合作方"]', '[]', '[]', '[]', 3),
('recommend_plans', 1, '推广方案推荐', '结合预算与目标查询可执行方案', 'read', '["方案","推广","预算","推荐","投放"]', '[]', '[]', '[]', 4),
('connect_partner', 1, '发起合作会话', '连接检索结果中的最佳伙伴并建立会话', 'write', '["联系","沟通","合作","会话","对接"]', '["联系过","已联系","联系记录","联系状态","沟通数据","沟通记录"]', '[["帮我联系"],["请联系"],["直接联系"],["立即联系"],["联系最佳"],["联系最合适"],["联系第一"],["联系伙伴"],["联系合作方"],["发起合作"],["开始沟通"],["立即沟通"],["和第一位沟通"],["与第一位沟通"],["建立会话"],["帮我对接"],["直接对接"]]', '["search_partners"]', 5),
('save_recommended_plan', 1, '收藏推荐方案', '将推荐方案写入用户收藏', 'write', '["收藏","保存"]', '["取消收藏","已收藏","收藏了","收藏多少","收藏数据","收藏记录"]', '[["帮我收藏"],["请收藏"],["收藏最佳"],["收藏这个"],["收藏推荐"],["收藏方案"],["保存方案"],["保存这个"],["加入收藏"]]', '["recommend_plans"]', 6),
('create_follow_up_task', 1, '创建跟进任务', '生成任务并发送到消息中心', 'write', '["创建","生成","添加","安排","提醒","执行"]', '["任务记录","已有任务","有哪些任务","执行情况","执行数据"]', '[["提醒我"],["安排跟进"],["执行方案"],["开始执行"],["直接执行"],["创建","任务"],["创建","跟进"],["生成","任务"],["生成","跟进"],["添加","任务"],["添加","跟进"]]', '[]', 7);
