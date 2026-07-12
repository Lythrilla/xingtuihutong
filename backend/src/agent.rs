use crate::{
    analytics::track_event,
    api::current_user,
    error::{AppError, AppResult},
    models::UserRow,
    state::AppState,
};
use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, Row};
use std::time::Instant;
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/bootstrap", get(bootstrap))
        .route("/query", post(query))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentQuery {
    session_id: Option<String>,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentBootstrap {
    session_id: String,
    engine: String,
    messages: Vec<AgentMessage>,
    recent_tool_calls: Vec<ToolCallView>,
    suggestions: Vec<String>,
    tools: Vec<ToolDefinition>,
}

#[derive(Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct AgentMessage {
    id: String,
    role: String,
    content: String,
    created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentResponse {
    session_id: String,
    message: AgentMessage,
    tool_calls: Vec<ToolCallView>,
    artifacts: Vec<Artifact>,
    suggestions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolDefinition {
    name: &'static str,
    label: &'static str,
    description: &'static str,
    mode: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCallView {
    id: String,
    name: String,
    label: String,
    status: String,
    input: Value,
    output: Value,
    duration_ms: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    kind: String,
    title: String,
    summary: String,
    data: Value,
}

struct ToolExecution {
    output: Value,
    artifact: Artifact,
}

async fn bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<AgentBootstrap>> {
    let user = current_user(&state, &headers).await?;
    let session_id = latest_or_create_session(&state, &user).await?;
    let messages = load_messages(&state, &session_id).await?;
    let recent_tool_calls = load_tool_calls(&state, &session_id).await?;
    Ok(Json(AgentBootstrap {
        session_id,
        engine: "StarConnect Agent Runtime · Data Grounded".into(),
        messages,
        recent_tool_calls,
        suggestions: default_suggestions(),
        tools: tool_definitions(),
    }))
}

async fn query(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AgentQuery>,
) -> AppResult<Json<AgentResponse>> {
    let user = current_user(&state, &headers).await?;
    let prompt = input.message.trim();
    if prompt.is_empty() {
        return Err(AppError::BadRequest("agent message is required".into()));
    }
    let session_id = match input.session_id {
        Some(id) => {
            ensure_session_owner(&state, &id, &user.id).await?;
            id
        }
        None => latest_or_create_session(&state, &user).await?,
    };
    let user_message_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_messages (id, session_id, role, content)
         VALUES (?, ?, 'user', ?)",
    )
    .bind(&user_message_id)
    .bind(&session_id)
    .bind(prompt)
    .execute(&state.pool)
    .await?;

    let tools = select_tools(prompt);
    let mut tool_calls = Vec::new();
    let mut artifacts = Vec::new();
    for tool_name in tools {
        let started = Instant::now();
        let execution = execute_tool(&state, &user, &session_id, tool_name, prompt).await?;
        let duration_ms = started.elapsed().as_millis() as i64;
        let tool_call = persist_tool_call(
            &state,
            &session_id,
            &user_message_id,
            tool_name,
            json!({ "query": prompt }),
            execution.output.clone(),
            duration_ms,
        )
        .await?;
        tool_calls.push(tool_call);
        artifacts.push(execution.artifact);
    }

    let reply = compose_reply(prompt, &tool_calls);
    let assistant_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_messages (id, session_id, role, content)
         VALUES (?, ?, 'assistant', ?)",
    )
    .bind(&assistant_id)
    .bind(&session_id)
    .bind(&reply)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE agent_sessions SET title = CASE WHEN title = '新的 Agent 会话' THEN ? ELSE title END,
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(short_title(prompt))
    .bind(&session_id)
    .execute(&state.pool)
    .await?;
    track_event(
        &state,
        Some(&user.id),
        "agent_query_completed",
        Some("agent_session"),
        Some(&session_id),
        json!({ "tools": tool_calls.iter().map(|call| call.name.as_str()).collect::<Vec<_>>() }),
    )
    .await?;
    let message = sqlx::query_as::<_, AgentMessage>(
        "SELECT id, role, content, created_at FROM agent_messages WHERE id = ?",
    )
    .bind(assistant_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(AgentResponse {
        session_id,
        message,
        tool_calls,
        artifacts,
        suggestions: follow_up_suggestions(prompt),
    }))
}

async fn latest_or_create_session(state: &AppState, user: &UserRow) -> AppResult<String> {
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM agent_sessions
         WHERE user_id = ? AND status = 'active' ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO agent_sessions (id, user_id, title) VALUES (?, ?, '新的 Agent 会话')")
        .bind(&id)
        .bind(&user.id)
        .execute(&state.pool)
        .await?;
    let welcome = format!(
        "你好，{}。我可以直接查询业务数据、检索合作伙伴、推荐方案，并执行收藏或创建跟进任务。",
        user.organization
    );
    sqlx::query(
        "INSERT INTO agent_messages (id, session_id, role, content)
         VALUES (?, ?, 'assistant', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&id)
    .bind(welcome)
    .execute(&state.pool)
    .await?;
    Ok(id)
}

async fn ensure_session_owner(state: &AppState, session_id: &str, user_id: &str) -> AppResult<()> {
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id = ? AND user_id = ?)",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;
    if !owned {
        return Err(AppError::NotFound("agent session not found".into()));
    }
    Ok(())
}

async fn load_messages(state: &AppState, session_id: &str) -> AppResult<Vec<AgentMessage>> {
    Ok(sqlx::query_as::<_, AgentMessage>(
        "SELECT id, role, content, created_at FROM agent_messages
         WHERE session_id = ? ORDER BY created_at DESC LIMIT 30",
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .rev()
    .collect())
}

async fn load_tool_calls(state: &AppState, session_id: &str) -> AppResult<Vec<ToolCallView>> {
    let rows = sqlx::query(
        "SELECT id, tool_name, input_json, output_json, status, duration_ms
         FROM agent_tool_calls WHERE session_id = ? ORDER BY created_at DESC LIMIT 8",
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let name: String = row.get("tool_name");
            ToolCallView {
                id: row.get("id"),
                label: tool_label(&name).into(),
                name,
                status: row.get("status"),
                input: parse_json(row.get("input_json")),
                output: parse_json(row.get("output_json")),
                duration_ms: row.get("duration_ms"),
            }
        })
        .collect())
}

