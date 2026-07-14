use crate::{
    analytics::track_event,
    api::{current_user, establish_partner_connection},
    error::{AppError, AppResult},
    models::{AgentSettings, AgentTool, UserRow},
    state::AppState,
};
use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
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
    role: String,
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
pub(crate) struct AgentResponse {
    session_id: String,
    message: AgentMessage,
    tool_calls: Vec<ToolCallView>,
    artifacts: Vec<Artifact>,
    suggestions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolDefinition {
    name: String,
    label: String,
    description: String,
    mode: String,
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

struct AgentOrchestration {
    reply: String,
    tool_calls: Vec<ToolCallView>,
    artifacts: Vec<Artifact>,
}

#[derive(Debug)]
struct ModelToolCall {
    id: String,
    name: String,
    raw_arguments: String,
    arguments: Value,
}

#[derive(Debug)]
struct ModelTurn {
    content: Option<String>,
    tool_calls: Vec<ModelToolCall>,
}

#[derive(Clone, Debug)]
struct RuntimeTool {
    name: String,
    label: String,
    description: String,
    mode: String,
    keywords: Vec<String>,
    blocked_keywords: Vec<String>,
    keyword_groups: Vec<Vec<String>>,
    required_tools: Vec<String>,
}

async fn load_runtime_tools(state: &AppState) -> AppResult<Vec<RuntimeTool>> {
    let rows = sqlx::query_as::<_, AgentTool>(
        "SELECT name, enabled, label, description, mode, keywords, blocked_keywords, keyword_groups, required_tools, sort_order
         FROM agent_tools WHERE enabled = 1 ORDER BY sort_order, name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows.into_iter().map(into_runtime_tool).collect())
}

fn into_runtime_tool(row: AgentTool) -> RuntimeTool {
    RuntimeTool {
        name: row.name,
        label: row.label,
        description: row.description,
        mode: row.mode,
        keywords: parse_string_list(&row.keywords),
        blocked_keywords: parse_string_list(&row.blocked_keywords),
        keyword_groups: parse_string_groups(&row.keyword_groups),
        required_tools: parse_string_list(&row.required_tools),
    }
}

fn parse_string_list(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

fn parse_string_groups(json: &str) -> Vec<Vec<String>> {
    serde_json::from_str::<Vec<Vec<String>>>(json).unwrap_or_default()
}

async fn bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<AgentBootstrap>> {
    let user = current_user(&state, &headers).await?;
    let settings = load_agent_settings(&state).await?;
    let session_id = latest_or_create_session(&state, &user, &settings).await?;
    let messages = load_messages(&state, &session_id, settings.max_history).await?;
    let runtime_tools = load_runtime_tools(&state).await?;
    let recent_tool_calls = load_tool_calls(&state, &session_id, &runtime_tools).await?;
    let engine = settings.engine.clone();
    Ok(Json(AgentBootstrap {
        session_id,
        engine,
        role: user.role.clone(),
        messages,
        recent_tool_calls,
        suggestions: suggestions_for_user(&settings, &user.role, false),
        tools: tool_definitions(&runtime_tools),
    }))
}

async fn query(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AgentQuery>,
) -> AppResult<Json<AgentResponse>> {
    let user = current_user(&state, &headers).await?;
    Ok(Json(
        query_for_user(&state, &user, &input.message, input.session_id).await?,
    ))
}

pub(crate) async fn query_for_user(
    state: &AppState,
    user: &UserRow,
    message: &str,
    session_id: Option<String>,
) -> AppResult<AgentResponse> {
    let settings = load_agent_settings(state).await?;
    if !settings.enabled {
        return Err(AppError::BadRequest("agent 已暂停服务".into()));
    }
    let prompt = message.trim();
    if prompt.is_empty() {
        return Err(AppError::BadRequest("agent message is required".into()));
    }
    if prompt.chars().count() > 1000 {
        return Err(AppError::BadRequest("agent message is too long".into()));
    }
    let runtime_tools = load_runtime_tools(state).await?;
    let session_id = match session_id {
        Some(id) => {
            ensure_session_owner(state, &id, &user.id).await?;
            id
        }
        None => latest_or_create_session(state, user, &settings).await?,
    };
    let max_tool_calls = settings.max_tool_calls.max(1);
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

    let orchestration = match run_model_orchestration(
        state,
        user,
        &settings,
        &runtime_tools,
        &session_id,
        &user_message_id,
        prompt,
        max_tool_calls as usize,
    )
    .await?
    {
        Some(orchestration) => orchestration,
        None => {
            run_deterministic_orchestration(
                state,
                user,
                &settings,
                &runtime_tools,
                &session_id,
                &user_message_id,
                prompt,
                max_tool_calls as usize,
            )
            .await?
        }
    };
    let AgentOrchestration {
        reply,
        tool_calls,
        artifacts,
    } = orchestration;
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
    let _ = track_event(
        state,
        Some(&user.id),
        "agent_query_completed",
        Some("agent_session"),
        Some(&session_id),
        json!({ "tools": tool_calls.iter().map(|call| call.name.as_str()).collect::<Vec<_>>() }),
    )
    .await;
    let message = sqlx::query_as::<_, AgentMessage>(
        "SELECT id, role, content, created_at FROM agent_messages WHERE id = ?",
    )
    .bind(assistant_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(AgentResponse {
        session_id,
        message,
        tool_calls,
        artifacts,
        suggestions: suggestions_for_user(&settings, &user.role, true),
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_model_orchestration(
    state: &AppState,
    user: &UserRow,
    settings: &AgentSettings,
    runtime_tools: &[RuntimeTool],
    session_id: &str,
    message_id: &str,
    prompt: &str,
    max_tool_calls: usize,
) -> AppResult<Option<AgentOrchestration>> {
    if state.config.agent_model_api_url.is_none() {
        return Ok(None);
    }

    let history = load_messages(state, session_id, settings.max_history).await?;
    let mut model_messages = vec![json!({
        "role": "system",
        "content": model_system_prompt(settings, user),
    })];
    model_messages.extend(history.into_iter().map(|message| {
        json!({
            "role": message.role,
            "content": message.content,
        })
    }));

    let model_tools = model_tool_definitions(runtime_tools);
    let mut tool_calls = Vec::new();
    let mut artifacts = Vec::new();
    let mut model_turns = 0_usize;

    loop {
        model_turns += 1;
        if model_turns > max_tool_calls.saturating_add(2) {
            return Ok(Some(AgentOrchestration {
                reply: compose_reply(prompt, &tool_calls, settings),
                tool_calls,
                artifacts,
            }));
        }
        let turn = match request_model_turn(state, settings, &model_messages, &model_tools).await {
            Ok(turn) => turn,
            Err(error) => {
                tracing::warn!(error = %error, "agent model request failed; using local fallback");
                if tool_calls.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(AgentOrchestration {
                    reply: compose_reply(prompt, &tool_calls, settings),
                    tool_calls,
                    artifacts,
                }));
            }
        };

        if turn.tool_calls.is_empty() {
            let reply = turn
                .content
                .filter(|content| !content.trim().is_empty())
                .unwrap_or_else(|| compose_reply(prompt, &tool_calls, settings));
            return Ok(Some(AgentOrchestration {
                reply,
                tool_calls,
                artifacts,
            }));
        }

        model_messages.push(model_assistant_message(&turn));
        for requested in turn.tool_calls {
            if tool_calls.len() >= max_tool_calls {
                model_messages.push(json!({
                    "role": "tool",
                    "tool_call_id": requested.id,
                    "name": requested.name,
                    "content": json!({
                        "error": "已达到本轮最大工具调用数，请基于现有结果回答"
                    })
                    .to_string(),
                }));
                continue;
            }

            let runtime_tool = runtime_tools
                .iter()
                .find(|tool| tool.name == requested.name);
            if let Some(runtime_tool) = runtime_tool {
                for required_name in &runtime_tool.required_tools {
                    if tool_calls.len() >= max_tool_calls
                        || tool_calls
                            .iter()
                            .any(|call| call.name == *required_name && call.status == "completed")
                    {
                        continue;
                    }
                    let input = json!({
                        "query": prompt,
                        "source": "required_tool",
                        "requestedBy": requested.name,
                    });
                    let (call, artifact) = execute_recorded_tool(
                        state,
                        user,
                        session_id,
                        message_id,
                        runtime_tools,
                        required_name,
                        prompt,
                        input,
                    )
                    .await?;
                    tool_calls.push(call);
                    artifacts.push(artifact);
                }
            }

            if tool_calls.len() >= max_tool_calls {
                model_messages.push(json!({
                    "role": "tool",
                    "tool_call_id": requested.id,
                    "name": requested.name,
                    "content": json!({
                        "error": "已达到本轮最大工具调用数，请基于现有结果回答"
                    })
                    .to_string(),
                }));
                continue;
            }

            let input = if requested.arguments.is_object() {
                requested.arguments.clone()
            } else {
                json!({ "query": prompt })
            };
            let tool_prompt = requested
                .arguments
                .get("query")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(prompt);

            let (call, artifact) = if runtime_tool
                .is_some_and(|tool| tool.mode == "write" && !tool_matches(tool, prompt))
            {
                persist_failed_tool(
                    state,
                    session_id,
                    message_id,
                    runtime_tools,
                    &requested.name,
                    input,
                    "写入工具仅在用户明确要求执行时运行",
                )
                .await?
            } else {
                execute_recorded_tool(
                    state,
                    user,
                    session_id,
                    message_id,
                    runtime_tools,
                    &requested.name,
                    tool_prompt,
                    input,
                )
                .await?
            };
            let tool_output = call.output.clone();
            tool_calls.push(call);
            artifacts.push(artifact);
            model_messages.push(json!({
                "role": "tool",
                "tool_call_id": requested.id,
                "name": requested.name,
                "content": tool_output.to_string(),
            }));
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_deterministic_orchestration(
    state: &AppState,
    user: &UserRow,
    settings: &AgentSettings,
    runtime_tools: &[RuntimeTool],
    session_id: &str,
    message_id: &str,
    prompt: &str,
    max_tool_calls: usize,
) -> AppResult<AgentOrchestration> {
    let tools = select_tools(runtime_tools, prompt)
        .into_iter()
        .take(max_tool_calls)
        .collect::<Vec<_>>();
    let mut tool_calls = Vec::new();
    let mut artifacts = Vec::new();
    for tool_name in tools {
        let (call, artifact) = execute_recorded_tool(
            state,
            user,
            session_id,
            message_id,
            runtime_tools,
            &tool_name,
            prompt,
            json!({ "query": prompt, "source": "local_fallback" }),
        )
        .await?;
        tool_calls.push(call);
        artifacts.push(artifact);
    }
    Ok(AgentOrchestration {
        reply: compose_reply(prompt, &tool_calls, settings),
        tool_calls,
        artifacts,
    })
}

#[allow(clippy::too_many_arguments)]
async fn execute_recorded_tool(
    state: &AppState,
    user: &UserRow,
    session_id: &str,
    message_id: &str,
    runtime_tools: &[RuntimeTool],
    tool_name: &str,
    prompt: &str,
    input: Value,
) -> AppResult<(ToolCallView, Artifact)> {
    let started = Instant::now();
    match execute_tool(
        state, user, session_id, message_id, tool_name, prompt, &input,
    )
    .await
    {
        Ok(execution) => {
            let call = persist_tool_call(
                state,
                session_id,
                message_id,
                runtime_tools,
                tool_name,
                "completed",
                input,
                execution.output,
                started.elapsed().as_millis() as i64,
            )
            .await?;
            Ok((call, execution.artifact))
        }
        Err(error) => {
            let message = safe_tool_error(&error);
            persist_failed_tool(
                state,
                session_id,
                message_id,
                runtime_tools,
                tool_name,
                input,
                &message,
            )
            .await
        }
    }
}

async fn persist_failed_tool(
    state: &AppState,
    session_id: &str,
    message_id: &str,
    runtime_tools: &[RuntimeTool],
    tool_name: &str,
    input: Value,
    message: &str,
) -> AppResult<(ToolCallView, Artifact)> {
    let output = json!({ "error": message });
    let call = persist_tool_call(
        state,
        session_id,
        message_id,
        runtime_tools,
        tool_name,
        "failed",
        input,
        output,
        0,
    )
    .await?;
    Ok((
        call,
        Artifact {
            kind: "error".into(),
            title: format!("{}未完成", tool_label(runtime_tools, tool_name)),
            summary: message.into(),
            data: json!({ "tool": tool_name }),
        },
    ))
}

fn model_system_prompt(settings: &AgentSettings, user: &UserRow) -> String {
    format!(
        "{}\n\n当前用户身份：{}。所有业务数据必须通过已注册工具实时读取，禁止编造数据库结果。\
         你负责理解意图、选择工具和基于工具结果总结；不使用 RAG、向量检索或第二个模型。\
         每个工具只接受 query 字段。只有用户明确要求执行写入动作时，才可调用 write 工具。\
         当前用户组织：{}。",
        settings.system_prompt, user.role, user.organization
    )
}

fn model_tool_definitions(tools: &[RuntimeTool]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            let parameters = match tool.name.as_str() {
                "search_partners" => json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "用户的合作伙伴检索需求" },
                        "keyword": { "type": "string", "description": "能力、渠道或内容类型关键词" },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 10 }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
                "recommend_plans" => json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "用户的推广目标和约束" },
                        "maxBudgetCents": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "最高预算，单位为人民币分"
                        },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 8 }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
                "connect_partner" => json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "用户明确发起合作的原始要求" },
                        "partnerId": {
                            "type": "string",
                            "description": "优先使用 search_partners 返回的目标伙伴 ID"
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
                "save_recommended_plan" => json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "用户明确收藏方案的原始要求" },
                        "planId": {
                            "type": "string",
                            "description": "优先使用 recommend_plans 返回的目标方案 ID"
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
                "create_follow_up_task" => json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "用户明确创建任务的原始要求" },
                        "title": { "type": "string", "description": "简短、可执行的跟进任务标题" }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
                _ => json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "用于实时业务数据查询的原始需求"
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
            };
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": format!(
                        "{}。模式：{}。结果直接来自业务数据库。",
                        tool.description, tool.mode
                    ),
                    "parameters": parameters
                }
            })
        })
        .collect()
}

