use crate::{admin::require_admin, api::current_user, error::AppResult, state::AppState};
use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

pub fn user_routes() -> Router<AppState> {
    Router::new().route("/overview", get(user_overview))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new().route("/", get(admin_overview))
}

pub async fn track_event(
    state: &AppState,
    user_id: Option<&str>,
    event_name: &str,
    entity_type: Option<&str>,
    entity_id: Option<&str>,
    properties: Value,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO analytics_events
         (id, user_id, event_name, entity_type, entity_id, properties)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(event_name)
    .bind(entity_type)
    .bind(entity_id)
    .bind(properties.to_string())
    .execute(&state.pool)
    .await?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserAnalytics {
    metrics: Vec<Metric>,
    trend: Vec<UserTrendPoint>,
    funnel: Vec<FunnelStage>,
    channels: Vec<DistributionItem>,
    opportunity: Opportunity,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Metric {
    key: String,
    label: String,
    value: i64,
    display_value: String,
    change: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserTrendPoint {
    date: String,
    label: String,
    matches: i64,
    connections: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FunnelStage {
    label: String,
    value: i64,
    conversion: i64,
}

#[derive(Serialize)]
struct DistributionItem {
    label: String,
    value: i64,
    percent: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Opportunity {
    score: i64,
    title: String,
    description: String,
    action: String,
}

async fn user_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<UserAnalytics>> {
    let user = current_user(&state, &headers).await?;
    let matches = user_count(
        &state,
        "SELECT COUNT(*) FROM match_requests WHERE user_id = ?",
        &user.id,
    )
    .await?;
    let connections = user_count(
        &state,
        "SELECT COUNT(*) FROM conversations WHERE user_id = ?",
        &user.id,
    )
    .await?;
    let saved = user_count(
        &state,
        "SELECT COUNT(*) FROM saved_plans WHERE user_id = ?",
        &user.id,
    )
    .await?;
    let revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM settlements
         WHERE user_id = ? AND status = 'completed' AND amount > 0",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    let previous_matches: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM match_requests
         WHERE user_id = ? AND created_at >= datetime('now', '-14 days')
         AND created_at < datetime('now', '-7 days')",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    let recent_matches: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM match_requests
         WHERE user_id = ? AND created_at >= datetime('now', '-7 days')",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    let trend = user_trend(&state, &user.id).await?;
    let channels = user_channels(&state, &user.id).await?;
    let completed = user_count(
        &state,
        "SELECT COUNT(*) FROM settlements WHERE user_id = ? AND status = 'completed'",
        &user.id,
    )
    .await?;
    let discovered: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM partners WHERE active = 1")
        .fetch_one(&state.pool)
        .await?;
    let score = (55 + matches * 6 + connections * 8 + saved * 3).clamp(55, 98);
    let opportunity = if matches == 0 {
        Opportunity {
            score,
            title: "生成第一份 AI 推广阵容".into(),
            description: "补充目标和预算后，Agent 可自动查询并组合推广资源。".into(),
            action: "开始 AI 匹配".into(),
        }
    } else if connections == 0 {
        Opportunity {
            score,
            title: "优先建立首个合作会话".into(),
            description: "高匹配伙伴已经就绪，主动沟通能显著提升方案落地率。".into(),
            action: "去找合作".into(),
        }
    } else {
        Opportunity {
            score,
            title: "扩大高转化渠道投入".into(),
            description: channels
                .first()
                .map(|item| format!("{}是当前使用最多的渠道，可继续验证规模化效果。", item.label))
                .unwrap_or_else(|| "持续积累合作数据后，Agent 会自动识别高转化渠道。".into()),
            action: "询问 AI Agent".into(),
        }
    };
    Ok(Json(UserAnalytics {
        metrics: vec![
            metric(
                "matches",
                "智能匹配",
                matches,
                matches.to_string(),
                growth(recent_matches, previous_matches),
            ),
            metric(
                "connections",
                "合作会话",
                connections,
                connections.to_string(),
                0,
            ),
            metric("saved", "收藏方案", saved, saved.to_string(), 0),
            metric("revenue", "累计收益", revenue, format_money(revenue), 0),
        ],
        trend,
        funnel: vec![
            funnel("可用伙伴池", discovered, discovered),
            funnel("发起匹配", matches, discovered),
            funnel("建立沟通", connections, matches),
            funnel("完成结算", completed, connections),
        ],
        channels,
        opportunity,
    }))
}

fn metric(key: &str, label: &str, value: i64, display_value: String, change: i64) -> Metric {
    Metric {
        key: key.into(),
        label: label.into(),
        value,
        display_value,
        change,
    }
}

fn funnel(label: &str, value: i64, previous: i64) -> FunnelStage {
    FunnelStage {
        label: label.into(),
        value,
        conversion: if previous > 0 {
            (((value as f64 / previous as f64) * 100.0).round() as i64).clamp(0, 100)
        } else {
            0
        },
    }
}

async fn user_count(state: &AppState, query: &str, user_id: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar(query)
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?)
}

async fn user_trend(state: &AppState, user_id: &str) -> AppResult<Vec<UserTrendPoint>> {
    let match_rows = sqlx::query(
        "SELECT date(created_at) AS day, COUNT(*) AS total
         FROM match_requests WHERE user_id = ? AND created_at >= datetime('now', '-6 days')
         GROUP BY date(created_at)",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    let connection_rows = sqlx::query(
        "SELECT date(first_seen) AS day, COUNT(*) AS total FROM (
           SELECT c.id, COALESCE(MIN(m.created_at), c.updated_at) AS first_seen
           FROM conversations c
           LEFT JOIN conversation_messages m ON m.conversation_id = c.id
           WHERE c.user_id = ?
           GROUP BY c.id
         )
         WHERE first_seen >= datetime('now', '-6 days')
         GROUP BY date(first_seen)",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    let matches = rows_to_map(match_rows);
    let connections = rows_to_map(connection_rows);
    Ok((0..7)
        .rev()
        .map(|offset| {
            let day = (Utc::now() - Duration::days(offset)).date_naive();
            let key = day.format("%Y-%m-%d").to_string();
            UserTrendPoint {
                date: key.clone(),
                label: day.format("%m/%d").to_string(),
                matches: matches.get(&key).copied().unwrap_or_default(),
                connections: connections.get(&key).copied().unwrap_or_default(),
            }
        })
        .collect())
}

async fn user_channels(state: &AppState, user_id: &str) -> AppResult<Vec<DistributionItem>> {
    let rows = sqlx::query("SELECT target_keys FROM match_requests WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(&state.pool)
        .await?;
    let mut counts: HashMap<String, i64> = HashMap::new();
    for row in rows {
        let value: String = row.get("target_keys");
        for key in serde_json::from_str::<Vec<String>>(&value).unwrap_or_default() {
            *counts.entry(channel_label(&key).into()).or_default() += 1;
        }
    }
    distribution(counts)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminAnalytics {
    metrics: Vec<Metric>,
    trend: Vec<AdminTrendPoint>,
    funnel: Vec<FunnelStage>,
    partner_mix: Vec<DistributionItem>,
    tool_usage: Vec<DistributionItem>,
    recent_runs: Vec<AgentRun>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminTrendPoint {
    date: String,
    label: String,
    users: i64,
    matches: i64,
    connections: i64,
    revenue: i64,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct AgentRun {
    id: String,
    user_name: String,
    title: String,
    status: String,
    tool_calls: i64,
    updated_at: String,
}

async fn admin_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<AdminAnalytics>> {
    require_admin(&state, &headers).await?;
    let users = count(&state, "SELECT COUNT(*) FROM users").await?;
    let partners = count(&state, "SELECT COUNT(*) FROM partners WHERE active = 1").await?;
    let matches = count(&state, "SELECT COUNT(*) FROM match_requests").await?;
    let connections = count(&state, "SELECT COUNT(*) FROM conversations").await?;
    let agent_calls = count(&state, "SELECT COUNT(*) FROM agent_tool_calls").await?;
    let revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM settlements
         WHERE status = 'completed' AND amount > 0",
    )
    .fetch_one(&state.pool)
    .await?;
    let engaged_users: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT user_id) FROM (
           SELECT user_id FROM match_requests
           UNION ALL
           SELECT user_id FROM conversations
         )",
    )
    .fetch_one(&state.pool)
    .await?;
    let connected_users =
        count(&state, "SELECT COUNT(DISTINCT user_id) FROM conversations").await?;
    let settled_users = count(
        &state,
        "SELECT COUNT(DISTINCT user_id) FROM settlements WHERE status = 'completed'",
    )
    .await?;
    let trend = admin_trend(&state).await?;
    let partner_rows = sqlx::query(
        "SELECT partner_type AS label, COUNT(*) AS total FROM partners GROUP BY partner_type",
    )
    .fetch_all(&state.pool)
    .await?;
    let tool_rows = sqlx::query(
        "SELECT tool_name AS label, COUNT(*) AS total FROM agent_tool_calls GROUP BY tool_name",
    )
    .fetch_all(&state.pool)
    .await?;
    let recent_runs = sqlx::query_as::<_, AgentRun>(
        "SELECT s.id, u.organization AS user_name, s.title, s.status,
         COUNT(t.id) AS tool_calls, s.updated_at
         FROM agent_sessions s
         JOIN users u ON u.id = s.user_id
         LEFT JOIN agent_tool_calls t ON t.session_id = s.id
         GROUP BY s.id ORDER BY s.updated_at DESC LIMIT 12",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(AdminAnalytics {
        metrics: vec![
            metric("users", "累计用户", users, users.to_string(), 0),
            metric("partners", "活跃合作方", partners, partners.to_string(), 0),
            metric("matches", "AI 匹配请求", matches, matches.to_string(), 0),
            metric(
                "agentCalls",
                "Agent 工具调用",
                agent_calls,
                agent_calls.to_string(),
                0,
            ),
            metric(
                "connections",
                "合作会话",
                connections,
                connections.to_string(),
                0,
            ),
            metric("revenue", "完成交易额", revenue, format_money(revenue), 0),
        ],
        trend,
        funnel: vec![
            funnel("注册用户", users, users),
            funnel("产生业务行为", engaged_users, users),
            funnel("建立会话", connected_users, engaged_users),
            funnel("完成结算", settled_users, connected_users),
        ],
        partner_mix: distribution(
            partner_rows
                .into_iter()
                .map(|row| {
                    let key: String = row.get("label");
                    (
                        if key == "provider" {
                            "推广服务方"
                        } else {
                            "音乐创作者"
                        }
                        .into(),
                        row.get("total"),
                    )
                })
                .collect(),
        )?,
        tool_usage: distribution(
            tool_rows
                .into_iter()
                .map(|row| {
                    (
                        tool_label(&row.get::<String, _>("label")).into(),
                        row.get("total"),
                    )
                })
                .collect(),
        )?,
        recent_runs,
    }))
}

async fn admin_trend(state: &AppState) -> AppResult<Vec<AdminTrendPoint>> {
    let users = grouped_counts(
        state,
        "SELECT date(created_at) AS day, COUNT(*) AS total FROM users
         WHERE created_at >= datetime('now', '-13 days') GROUP BY date(created_at)",
    )
    .await?;
    let matches = grouped_counts(
        state,
        "SELECT date(created_at) AS day, COUNT(*) AS total FROM match_requests
         WHERE created_at >= datetime('now', '-13 days') GROUP BY date(created_at)",
    )
    .await?;
    let connections = grouped_counts(
        state,
        "SELECT date(first_seen) AS day, COUNT(*) AS total FROM (
           SELECT c.id, COALESCE(MIN(m.created_at), c.updated_at) AS first_seen
           FROM conversations c
           LEFT JOIN conversation_messages m ON m.conversation_id = c.id
           GROUP BY c.id
         )
         WHERE first_seen >= datetime('now', '-13 days')
         GROUP BY date(first_seen)",
    )
    .await?;
    let revenue_rows = sqlx::query(
        "SELECT date(created_at) AS day, COALESCE(SUM(amount), 0) AS total FROM settlements
         WHERE status = 'completed' AND amount > 0
         AND created_at >= datetime('now', '-13 days') GROUP BY date(created_at)",
    )
    .fetch_all(&state.pool)
    .await?;
    let revenue = rows_to_map(revenue_rows);
    Ok((0..14)
        .rev()
        .map(|offset| {
            let day = (Utc::now() - Duration::days(offset)).date_naive();
            let key = day.format("%Y-%m-%d").to_string();
            AdminTrendPoint {
                date: key.clone(),
                label: day.format("%m/%d").to_string(),
                users: users.get(&key).copied().unwrap_or_default(),
                matches: matches.get(&key).copied().unwrap_or_default(),
                connections: connections.get(&key).copied().unwrap_or_default(),
                revenue: revenue.get(&key).copied().unwrap_or_default(),
            }
        })
        .collect())
}

async fn grouped_counts(state: &AppState, query: &str) -> AppResult<HashMap<String, i64>> {
    Ok(rows_to_map(
        sqlx::query(query).fetch_all(&state.pool).await?,
    ))
}

fn rows_to_map(rows: Vec<sqlx::sqlite::SqliteRow>) -> HashMap<String, i64> {
    rows.into_iter()
        .map(|row| (row.get("day"), row.get("total")))
        .collect()
}

fn distribution(counts: HashMap<String, i64>) -> AppResult<Vec<DistributionItem>> {
    let total: i64 = counts.values().sum();
    let mut items: Vec<DistributionItem> = counts
        .into_iter()
        .map(|(label, value)| DistributionItem {
            label,
            value,
            percent: if total > 0 {
                ((value as f64 / total as f64) * 100.0).round() as i64
            } else {
                0
            },
        })
        .collect();
    items.sort_by_key(|item| std::cmp::Reverse(item.value));
    Ok(items)
}

async fn count(state: &AppState, query: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar(query).fetch_one(&state.pool).await?)
}

fn growth(current: i64, previous: i64) -> i64 {
    if previous == 0 {
        return if current > 0 { 100 } else { 0 };
    }
    (((current - previous) as f64 / previous as f64) * 100.0).round() as i64
}

fn channel_label(key: &str) -> &str {
    match key {
        "creator" => "短视频创作者",
        "campus" => "校园音乐人",
        "brand" => "品牌营销",
        "media" => "音乐媒体",
        _ => "其他渠道",
    }
}

fn tool_label(key: &str) -> &str {
    match key {
        "query_business_metrics" => "业务数据查询",
        "inspect_collaboration_pipeline" => "合作漏斗分析",
        "search_partners" => "伙伴检索",
        "recommend_plans" => "方案推荐",
        "connect_partner" => "发起合作会话",
        "save_recommended_plan" => "收藏方案",
        "create_follow_up_task" => "创建跟进任务",
        _ => key,
    }
}

fn format_money(cents: i64) -> String {
    format!("¥{:.2}", cents as f64 / 100.0)
}

#[cfg(test)]
mod tests {
    use super::funnel;

    #[test]
    fn funnel_conversion_stays_in_percentage_range() {
        assert_eq!(funnel("建立沟通", 8, 4).conversion, 100);
        assert_eq!(funnel("建立沟通", 0, 0).conversion, 0);
    }
}
