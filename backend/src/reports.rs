use crate::{admin::require_admin, error::AppResult, state::AppState};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(overview))
        .route("/export", get(export))
}

#[derive(Deserialize)]
struct ReportQuery {
    #[serde(default = "default_days")]
    days: i64,
}

fn default_days() -> i64 {
    30
}

fn clamp_days(days: i64) -> i64 {
    days.clamp(1, 365)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportOverview {
    period: Period,
    user_growth: Vec<DailyCount>,
    active_users: Vec<DailyCount>,
    demand_pipeline: DemandPipeline,
    settlement_summary: SettlementSummary,
    partner_performance: Vec<PartnerPerformance>,
    ai_usage: AiUsage,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Period {
    start: String,
    end: String,
    days: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DailyCount {
    date: String,
    label: String,
    value: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DemandPipeline {
    open: i64,
    following: i64,
    completed: i64,
    closed: i64,
    total_proposals: i64,
    accepted_proposals: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettlementSummary {
    total_count: i64,
    completed_count: i64,
    pending_count: i64,
    completed_amount_cents: i64,
    pending_amount_cents: i64,
    average_amount_cents: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PartnerPerformance {
    id: String,
    name: String,
    partner_type: String,
    match_score: i64,
    conversations: i64,
    proposals: i64,
    revenue_cents: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiUsage {
    sessions: i64,
    messages: i64,
    tool_calls: i64,
    average_tool_calls_per_session: f64,
    average_tool_duration_ms: f64,
    daily_sessions: Vec<DailyCount>,
    daily_tool_calls: Vec<DailyCount>,
    top_tools: Vec<ToolUsage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolUsage {
    name: String,
    value: i64,
    percent: i64,
}

async fn overview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReportQuery>,
) -> AppResult<Json<ReportOverview>> {
    require_admin(&state, &headers).await?;
    let days = clamp_days(query.days);
    let overview = build_overview(&state, days).await?;
    Ok(Json(overview))
}

async fn build_overview(state: &AppState, days: i64) -> AppResult<ReportOverview> {
    let end = Utc::now().date_naive();
    let start = end - Duration::days(days - 1);
    let since = format!("datetime('now', '-{} days')", days);

    let user_growth = daily_counts(
        state,
        &format!(
            "SELECT date(created_at) AS day, COUNT(*) AS total FROM users \
             WHERE created_at >= {since} GROUP BY date(created_at)"
        ),
        start,
        end,
    )
    .await?;

    let active_users = daily_active_users(state, &since, start, end).await?;

    let demand_pipeline = build_demand_pipeline(state).await?;
    let settlement_summary = build_settlement_summary(state).await?;
    let partner_performance = build_partner_performance(state, days).await?;
    let ai_usage = build_ai_usage(state, &since, start, end).await?;

    Ok(ReportOverview {
        period: Period {
            start: start.to_string(),
            end: end.to_string(),
            days,
        },
        user_growth,
        active_users,
        demand_pipeline,
        settlement_summary,
        partner_performance,
        ai_usage,
    })
}

async fn daily_active_users(
    state: &AppState,
    since: &str,
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> AppResult<Vec<DailyCount>> {
    let query = format!(
        "SELECT day, COUNT(DISTINCT user_id) AS total FROM (
            SELECT date(created_at) AS day, user_id FROM match_requests WHERE created_at >= {since}
            UNION ALL
            SELECT date(updated_at) AS day, user_id FROM conversations WHERE updated_at >= {since}
            UNION ALL
            SELECT date(created_at) AS day, user_id FROM agent_sessions WHERE created_at >= {since}
        ) GROUP BY day"
    );
    daily_counts(state, &query, start, end).await
}

async fn daily_counts(
    state: &AppState,
    query: &str,
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> AppResult<Vec<DailyCount>> {
    let rows = sqlx::query(query).fetch_all(&state.pool).await?;
    let mut counts: HashMap<String, i64> = HashMap::new();
    for row in rows {
        let day: String = row.get("day");
        let total: i64 = row.get("total");
        counts.insert(day, total);
    }

    let mut result = Vec::new();
    let mut current = start;
    while current <= end {
        let key = current.to_string();
        result.push(DailyCount {
            date: key.clone(),
            label: current.format("%m/%d").to_string(),
            value: counts.get(&key).copied().unwrap_or_default(),
        });
        current += Duration::days(1);
    }
    Ok(result)
}

async fn build_demand_pipeline(state: &AppState) -> AppResult<DemandPipeline> {
    let match_rows = sqlx::query(
        "SELECT status, COUNT(*) AS total FROM match_requests GROUP BY status",
    )
    .fetch_all(&state.pool)
    .await?;
    let mut demand_counts: HashMap<String, i64> = HashMap::new();
    for row in match_rows {
        let status: String = row.get("status");
        let total: i64 = row.get("total");
        demand_counts.insert(status, total);
    }

    let proposal_rows = sqlx::query(
        "SELECT status, COUNT(*) AS total FROM demand_proposals GROUP BY status",
    )
    .fetch_all(&state.pool)
    .await?;
    let mut proposal_counts: HashMap<String, i64> = HashMap::new();
    for row in proposal_rows {
        let status: String = row.get("status");
        let total: i64 = row.get("total");
        proposal_counts.insert(status, total);
    }

    Ok(DemandPipeline {
        open: demand_counts.get("open").copied().unwrap_or_default(),
        following: demand_counts.get("following").copied().unwrap_or_default(),
        completed: demand_counts.get("completed").copied().unwrap_or_default(),
        closed: demand_counts.get("closed").copied().unwrap_or_default(),
        total_proposals: proposal_counts.values().sum(),
        accepted_proposals: proposal_counts.get("accepted").copied().unwrap_or_default(),
    })
}

async fn build_settlement_summary(state: &AppState) -> AppResult<SettlementSummary> {
    let rows = sqlx::query(
        "SELECT status, COUNT(*) AS total, COALESCE(SUM(amount), 0) AS amount
         FROM settlements GROUP BY status",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut completed_count = 0i64;
    let mut pending_count = 0i64;
    let mut completed_amount = 0i64;
    let mut pending_amount = 0i64;
    let mut total_count = 0i64;
    let mut total_amount = 0i64;

    for row in rows {
        let status: String = row.get("status");
        let count: i64 = row.get("total");
        let amount: i64 = row.get("amount");
        total_count += count;
        total_amount += amount;
        if status == "completed" {
            completed_count = count;
            completed_amount = amount;
        } else if status == "pending" {
            pending_count = count;
            pending_amount = amount;
        }
    }

    let average = if total_count > 0 {
        total_amount / total_count
    } else {
        0
    };

    Ok(SettlementSummary {
        total_count,
        completed_count,
        pending_count,
        completed_amount_cents: completed_amount,
        pending_amount_cents: pending_amount,
        average_amount_cents: average,
    })
}

async fn build_partner_performance(
    state: &AppState,
    days: i64,
) -> AppResult<Vec<PartnerPerformance>> {
    let since = format!("datetime('now', '-{} days')", days);
    let rows = sqlx::query(&format!(
        "SELECT p.id, p.name, p.partner_type, p.match_score,
                COUNT(DISTINCT c.id) AS conversations,
                COUNT(DISTINCT dp.id) AS proposals,
                COALESCE(SUM(s.amount), 0) AS revenue
         FROM partners p
         LEFT JOIN conversations c ON c.partner_id = p.id AND c.updated_at >= {since}
         LEFT JOIN demand_proposals dp ON dp.provider_user_id = p.source_user_id AND dp.created_at >= {since}
         LEFT JOIN settlements s ON s.user_id = p.source_user_id AND s.status = 'completed' AND s.created_at >= {since}
         WHERE p.active = 1
         GROUP BY p.id
         ORDER BY conversations DESC, p.match_score DESC
         LIMIT 50"
    ))
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| PartnerPerformance {
            id: row.get("id"),
            name: row.get("name"),
            partner_type: row.get("partner_type"),
            match_score: row.get("match_score"),
            conversations: row.get("conversations"),
            proposals: row.get("proposals"),
            revenue_cents: row.get("revenue"),
        })
        .collect())
}

async fn build_ai_usage(
    state: &AppState,
    since: &str,
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> AppResult<AiUsage> {
    let sessions: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM agent_sessions WHERE created_at >= {since}"
    ))
    .fetch_one(&state.pool)
    .await?;

    let messages: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM agent_messages WHERE created_at >= {since}"
    ))
    .fetch_one(&state.pool)
    .await?;

    let tool_calls: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM agent_tool_calls WHERE created_at >= {since}"
    ))
    .fetch_one(&state.pool)
    .await?;

    let avg_tool_duration: Option<f64> = sqlx::query_scalar(&format!(
        "SELECT AVG(duration_ms) FROM agent_tool_calls WHERE created_at >= {since}"
    ))
    .fetch_one(&state.pool)
    .await?;

    let daily_sessions = daily_counts(
        state,
        &format!(
            "SELECT date(created_at) AS day, COUNT(*) AS total FROM agent_sessions \
             WHERE created_at >= {since} GROUP BY date(created_at)"
        ),
        start,
        end,
    )
    .await?;

    let daily_tool_calls = daily_counts(
        state,
        &format!(
            "SELECT date(created_at) AS day, COUNT(*) AS total FROM agent_tool_calls \
             WHERE created_at >= {since} GROUP BY date(created_at)"
        ),
        start,
        end,
    )
    .await?;

    let tool_rows = sqlx::query(&format!(
        "SELECT tool_name, COUNT(*) AS total FROM agent_tool_calls \
         WHERE created_at >= {since} GROUP BY tool_name ORDER BY total DESC"
    ))
    .fetch_all(&state.pool)
    .await?;
    let mut top_tools: Vec<ToolUsage> = tool_rows
        .into_iter()
        .map(|row| ToolUsage {
            name: row.get("tool_name"),
            value: row.get("total"),
            percent: 0,
        })
        .collect();
    if !top_tools.is_empty() {
        let total = top_tools.iter().map(|item| item.value).sum::<i64>().max(1);
        for item in &mut top_tools {
            item.percent = ((item.value as f64 / total as f64) * 100.0).round() as i64;
        }
    }

    let avg_per_session = if sessions > 0 {
        tool_calls as f64 / sessions as f64
    } else {
        0.0
    };

    Ok(AiUsage {
        sessions,
        messages,
        tool_calls,
        average_tool_calls_per_session: round_two(avg_per_session),
        average_tool_duration_ms: round_two(avg_tool_duration.unwrap_or(0.0)),
        daily_sessions,
        daily_tool_calls,
        top_tools,
    })
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[derive(Deserialize)]
struct ExportQuery {
    #[serde(rename = "type")]
    report_type: String,
    #[serde(default = "default_days")]
    days: i64,
}

async fn export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ExportQuery>,
) -> AppResult<impl IntoResponse> {
    require_admin(&state, &headers).await?;
    let days = clamp_days(query.days);
    let (filename, content) = match query.report_type.as_str() {
        "user-growth" => export_user_growth(&state, days).await?,
        "demand-pipeline" => export_demand_pipeline(&state).await?,
        "settlements" => export_settlements(&state, days).await?,
        "partner-performance" => export_partner_performance(&state, days).await?,
        "ai-usage" => export_ai_usage(&state, days).await?,
        _ => return Err(crate::error::AppError::BadRequest("unknown report type".into())),
    };

    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/csv; charset=utf-8"));
    let disposition = format!("attachment; filename=\"{}\"", filename);
    response_headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).map_err(|error| crate::error::AppError::Internal(error.into()))?,
    );
    Ok((StatusCode::OK, response_headers, content))
}