fn model_assistant_message(turn: &ModelTurn) -> Value {
    json!({
        "role": "assistant",
        "content": turn.content,
        "tool_calls": turn.tool_calls.iter().map(|call| {
            json!({
                "id": call.id,
                "type": "function",
                "function": {
                    "name": call.name,
                    "arguments": call.raw_arguments,
                }
            })
        }).collect::<Vec<_>>(),
    })
}

async fn request_model_turn(
    state: &AppState,
    settings: &AgentSettings,
    messages: &[Value],
    tools: &[Value],
) -> Result<ModelTurn, String> {
    let url = state
        .config
        .agent_model_api_url
        .as_deref()
        .ok_or_else(|| "model endpoint is not configured".to_string())?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(
            state.config.agent_model_timeout_secs,
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let mut request = client.post(url).json(&json!({
        "model": settings.model,
        "messages": messages,
        "tools": tools,
        "tool_choice": "auto",
        "temperature": settings.temperature,
        "max_tokens": settings.max_tokens,
    }));
    if let Some(api_key) = &state.config.agent_model_api_key {
        request = request.bearer_auth(api_key);
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("model endpoint returned {status}"));
    }
    let body: Value = response.json().await.map_err(|error| error.to_string())?;
    parse_model_turn(&body)
}

fn parse_model_turn(body: &Value) -> Result<ModelTurn, String> {
    let message = body
        .pointer("/choices/0/message")
        .ok_or_else(|| "model response has no message".to_string())?;
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let tool_calls: Vec<ModelToolCall> = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .filter_map(|call| {
                    let id = call.get("id")?.as_str()?.to_owned();
                    let function = call.get("function")?;
                    let name = function.get("name")?.as_str()?.to_owned();
                    let raw_arguments = function
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("{}")
                        .to_owned();
                    let arguments =
                        serde_json::from_str(&raw_arguments).unwrap_or_else(|_| json!({}));
                    Some(ModelToolCall {
                        id,
                        name,
                        raw_arguments,
                        arguments,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if content.is_none() && tool_calls.is_empty() {
        return Err("model response is empty".into());
    }
    Ok(ModelTurn {
        content,
        tool_calls,
    })
}

async fn latest_or_create_session(
    state: &AppState,
    user: &UserRow,
    settings: &AgentSettings,
) -> AppResult<String> {
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM agent_sessions
         WHERE user_id = ? AND status = 'active'
         ORDER BY updated_at DESC, rowid DESC LIMIT 1",
    )
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    let mut transaction = state.pool.begin().await?;
    sqlx::query("INSERT INTO agent_sessions (id, user_id, title) VALUES (?, ?, '新的 Agent 会话')")
        .bind(&id)
        .bind(&user.id)
        .execute(&mut *transaction)
        .await?;
    let welcome = settings
        .welcome_message
        .replace("{organization}", &user.organization);
    sqlx::query(
        "INSERT INTO agent_messages (id, session_id, role, content)
         VALUES (?, ?, 'assistant', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&id)
    .bind(welcome)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
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

async fn load_messages(
    state: &AppState,
    session_id: &str,
    max_history: i64,
) -> AppResult<Vec<AgentMessage>> {
    Ok(sqlx::query_as::<_, AgentMessage>(
        "SELECT id, role, content, created_at FROM agent_messages
         WHERE session_id = ? ORDER BY created_at DESC, rowid DESC LIMIT ?",
    )
    .bind(session_id)
    .bind(max_history)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .rev()
    .collect())
}

async fn load_agent_settings(state: &AppState) -> AppResult<AgentSettings> {
    let row = sqlx::query_as::<_, AgentSettings>(
        "SELECT id, enabled, engine, model, welcome_message, system_prompt, max_tokens, temperature, max_tool_calls, max_history, fallback_reply, suggestion_count, default_suggestions, follow_up_suggestions
         FROM agent_settings WHERE id = 'default'",
    )
    .fetch_optional(&state.pool)
    .await?;
    if let Some(row) = row {
        return Ok(row);
    }
    sqlx::query("INSERT INTO agent_settings (id) VALUES ('default')")
        .execute(&state.pool)
        .await?;
    sqlx::query_as::<_, AgentSettings>(
        "SELECT id, enabled, engine, model, welcome_message, system_prompt, max_tokens, temperature, max_tool_calls, max_history, fallback_reply, suggestion_count, default_suggestions, follow_up_suggestions
         FROM agent_settings WHERE id = 'default'",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| e.into())
}

async fn load_tool_calls(
    state: &AppState,
    session_id: &str,
    runtime_tools: &[RuntimeTool],
) -> AppResult<Vec<ToolCallView>> {
    let rows = sqlx::query(
        "SELECT id, tool_name, input_json, output_json, status, duration_ms
         FROM agent_tool_calls WHERE session_id = ?
         ORDER BY created_at DESC, rowid DESC LIMIT 8",
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
                label: tool_label(runtime_tools, &name),
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
    runtime_tools: &[RuntimeTool],
    tool_name: &str,
    status: &str,
    input: Value,
    output: Value,
    duration_ms: i64,
) -> AppResult<ToolCallView> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_tool_calls
         (id, session_id, message_id, tool_name, status, input_json, output_json, duration_ms)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(session_id)
    .bind(message_id)
    .bind(tool_name)
    .bind(status)
    .bind(input.to_string())
    .bind(output.to_string())
    .bind(duration_ms)
    .execute(&state.pool)
    .await?;
    Ok(ToolCallView {
        id,
        name: tool_name.into(),
        label: tool_label(runtime_tools, tool_name),
        status: status.into(),
        input,
        output,
        duration_ms,
    })
}

fn select_tools(tools: &[RuntimeTool], prompt: &str) -> Vec<String> {
    let mut selected: Vec<String> = Vec::new();
    for tool in tools {
        if !tool_matches(tool, prompt) {
            continue;
        }
        for required in &tool.required_tools {
            if let Some(req) = tools.iter().find(|t| t.name == *required) {
                push_tool_string(&mut selected, &req.name);
            }
        }
        push_tool_string(&mut selected, &tool.name);
    }
    if selected.is_empty() {
        for tool in tools.iter().filter(|t| t.mode == "read") {
            push_tool_string(&mut selected, &tool.name);
        }
    }
    selected
}

fn tool_matches(tool: &RuntimeTool, prompt: &str) -> bool {
    if contains_any(prompt, &tool.blocked_keywords) {
        return false;
    }
    if tool.mode == "read" && contains_any(prompt, &tool.keywords) {
        return true;
    }
    if tool.mode == "read" {
        for group in &tool.keyword_groups {
            if !group.is_empty() && group.iter().all(|keyword| prompt.contains(keyword)) {
                return true;
            }
        }
        return false;
    }
    // Write tools use keyword groups for intent and action_is_blocked for safety.
    for group in &tool.keyword_groups {
        if !group.is_empty() && group.iter().all(|keyword| prompt.contains(keyword)) {
            if action_is_blocked(prompt, &tool.keywords, &tool.blocked_keywords) {
                return false;
            }
            return true;
        }
    }
    false
}

async fn execute_tool(
    state: &AppState,
    user: &UserRow,
    session_id: &str,
    message_id: &str,
    tool_name: &str,
    prompt: &str,
    input: &Value,
) -> AppResult<ToolExecution> {
    match tool_name {
        "query_business_metrics" => query_business_metrics(state, user).await,
        "inspect_collaboration_pipeline" => inspect_pipeline(state, user).await,
        "search_partners" => search_partners(state, user, prompt, input).await,
        "recommend_plans" => recommend_plans(state, prompt, input).await,
        "connect_partner" => connect_partner(state, user, session_id, message_id, input).await,
        "save_recommended_plan" => {
            save_recommended_plan(state, user, session_id, message_id, input).await
        }
        "create_follow_up_task" => {
            create_follow_up_task(state, user, session_id, prompt, input).await
        }
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
            { "label": "可用伙伴池", "value": discovered },
        { "label": "智能匹配", "value": matched },
        { "label": "建立沟通", "value": connected },
        { "label": "完成结算", "value": settled }
    ]);
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "funnel".into(),
            title: "合作转化漏斗".into(),
            summary: if matched > 0 {
                format!(
                    "匹配到沟通转化率 {}%",
                    (connected * 100 / matched.max(1)).clamp(0, 100)
                )
            } else {
                "尚未发起智能匹配".into()
            },
            data: output.clone(),
        },
        output,
    })
}

async fn search_partners(
    state: &AppState,
    user: &UserRow,
    prompt: &str,
    input: &Value,
) -> AppResult<ToolExecution> {
    let partner_type = if user.role == "provider" {
        "client"
    } else {
        "provider"
    };
    let configured_keyword = input
        .get("keyword")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let keyword = configured_keyword.or_else(|| {
        ["短视频", "校园", "品牌", "媒体", "词曲", "混音", "推广"]
            .into_iter()
            .find(|keyword| prompt.contains(keyword))
    });
    let limit = input
        .get("limit")
        .and_then(Value::as_i64)
        .unwrap_or(5)
        .clamp(1, 10);
    let rows = match keyword {
        Some(keyword) => {
            sqlx::query(
                "SELECT id, name, identity, description, tags, match_score
                 FROM partners WHERE active = 1 AND partner_type = ?
                 AND (description LIKE ? OR tags LIKE ?) ORDER BY match_score DESC LIMIT ?",
            )
            .bind(partner_type)
            .bind(format!("%{keyword}%"))
            .bind(format!("%{keyword}%"))
            .bind(limit)
            .fetch_all(&state.pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, name, identity, description, tags, match_score
                 FROM partners WHERE active = 1 AND partner_type = ?
                 ORDER BY match_score DESC LIMIT ?",
            )
            .bind(partner_type)
            .bind(limit)
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
                "tags": parse_json_array(row.get("tags")),
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

async fn recommend_plans(
    state: &AppState,
    prompt: &str,
    input: &Value,
) -> AppResult<ToolExecution> {
    let maximum = input
        .get("maxBudgetCents")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .or_else(|| {
            if prompt.contains("低预算") || prompt.contains("5000") {
                Some(500_000_i64)
            } else if prompt.contains("2万") || prompt.contains("20000") {
                Some(2_000_000_i64)
            } else {
                None
            }
        });
    let limit = input
        .get("limit")
        .and_then(Value::as_i64)
        .unwrap_or(4)
        .clamp(1, 8);
    let rows = if let Some(maximum) = maximum {
        sqlx::query(
            "SELECT id, title, plan_type, description, tags, budget_amount, score
             FROM plans WHERE active = 1 AND budget_amount <= ?
             ORDER BY score DESC LIMIT ?",
        )
        .bind(maximum)
        .bind(limit)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, title, plan_type, description, tags, budget_amount, score
             FROM plans WHERE active = 1 ORDER BY score DESC LIMIT ?",
        )
        .bind(limit)
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
                "tags": parse_json_array(row.get("tags")),
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
    message_id: &str,
    input: &Value,
) -> AppResult<ToolExecution> {
    let plan_id = match input
        .get("planId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(plan_id) => plan_id.to_owned(),
        None => {
            let previous: Option<String> = sqlx::query_scalar(
                "SELECT output_json FROM agent_tool_calls
                 WHERE session_id = ? AND message_id = ? AND tool_name = 'recommend_plans'
                 ORDER BY created_at DESC, rowid DESC LIMIT 1",
            )
            .bind(session_id)
            .bind(message_id)
            .fetch_optional(&state.pool)
            .await?;
            previous
                .as_deref()
                .and_then(|value| serde_json::from_str::<Value>(value).ok())
                .and_then(|value| value["plans"][0]["id"].as_str().map(str::to_owned))
                .ok_or_else(|| AppError::NotFound("当前没有可收藏的推荐方案".into()))?
        }
    };
    let active: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM plans WHERE id = ? AND active = 1)")
            .bind(&plan_id)
            .fetch_one(&state.pool)
            .await?;
    if !active {
        return Err(AppError::NotFound("推荐方案已下架，请重新查询".into()));
    }
    let mut transaction = state.pool.begin().await?;
    let result = sqlx::query(
        "INSERT INTO saved_plans (user_id, plan_id) VALUES (?, ?)
         ON CONFLICT(user_id, plan_id) DO NOTHING",
    )
    .bind(&user.id)
    .bind(&plan_id)
    .execute(&mut *transaction)
    .await?;
    let created = result.rows_affected() == 1;
    if created {
        sqlx::query(
            "INSERT INTO agent_actions
             (id, session_id, user_id, action_type, title, payload)
             VALUES (?, ?, ?, 'save_plan', '收藏推荐方案', ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(&user.id)
        .bind(json!({ "planId": plan_id }).to_string())
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    if created {
        let _ = track_event(
            state,
            Some(&user.id),
            "plan_saved",
            Some("plan"),
            Some(&plan_id),
            json!({ "source": "agent" }),
        )
        .await;
    }
    let output = json!({ "saved": true, "created": created, "planId": plan_id });
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "action".into(),
            title: if created {
                "方案已收藏".into()
            } else {
                "方案已在收藏中".into()
            },
            summary: if created {
                "已写入你的收藏，可随时继续执行。".into()
            } else {
                "无需重复收藏，可继续安排后续执行。".into()
            },
            data: output.clone(),
        },
        output,
    })
}

async fn connect_partner(
    state: &AppState,
    user: &UserRow,
    session_id: &str,
    message_id: &str,
    input: &Value,
) -> AppResult<ToolExecution> {
    let partner_id = match input
        .get("partnerId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(partner_id) => partner_id.to_owned(),
        None => {
            let previous: Option<String> = sqlx::query_scalar(
                "SELECT output_json FROM agent_tool_calls
                 WHERE session_id = ? AND message_id = ? AND tool_name = 'search_partners'
                 ORDER BY created_at DESC, rowid DESC LIMIT 1",
            )
            .bind(session_id)
            .bind(message_id)
            .fetch_optional(&state.pool)
            .await?;
            previous
                .as_deref()
                .and_then(|value| serde_json::from_str::<Value>(value).ok())
                .and_then(|value| value["partners"][0]["id"].as_str().map(str::to_owned))
                .ok_or_else(|| AppError::NotFound("当前没有可联系的匹配伙伴".into()))?
        }
    };
    let connection =
        establish_partner_connection(state, user, &partner_id, Some(session_id)).await?;
    let partner_name = connection.partner_name;
    let stored_id = connection.conversation_id;
    let created = connection.created;
    let output = json!({
        "conversationId": stored_id,
        "partnerId": partner_id,
        "partnerName": partner_name,
        "created": created,
        "status": "completed"
    });
    Ok(ToolExecution {
        artifact: Artifact {
            kind: "action".into(),
            title: if created {
                format!("已联系{partner_name}")
            } else {
                format!("与{partner_name}的会话已存在")
            },
            summary: "可前往消息中心继续沟通合作细节。".into(),
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
    input: &Value,
) -> AppResult<ToolExecution> {
    let title = input
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(short_title)
        .unwrap_or_else(|| short_title(prompt));
    let bucket = Utc::now().timestamp() / 300;
    let dedupe_key = format!("follow_up:{}:{}:{}:{}", session_id, user.id, bucket, title);
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM agent_actions
         WHERE dedupe_key = ?
         ORDER BY created_at DESC, rowid DESC LIMIT 1",
    )
    .bind(&dedupe_key)
    .fetch_optional(&state.pool)
    .await?;
    if let Some(id) = existing {
        return Ok(follow_up_result(id, title, false));
    }
    let id = Uuid::new_v4().to_string();
    let mut transaction = state.pool.begin().await?;
    let result = sqlx::query(
        "INSERT INTO agent_actions
         (id, session_id, user_id, action_type, title, payload, dedupe_key)
         VALUES (?, ?, ?, 'follow_up', ?, ?, ?)
         ON CONFLICT DO NOTHING",
    )
    .bind(&id)
    .bind(session_id)
    .bind(&user.id)
    .bind(&title)
    .bind(json!({ "sourceQuery": prompt }).to_string())
    .bind(&dedupe_key)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() == 0 {
        let existing_id: String =
            sqlx::query_scalar("SELECT id FROM agent_actions WHERE dedupe_key = ?")
                .bind(&dedupe_key)
                .fetch_one(&mut *transaction)
                .await?;
        transaction.commit().await?;
        return Ok(follow_up_result(existing_id, title, false));
    }
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, description)
         VALUES (?, ?, 'spark', 'Agent 跟进任务已创建', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(&title)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(follow_up_result(id, title, true))
}

fn follow_up_result(id: String, title: String, created: bool) -> ToolExecution {
    let output =
        json!({ "actionId": id, "status": "completed", "created": created, "title": title });
    ToolExecution {
        artifact: Artifact {
            kind: "action".into(),
            title: if created {
                "执行任务已创建".into()
            } else {
                "跟进任务已存在".into()
            },
            summary: if created {
                "任务已进入消息中心，后续可继续跟进。".into()
            } else {
                "五分钟内不会重复创建相同任务。".into()
            },
            data: output.clone(),
        },
        output,
    }
}

async fn user_count(state: &AppState, query: &str, user_id: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar(query)
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?)
}

fn compose_reply(prompt: &str, tool_calls: &[ToolCallView], settings: &AgentSettings) -> String {
    let failed = tool_calls
        .iter()
        .filter(|call| call.status == "failed")
        .count();
    let labels = tool_calls
        .iter()
        .map(|call| call.label.as_str())
        .collect::<Vec<_>>()
        .join("、");
    if tool_calls.is_empty() {
        return settings.fallback_reply.clone();
    }
    if failed > 0 {
        return format!(
            "已围绕“{}”执行 {}，其中 {} 个工具未完成。已保留成功结果和失败轨迹，你可以调整条件后继续。",
            short_title(prompt),
            labels,
            failed
        );
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

fn parse_json_array(value: String) -> Value {
    serde_json::from_str::<Vec<Value>>(&value)
        .map(Value::Array)
        .unwrap_or_else(|_| json!([]))
}

fn contains_any(value: &str, keywords: &[String]) -> bool {
    keywords.iter().any(|keyword| value.contains(keyword))
}

fn action_is_blocked(prompt: &str, verbs: &[String], informational: &[String]) -> bool {
    if contains_any(prompt, informational) {
        return true;
    }
    verbs.iter().any(|verb| {
        prompt.match_indices(verb).any(|(index, _)| {
            let prefix = &prompt[..index];
            let recent = prefix
                .chars()
                .rev()
                .take(10)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            let negations: Vec<String> = [
                "不要",
                "别",
                "无需",
                "不必",
                "暂不",
                "先不",
                "请勿",
                "禁止",
                "取消",
                "怎么",
                "如何",
                "能否",
                "是否",
                "可否",
                "该不该",
            ]
            .into_iter()
            .map(String::from)
            .collect();
            contains_any(&recent, &negations) || recent.ends_with('不')
        })
    })
}

fn push_tool_string(tools: &mut Vec<String>, tool: &str) {
    if !tools.iter().any(|t| t == tool) {
        tools.push(tool.into());
    }
}

fn safe_tool_error(error: &AppError) -> String {
    match error {
        AppError::NotFound(message) | AppError::BadRequest(message) => message.clone(),
        AppError::Unauthorized => "登录状态已失效".into(),
        AppError::Database(_) | AppError::Internal(_) => "工具暂时不可用，请稍后重试".into(),
    }
}

fn tool_definitions(tools: &[RuntimeTool]) -> Vec<ToolDefinition> {
    tools
        .iter()
        .map(|tool| ToolDefinition {
            name: tool.name.clone(),
            label: tool.label.clone(),
            description: tool.description.clone(),
            mode: tool.mode.clone(),
        })
        .collect()
}

fn tool_label(tools: &[RuntimeTool], name: &str) -> String {
    tools
        .iter()
        .find(|tool| tool.name == name)
        .map(|tool| tool.label.clone())
        .unwrap_or_else(|| name.into())
}

fn suggestions_for_user(settings: &AgentSettings, role: &str, follow_up: bool) -> Vec<String> {
    let role_suggestions = match (role, follow_up) {
        ("client", false) => vec![
            "分析我的作品推广数据",
            "帮我找短视频推广方",
            "推荐一个适合新作品的推广方案",
        ],
        ("client", true) => vec![
            "继续细分预算和渠道",
            "帮我联系最合适的推广方",
            "为这个方案创建跟进任务",
        ],
        ("provider", false) => vec![
            "分析我的服务与合作数据",
            "帮我找合适的创作者项目",
            "梳理最近的合作转化漏斗",
        ],
        _ => vec![
            "继续分析最有潜力的项目",
            "帮我联系最合适的创作者",
            "为这个合作创建跟进任务",
        ],
    };
    let configured = if follow_up {
        &settings.follow_up_suggestions
    } else {
        &settings.default_suggestions
    };
    role_suggestions
        .into_iter()
        .map(str::to_owned)
        .chain(parse_string_list(configured))
        .take(settings.suggestion_count.max(1) as usize)
        .collect()
}

fn format_money(cents: i64) -> String {
    format!("¥{:.2}", cents as f64 / 100.0)
}

#[cfg(test)]
mod tests {
    use super::{parse_model_turn, select_tools, RuntimeTool};
    use serde_json::json;

    fn test_tool(
        name: &str,
        keywords: &[&str],
        blocked: &[&str],
        groups: &[&[&str]],
        required: &[&str],
        mode: &str,
    ) -> RuntimeTool {
        RuntimeTool {
            name: name.into(),
            label: name.into(),
            description: "".into(),
            mode: mode.into(),
            keywords: keywords.iter().map(|s| (*s).into()).collect(),
            blocked_keywords: blocked.iter().map(|s| (*s).into()).collect(),
            keyword_groups: groups
                .iter()
                .map(|g| g.iter().map(|s| (*s).into()).collect())
                .collect(),
            required_tools: required.iter().map(|s| (*s).into()).collect(),
        }
    }

    fn default_tools() -> Vec<RuntimeTool> {
        vec![
            test_tool(
                "query_business_metrics",
                &["数据", "统计", "趋势", "表现", "多少", "收益"],
                &[],
                &[],
                &[],
                "read",
            ),
            test_tool(
                "inspect_collaboration_pipeline",
                &["漏斗", "转化", "pipeline"],
                &[],
                &[],
                &[],
                "read",
            ),
            test_tool(
                "search_partners",
                &["找", "伙伴", "服务", "需求", "达人", "合作方"],
                &[],
                &[],
                &[],
                "read",
            ),
            test_tool(
                "recommend_plans",
                &["方案", "推广", "预算", "推荐", "投放"],
                &[],
                &[],
                &[],
                "read",
            ),
            test_tool(
                "connect_partner",
                &["联系", "沟通", "合作", "会话", "对接"],
                &[
                    "联系过",
                    "已联系",
                    "联系记录",
                    "联系状态",
                    "沟通数据",
                    "沟通记录",
                ],
                &[
                    &["帮我联系"],
                    &["请联系"],
                    &["直接联系"],
                    &["立即联系"],
                    &["联系最佳"],
                    &["联系最合适"],
                    &["联系第一"],
                    &["联系伙伴"],
                    &["联系合作方"],
                    &["发起合作"],
                    &["开始沟通"],
                    &["立即沟通"],
                    &["和第一位沟通"],
                    &["与第一位沟通"],
                    &["建立会话"],
                    &["帮我对接"],
                    &["直接对接"],
                ],
                &["search_partners"],
                "write",
            ),
            test_tool(
                "save_recommended_plan",
                &["收藏", "保存"],
                &[
                    "取消收藏",
                    "已收藏",
                    "收藏了",
                    "收藏多少",
                    "收藏数据",
                    "收藏记录",
                ],
                &[
                    &["帮我收藏"],
                    &["请收藏"],
                    &["收藏最佳"],
                    &["收藏这个"],
                    &["收藏推荐"],
                    &["收藏方案"],
                    &["保存方案"],
                    &["保存这个"],
                    &["加入收藏"],
                ],
                &["recommend_plans"],
                "write",
            ),
            test_tool(
                "create_follow_up_task",
                &["创建", "生成", "添加", "安排", "提醒", "执行"],
                &["任务记录", "已有任务", "有哪些任务", "执行情况", "执行数据"],
                &[
                    &["提醒我"],
                    &["安排跟进"],
                    &["执行方案"],
                    &["开始执行"],
                    &["直接执行"],
                    &["创建", "任务"],
                    &["创建", "跟进"],
                    &["生成", "任务"],
                    &["生成", "跟进"],
                    &["添加", "任务"],
                    &["添加", "跟进"],
                ],
                &[],
                "write",
            ),
        ]
    }

    fn select(prompt: &str) -> Vec<String> {
        select_tools(&default_tools(), prompt)
    }

    #[test]
    fn selects_query_tools_for_data_questions() {
        assert_eq!(
            select("分析一下最近数据趋势和合作漏斗"),
            vec!["query_business_metrics", "inspect_collaboration_pipeline"]
        );
    }

    #[test]
    fn keeps_read_tools_before_dependent_actions() {
        assert_eq!(
            select("帮我找短视频服务方并联系最佳伙伴"),
            vec!["search_partners", "connect_partner"]
        );
        assert_eq!(
            select("推荐一个推广方案并保存方案"),
            vec!["recommend_plans", "save_recommended_plan"]
        );
        assert_eq!(
            select("直接联系最合适的人"),
            vec!["search_partners", "connect_partner"]
        );
    }

    #[test]
    fn does_not_execute_negated_or_informational_write_intents() {
        assert_eq!(select("帮我找伙伴，但不要联系"), vec!["search_partners"]);
        assert_eq!(select("推荐方案但不要收藏"), vec!["recommend_plans"]);
        assert_eq!(select("如何联系这些合作方"), vec!["search_partners"]);
        assert!(!select("请不要帮我联系任何人").contains(&"connect_partner".into()));
        assert!(!select("分析沟通数据").contains(&"connect_partner".into()));
        assert!(!select("我收藏了几个方案").contains(&"save_recommended_plan".into()));
        assert!(!select("先不保存这个方案").contains(&"save_recommended_plan".into()));
        assert!(!select("有哪些任务").contains(&"create_follow_up_task".into()));
        assert!(select("请创建本周跟进任务").contains(&"create_follow_up_task".into()));
    }

    #[test]
    fn parses_openai_compatible_tool_calls() {
        let turn = parse_model_turn(&json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "search_partners",
                            "arguments": "{\"query\":\"找短视频推广方\"}"
                        }
                    }]
                }
            }]
        }))
        .expect("valid model response");
        assert_eq!(turn.tool_calls.len(), 1);
        assert_eq!(turn.tool_calls[0].name, "search_partners");
        assert_eq!(
            turn.tool_calls[0].arguments["query"],
            json!("找短视频推广方")
        );
    }

    #[test]
    fn rejects_empty_model_responses() {
        let result = parse_model_turn(&json!({
            "choices": [{ "message": { "role": "assistant", "content": null } }]
        }));
        assert!(result.is_err());
    }
}
