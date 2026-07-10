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
