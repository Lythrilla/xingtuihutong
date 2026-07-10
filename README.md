# 星推互通

微信小程序、Rust API 与后台管理系统。

## 启动后端

```bash
cd backend
cp .env.example .env
# 设置安全的 ADMIN_PASSWORD
cargo run
```

后端首次启动会创建 SQLite 数据库并执行迁移：

- API：`http://127.0.0.1:3000/api`
- 管理后台：`http://127.0.0.1:3000/admin/`
- 健康检查：`http://127.0.0.1:3000/health`

业务内容不会写入演示数据。请登录管理后台新增真实合作方、歌曲和推广方案。

## 连接小程序

在 `miniprogram/config.ts` 中配置可访问的 API 地址。微信真机调试或生产发布时，必须使用已配置到小程序后台的 HTTPS 合法域名。

```bash
npm install
npm run check
```

使用微信开发者工具打开项目，开发者工具会编译 TypeScript 与 Sass。