async fn persist_tool_call(
    state: &AppState,
    session_id: &str,
    message_id: &str,
    tool_name: &str,
    input: Value,
    output: Value,
    duration_ms: i64,
) -> AppResult<ToolCallView> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_tool_calls
         (id, session_id, message_id, tool_name, input_json, output_json, duration_ms)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(session_id)
    .bind(message_id)
    .bind(tool_name)
    .bind(input.to_string())
    .bind(output.to_string())
    .bind(duration_ms)
    .execute(&state.pool)
    .await?;
    Ok(ToolCallView {
        id,
        name: tool_name.into(),
        label: tool_label(tool_name).into(),
        status: "completed".into(),
        input,
        output,
        duration_ms,
    })
}

fn select_tools(prompt: &str) -> Vec<&'static str> {
    let mut tools = Vec::new();
    if contains_any(prompt, &["数据", "统计", "趋势", "表现", "多少", "收益"]) {
        tools.push("query_business_metrics");
        tools.push("inspect_collaboration_pipeline");
    }
    if contains_any(prompt, &["找", "伙伴", "服务", "需求", "达人", "合作方"]) {
        tools.push("search_partners");
    }
    if contains_any(prompt, &["方案", "推广", "预算", "推荐", "投放"]) {
        tools.push("recommend_plans");
    }
    if contains_any(prompt, &["联系", "沟通", "发起合作", "对接"]) {
        tools.push("connect_partner");
    }
    if contains_any(prompt, &["收藏", "保存方案"]) {
        tools.push("save_recommended_plan");
    }
    if contains_any(prompt, &["执行", "跟进", "提醒", "任务"]) {
        tools.push("create_follow_up_task");
    }
    if tools.is_empty() {
        tools.extend([
            "query_business_metrics",
            "search_partners",
            "recommend_plans",
        ]);
    }
    tools.dedup();
    tools
}

