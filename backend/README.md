# 星推互通 Rust 后端

## 本地运行

```bash
cp .env.example .env
# 修改 ADMIN_PASSWORD
cargo run
```

- API：`http://127.0.0.1:3000/api`
- 后台管理：`http://127.0.0.1:3000/admin/`
- 健康检查：`http://127.0.0.1:3000/health`

数据库使用 SQLite，首次启动自动执行迁移并创建 `data/xingtuihutong.db`。

## Agent 单模型工具调用

Agent 使用一个 OpenAI-compatible Chat Completions 模型完成意图理解、工具选择和结果总结，工具直接读写 SQLite 业务数据库，不依赖 RAG 或向量模型。

```bash
AGENT_MODEL_API_URL=https://your-provider.example/v1/chat/completions
AGENT_MODEL_API_KEY=replace-with-provider-key
```

OpenAI 兼容接口地址、API Key、模型名称、系统 Prompt、温度和工具调用上限均可在管理后台的「Agent 设置」中配置。后台保存的接口配置优先于环境变量；留空时回退到 `AGENT_MODEL_API_URL` 和 `AGENT_MODEL_API_KEY`。未配置接口地址，或模型临时不可用时，会自动使用本地确定性工具编排，业务查询和写入能力仍可运行。