fn csv(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut lines = vec![headers.join(",")];
    for row in rows {
        let escaped: Vec<String> = row
            .into_iter()
            .map(|cell| {
                if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                    format!("\"{}\"", cell.replace('"', "\"\""))
                } else {
                    cell
                }
            })
            .collect();
        lines.push(escaped.join(","));
    }
    lines.join("\n")
}

async fn export_user_growth(state: &AppState, days: i64) -> AppResult<(String, String)> {
    let end = Utc::now().date_naive();
    let start = end - Duration::days(days - 1);
    let since = format!("datetime('now', '-{} days')", days);
    let rows = daily_counts(
        state,
        &format!(
            "SELECT date(created_at) AS day, COUNT(*) AS total FROM users \
             WHERE created_at >= {since} GROUP BY date(created_at)"
        ),
        start,
        end,
    )
    .await?;
    let csv_rows: Vec<Vec<String>> = rows
        .into_iter()
        .map(|item| vec![item.date, item.label, item.value.to_string()])
        .collect();
    Ok((format!("user-growth-{}.csv", days), csv(&["日期", "标签", "新增用户"], csv_rows)))
}

async fn export_demand_pipeline(state: &AppState) -> AppResult<(String, String)> {
    let pipeline = build_demand_pipeline(state).await?;
    let rows = vec![
        vec!["open".into(), pipeline.open.to_string()],
        vec!["following".into(), pipeline.following.to_string()],
        vec!["completed".into(), pipeline.completed.to_string()],
        vec!["closed".into(), pipeline.closed.to_string()],
        vec!["total_proposals".into(), pipeline.total_proposals.to_string()],
        vec!["accepted_proposals".into(), pipeline.accepted_proposals.to_string()],
    ];
    Ok(("demand-pipeline.csv".into(), csv(&["阶段", "数量"], rows)))
}