async fn execute_tool(
    state: &AppState,
    user: &UserRow,
    session_id: &str,
    tool_name: &str,
    prompt: &str,
) -> AppResult<ToolExecution> {
    match tool_name {
        "query_business_metrics" => query_business_metrics(state, user).await,
        "inspect_collaboration_pipeline" => inspect_pipeline(state, user).await,
        "search_partners" => search_partners(state, prompt).await,
        "recommend_plans" => recommend_plans(state, prompt).await,
        "connect_partner" => connect_partner(state, user, session_id).await,
        "save_recommended_plan" => save_recommended_plan(state, user, session_id).await,
        "create_follow_up_task" => create_follow_up_task(state, user, session_id, prompt).await,
        _ => Err(AppError::BadRequest("unknown agent tool".into())),
    }
}

async fn query_business_metrics(state: &AppState, user: &UserRow) -> AppResult<ToolExecution> {
    let matches = user_count(
        state,
        "SELECT COUNT(*) FROM match_requests WHERE user_id = ?",
        &user.id,
    )
    .await?;
    let conversations = user_count(
        state,
        "SELECT COUNT(*) FROM conversations WHERE user_id = ?",
        &user.id,
    )
    .await?;
    let saved = user_count(
        state,
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
    let output = json!({
        "matches": matches,
        "conversations": conversations,
        "savedPlans": saved,
        "revenue": revenue,
        "revenueDisplay": format_money(revenue)
    });
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "metrics".into(),
            title: "业务实时指标".into(),
            summary: format!(
                "{} 次匹配 · {} 个会话 · {}",
                matches,
                conversations,
                format_money(revenue)
            ),
            data: output.clone(),
        },
        output,
    })
}

async fn inspect_pipeline(state: &AppState, user: &UserRow) -> AppResult<ToolExecution> {
    let discovered: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM partners WHERE active = 1")
        .fetch_one(&state.pool)
        .await?;
    let matched = user_count(
        state,
        "SELECT COUNT(*) FROM match_requests WHERE user_id = ?",
        &user.id,
    )
    .await?;
    let connected = user_count(
        state,
        "SELECT COUNT(*) FROM conversations WHERE user_id = ?",
        &user.id,
    )
    .await?;
    let settled = user_count(
        state,
        "SELECT COUNT(*) FROM settlements WHERE user_id = ? AND status = 'completed'",
        &user.id,
    )
    .await?;
    let output = json!([
        { "label": "发现伙伴", "value": discovered },
        { "label": "智能匹配", "value": matched },
        { "label": "建立沟通", "value": connected },
        { "label": "完成结算", "value": settled }
    ]);
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "funnel".into(),
            title: "合作转化漏斗".into(),
            summary: if matched > 0 {
                format!("匹配到沟通转化率 {}%", connected * 100 / matched.max(1))
            } else {
                "尚未发起智能匹配".into()
            },
            data: output.clone(),
        },
        output,
    })
}

