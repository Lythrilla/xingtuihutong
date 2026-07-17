use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Clone, Debug, FromRow)]
pub struct UserRow {
    pub id: String,
    pub display_name: String,
    pub organization: String,
    pub role: String,
    pub avatar: String,
    pub description: String,
    pub verified: bool,
    pub onboarding_status: String,
    pub review_note: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub display_name: String,
    pub organization: String,
    pub role: String,
    pub avatar: String,
    pub description: String,
    pub verified: bool,
    pub onboarding_status: String,
    pub review_note: String,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            display_name: row.display_name,
            organization: row.organization,
            role: row.role,
            avatar: row.avatar,
            description: row.description,
            verified: row.verified,
            onboarding_status: row.onboarding_status,
            review_note: row.review_note,
        }
    }
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Partner {
    pub id: String,
    #[sqlx(rename = "partner_type")]
    pub partner_type: String,
    pub avatar: String,
    pub avatar_class: String,
    pub name: String,
    pub identity: String,
    pub description: String,
    #[sqlx(skip)]
    pub tags: Vec<String>,
    #[serde(skip)]
    pub tags_json: String,
    pub match_score: i64,
    pub result_text: String,
    pub active: bool,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub cover_class: String,
    pub active: bool,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TargetType {
    pub key: String,
    pub icon_class: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BudgetOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub icon_class: String,
    pub color_class: String,
    pub title: String,
    pub plan_type: String,
    pub description: String,
    #[sqlx(skip)]
    pub tags: Vec<String>,
    #[serde(skip)]
    pub tags_json: String,
    pub budget_amount: i64,
    pub score: i64,
    pub active: bool,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub is_read: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Certification {
    pub id: String,
    pub title: String,
    pub icon_class: String,
    pub color_class: String,
    pub status: String,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioCase {
    pub id: String,
    pub case_type: String,
    pub name: String,
    pub result_text: String,
    pub color_class: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSession {
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRole {
    pub role: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMatch {
    pub song_id: String,
    pub target_keys: Vec<String>,
    pub budget_id: String,
    pub goal: String,
    pub cycle: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectPartner {
    pub partner_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SendMessage {
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfile {
    pub organization: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitOnboarding {
    pub entity_name: String,
    pub contact_name: String,
    pub contact_method: String,
    pub description: String,
    pub tags: Vec<String>,
    pub work_title: Option<String>,
    pub work_url: Option<String>,
    pub work_file_url: Option<String>,
    pub work_file_name: Option<String>,
    pub verification_items: Option<Vec<String>>,
    pub audience_size: Option<String>,
    pub cooperation_budget: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOnboarding {
    pub status: String,
    pub review_note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawalRequest {
    pub amount: i64,
}

#[derive(Debug, Deserialize)]
pub struct AdminLogin {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerInput {
    pub partner_type: String,
    pub avatar: String,
    pub avatar_class: String,
    pub name: String,
    pub identity: String,
    pub description: String,
    pub tags: Vec<String>,
    pub match_score: i64,
    pub result_text: String,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongInput {
    pub name: String,
    pub artist: String,
    pub cover_class: String,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInput {
    pub icon_class: String,
    pub color_class: String,
    pub title: String,
    pub plan_type: String,
    pub description: String,
    pub tags: Vec<String>,
    pub budget_amount: i64,
    pub score: i64,
    pub active: bool,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettings {
    pub id: String,
    pub enabled: bool,
    pub engine: String,
    pub model: String,
    pub api_url: String,
    #[serde(skip_serializing)]
    pub api_key: String,
    pub welcome_message: String,
    pub system_prompt: String,
    pub max_tokens: i64,
    pub temperature: f64,
    pub max_tool_calls: i64,
    pub max_history: i64,
    pub fallback_reply: String,
    pub suggestion_count: i64,
    #[serde(serialize_with = "serialize_json_string")]
    pub default_suggestions: String,
    #[serde(serialize_with = "serialize_json_string")]
    pub follow_up_suggestions: String,
}

fn serialize_json_string<S: serde::Serializer>(
    value: &str,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let parsed: serde_json::Value =
        serde_json::from_str(value).unwrap_or_else(|_| serde_json::Value::String(value.to_owned()));
    parsed.serialize(serializer)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettingsInput {
    pub enabled: bool,
    pub engine: String,
    pub model: String,
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    pub welcome_message: String,
    pub system_prompt: String,
    pub max_tokens: i64,
    pub temperature: f64,
    pub max_tool_calls: i64,
    pub max_history: i64,
    pub fallback_reply: String,
    pub suggestion_count: i64,
    pub default_suggestions: Vec<String>,
    pub follow_up_suggestions: Vec<String>,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AgentTool {
    pub name: String,
    pub enabled: bool,
    pub label: String,
    pub description: String,
    pub mode: String,
    #[serde(serialize_with = "serialize_json_string")]
    pub keywords: String,
    #[serde(serialize_with = "serialize_json_string")]
    pub blocked_keywords: String,
    #[serde(serialize_with = "serialize_json_string")]
    pub keyword_groups: String,
    #[serde(serialize_with = "serialize_json_string")]
    pub required_tools: String,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolInput {
    pub name: String,
    pub enabled: bool,
    pub label: String,
    pub description: String,
    pub mode: String,
    pub keywords: Vec<String>,
    pub blocked_keywords: Vec<String>,
    pub keyword_groups: Vec<Vec<String>>,
    pub required_tools: Vec<String>,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetTypeInput {
    pub key: String,
    pub icon_class: String,
    pub title: String,
    pub description: String,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetOptionInput {
    pub id: String,
    pub label: String,
    pub min_amount: Option<i64>,
    pub max_amount: Option<i64>,
    pub sort_order: i64,
}