async fn export_settlements(state: &AppState, days: i64) -> AppResult<(String, String)> {
    let since = format!("datetime('now', '-{} days')", days);
    let rows = sqlx::query(&format!(
        "SELECT id, user_id, title, amount, status, created_at \
         FROM settlements WHERE created_at >= {since} ORDER BY created_at DESC"
    ))
    .fetch_all(&state.pool)
    .await?;
    let csv_rows: Vec<Vec<String>> = rows
        .into_iter()
        .map(|row| {
            vec![
                row.get::<String, _>("id"),
                row.get::<String, _>("user_id"),
                row.get::<String, _>("title"),
                format_money(row.get::<i64, _>("amount")),
                row.get::<String, _>("status"),
                row.get::<String, _>("created_at"),
            ]
        })
        .collect();
    Ok((
        format!("settlements-{}.csv", days),
        csv(&["ID", "用户ID", "标题", "金额", "状态", "创建时间"], csv_rows),
    ))
}

async fn export_partner_performance(state: &AppState, days: i64) -> AppResult<(String, String)> {
    let partners = build_partner_performance(state, days).await?;
    let csv_rows: Vec<Vec<String>> = partners
        .into_iter()
        .map(|item| {
            vec![
                item.name,
                item.partner_type,
                item.match_score.to_string(),
                item.conversations.to_string(),
                item.proposals.to_string(),
                format_money(item.revenue_cents),
            ]
        })
        .collect();
    Ok((
        format!("partner-performance-{}.csv", days),
        csv(&["名称", "类型", "匹配分", "会话数", "报价数", "收益"], csv_rows),
    ))
}

async fn export_ai_usage(state: &AppState, days: i64) -> AppResult<(String, String)> {
    let end = Utc::now().date_naive();
    let start = end - Duration::days(days - 1);
    let since = format!("datetime('now', '-{} days')", days);
    let daily_sessions = daily_counts(
        state,
        &format!(
            "SELECT date(created_at) AS day, COUNT(*) AS total FROM agent_sessions \
             WHERE created_at >= {since} GROUP BY date(created_at)"
        ),
        start,
        end,
    )
    .await?;
    let csv_rows: Vec<Vec<String>> = daily_sessions
        .into_iter()
        .map(|item| vec![item.date, item.label, item.value.to_string()])
        .collect();
    Ok((
        format!("ai-usage-{}.csv", days),
        csv(&["日期", "标签", "会话数"], csv_rows),
    ))
}

fn format_money(cents: i64) -> String {
    format!("{:.2}", cents as f64 / 100.0)
}