async fn search_partners(state: &AppState, prompt: &str) -> AppResult<ToolExecution> {
    let partner_type = if prompt.contains("需求") {
        Some("client")
    } else if prompt.contains("服务") {
        Some("provider")
    } else {
        None
    };
    let keyword = ["短视频", "校园", "品牌", "媒体", "词曲", "混音", "推广"]
        .into_iter()
        .find(|keyword| prompt.contains(keyword));
    let rows = match (partner_type, keyword) {
        (Some(kind), Some(keyword)) => {
            sqlx::query(
                "SELECT id, name, identity, description, tags, match_score
                 FROM partners WHERE active = 1 AND partner_type = ?
                 AND (description LIKE ? OR tags LIKE ?) ORDER BY match_score DESC LIMIT 5",
            )
            .bind(kind)
            .bind(format!("%{keyword}%"))
            .bind(format!("%{keyword}%"))
            .fetch_all(&state.pool)
            .await?
        }
        (Some(kind), None) => {
            sqlx::query(
                "SELECT id, name, identity, description, tags, match_score
                 FROM partners WHERE active = 1 AND partner_type = ?
                 ORDER BY match_score DESC LIMIT 5",
            )
            .bind(kind)
            .fetch_all(&state.pool)
            .await?
        }
        (None, Some(keyword)) => {
            sqlx::query(
                "SELECT id, name, identity, description, tags, match_score
                 FROM partners WHERE active = 1 AND (description LIKE ? OR tags LIKE ?)
                 ORDER BY match_score DESC LIMIT 5",
            )
            .bind(format!("%{keyword}%"))
            .bind(format!("%{keyword}%"))
            .fetch_all(&state.pool)
            .await?
        }
        (None, None) => {
            sqlx::query(
                "SELECT id, name, identity, description, tags, match_score
                 FROM partners WHERE active = 1 ORDER BY match_score DESC LIMIT 5",
            )
            .fetch_all(&state.pool)
            .await?
        }
    };
    let partners: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.get::<String, _>("id"),
                "name": row.get::<String, _>("name"),
                "identity": row.get::<String, _>("identity"),
                "description": row.get::<String, _>("description"),
                "tags": parse_json(row.get("tags")),
                "matchScore": row.get::<i64, _>("match_score")
            })
        })
        .collect();
    let total = partners.len();
    let output = json!({ "partners": partners, "total": total });
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "partners".into(),
            title: "候选合作伙伴".into(),
            summary: format!("找到 {} 位高匹配伙伴", total),
            data: output.clone(),
        },
        output,
    })
}

async fn recommend_plans(state: &AppState, prompt: &str) -> AppResult<ToolExecution> {
    let maximum = if prompt.contains("低预算") || prompt.contains("5000") {
        Some(500_000_i64)
    } else if prompt.contains("2万") || prompt.contains("20000") {
        Some(2_000_000_i64)
    } else {
        None
    };
    let rows = if let Some(maximum) = maximum {
        sqlx::query(
            "SELECT id, title, plan_type, description, tags, budget_amount, score
             FROM plans WHERE active = 1 AND budget_amount <= ?
             ORDER BY score DESC LIMIT 4",
        )
        .bind(maximum)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, title, plan_type, description, tags, budget_amount, score
             FROM plans WHERE active = 1 ORDER BY score DESC LIMIT 4",
        )
        .fetch_all(&state.pool)
        .await?
    };
    let plans: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            let budget: i64 = row.get("budget_amount");
            json!({
                "id": row.get::<String, _>("id"),
                "title": row.get::<String, _>("title"),
                "planType": row.get::<String, _>("plan_type"),
                "description": row.get::<String, _>("description"),
                "tags": parse_json(row.get("tags")),
                "budgetAmount": budget,
                "budget": format_money(budget),
                "score": row.get::<i64, _>("score")
            })
        })
        .collect();
    let total = plans.len();
    let output = json!({ "plans": plans, "total": total });
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "plans".into(),
            title: "Agent 推荐方案".into(),
            summary: format!("生成 {} 个数据驱动方案", total),
            data: output.clone(),
        },
        output,
    })
}

async fn save_recommended_plan(
    state: &AppState,
    user: &UserRow,
    session_id: &str,
) -> AppResult<ToolExecution> {
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT output_json FROM agent_tool_calls
         WHERE session_id = ? AND tool_name = 'recommend_plans'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&state.pool)
    .await?;
    let plan_id = previous
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value["plans"][0]["id"].as_str().map(str::to_owned))
        .or(
            sqlx::query_scalar("SELECT id FROM plans WHERE active = 1 ORDER BY score DESC LIMIT 1")
                .fetch_optional(&state.pool)
                .await?,
        )
        .ok_or_else(|| AppError::NotFound("plan not found".into()))?;
    sqlx::query(
        "INSERT INTO saved_plans (user_id, plan_id) VALUES (?, ?)
         ON CONFLICT(user_id, plan_id) DO NOTHING",
    )
    .bind(&user.id)
    .bind(&plan_id)
    .execute(&state.pool)
    .await?;
    let output = json!({ "saved": true, "planId": plan_id });
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "action".into(),
            title: "方案已收藏".into(),
            summary: "已写入你的收藏，可随时继续执行。".into(),
            data: output.clone(),
        },
        output,
    })
}

async fn connect_partner(
    state: &AppState,
    user: &UserRow,
    session_id: &str,
) -> AppResult<ToolExecution> {
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT output_json FROM agent_tool_calls
         WHERE session_id = ? AND tool_name = 'search_partners'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&state.pool)
    .await?;
    let partner_id = previous
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value["partners"][0]["id"].as_str().map(str::to_owned))
        .or(sqlx::query_scalar(
            "SELECT id FROM partners WHERE active = 1 ORDER BY match_score DESC LIMIT 1",
        )
        .fetch_optional(&state.pool)
        .await?)
        .ok_or_else(|| AppError::NotFound("partner not found".into()))?;
    let partner_name: String = sqlx::query_scalar("SELECT name FROM partners WHERE id = ?")
        .bind(&partner_id)
        .fetch_one(&state.pool)
        .await?;
    let conversation_id = Uuid::new_v4().to_string();
    let welcome_message = "你好，Agent 已根据合作目标为我们建立会话，可以开始沟通细节。";
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO conversations
         (id, user_id, partner_id, last_message, unread_count)
         VALUES (?, ?, ?, ?, 1)
         ON CONFLICT(user_id, partner_id) DO NOTHING",
    )
    .bind(&conversation_id)
    .bind(&user.id)
    .bind(&partner_id)
    .bind(welcome_message)
    .execute(&mut *transaction)
    .await?;
    let stored_id: String =
        sqlx::query_scalar("SELECT id FROM conversations WHERE user_id = ? AND partner_id = ?")
            .bind(&user.id)
            .bind(&partner_id)
            .fetch_one(&mut *transaction)
            .await?;
    let message_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM conversation_messages
           WHERE conversation_id = ? AND content = ?
         )",
    )
    .bind(&stored_id)
    .bind(welcome_message)
    .fetch_one(&mut *transaction)
    .await?;
    if !message_exists {
        sqlx::query(
            "INSERT INTO conversation_messages (id, conversation_id, sender, content)
             VALUES (?, ?, 'partner', ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&stored_id)
        .bind(welcome_message)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    let output = json!({
        "conversationId": stored_id,
        "partnerId": partner_id,
        "partnerName": partner_name,
        "status": "completed"
    });
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "action".into(),
            title: format!("已联系{partner_name}"),
            summary: "合作会话已建立，可前往消息中心继续沟通。".into(),
            data: output.clone(),
        },
        output,
    })
}

async fn create_follow_up_task(
    state: &AppState,
    user: &UserRow,
    session_id: &str,
    prompt: &str,
) -> AppResult<ToolExecution> {
    let id = Uuid::new_v4().to_string();
    let title = short_title(prompt);
    sqlx::query(
        "INSERT INTO agent_actions
         (id, session_id, user_id, action_type, title, payload)
         VALUES (?, ?, ?, 'follow_up', ?, ?)",
    )
    .bind(&id)
    .bind(session_id)
    .bind(&user.id)
    .bind(&title)
    .bind(json!({ "sourceQuery": prompt }).to_string())
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, description)
         VALUES (?, ?, 'spark', 'Agent 跟进任务已创建', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(&title)
    .execute(&state.pool)
    .await?;
    let output = json!({ "actionId": id, "status": "completed", "title": title });
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "action".into(),
            title: "执行任务已创建".into(),
            summary: "任务已进入消息中心，后续可继续跟进。".into(),
            data: output.clone(),
        },
        output,
    })
}

async fn user_count(state: &AppState, query: &str, user_id: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar(query)
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?)
}

fn compose_reply(prompt: &str, tool_calls: &[ToolCallView]) -> String {
    let labels = tool_calls
        .iter()
        .map(|call| call.label.as_str())
        .collect::<Vec<_>>()
        .join("、");
    if tool_calls.is_empty() {
        return "我暂时没有找到需要调用的工具，请换一种方式描述目标。".into();
    }
    format!(
        "已围绕“{}”完成 {}。结果来自当前业务数据库，你可以继续追问细分渠道、预算或让我直接创建跟进任务。",
        short_title(prompt),
        labels
    )
}

fn short_title(value: &str) -> String {
    value.chars().take(24).collect()
}

fn parse_json(value: String) -> Value {
    serde_json::from_str(&value).unwrap_or_else(|_| json!({}))
}

fn contains_any(value: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| value.contains(keyword))
}

fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        tool(
            "query_business_metrics",
            "业务数据查询",
            "读取实时匹配、会话、收藏与收益",
            "read",
        ),
        tool(
            "inspect_collaboration_pipeline",
            "合作漏斗分析",
            "分析发现、匹配、沟通与结算转化",
            "read",
        ),
        tool(
            "search_partners",
            "合作伙伴检索",
            "按身份、能力和匹配度查询伙伴",
            "read",
        ),
        tool(
            "recommend_plans",
            "推广方案推荐",
            "结合预算与目标查询可执行方案",
            "read",
        ),
        tool(
            "connect_partner",
            "发起合作会话",
            "连接检索结果中的最佳伙伴并建立会话",
            "write",
        ),
        tool(
            "save_recommended_plan",
            "收藏推荐方案",
            "将推荐方案写入用户收藏",
            "write",
        ),
        tool(
            "create_follow_up_task",
            "创建跟进任务",
            "生成任务并发送到消息中心",
            "write",
        ),
    ]
}

fn tool(
    name: &'static str,
    label: &'static str,
    description: &'static str,
    mode: &'static str,
) -> ToolDefinition {
    ToolDefinition {
        name,
        label,
        description,
        mode,
    }
}

fn tool_label(name: &str) -> &str {
    match name {
        "query_business_metrics" => "业务数据查询",
        "inspect_collaboration_pipeline" => "合作漏斗分析",
        "search_partners" => "伙伴检索",
        "recommend_plans" => "方案推荐",
        "connect_partner" => "发起合作会话",
        "save_recommended_plan" => "收藏方案",
        "create_follow_up_task" => "创建跟进任务",
        _ => name,
    }
}

fn default_suggestions() -> Vec<String> {
    vec![
        "查询我最近 7 天的合作数据".into(),
        "帮我找短视频推广服务方".into(),
        "推荐 2 万元内的推广方案".into(),
        "分析当前合作转化漏斗".into(),
    ]
}

fn follow_up_suggestions(prompt: &str) -> Vec<String> {
    if contains_any(prompt, &["数据", "趋势", "漏斗"]) {
        vec![
            "找出最值得提升的转化环节".into(),
            "推荐适合当前数据表现的方案".into(),
            "创建本周跟进任务".into(),
        ]
    } else {
        vec![
            "查询这些伙伴的匹配依据".into(),
            "推荐可执行的推广方案".into(),
            "收藏最佳方案".into(),
        ]
    }
}

fn format_money(cents: i64) -> String {
    format!("¥{:.2}", cents as f64 / 100.0)
}

#[cfg(test)]
mod tests {
    use super::select_tools;

    #[test]
    fn selects_query_tools_for_data_questions() {
        assert_eq!(
            select_tools("分析一下最近数据趋势和合作漏斗"),
            vec!["query_business_metrics", "inspect_collaboration_pipeline"]
        );
    }

    #[test]
    fn keeps_read_tools_before_dependent_actions() {
        assert_eq!(
            select_tools("帮我找短视频服务方并联系最佳伙伴"),
            vec!["search_partners", "connect_partner"]
        );
        assert_eq!(
            select_tools("推荐一个推广方案并保存方案"),
            vec!["recommend_plans", "save_recommended_plan"]
        );
    }
}
