use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub type TenantId = Uuid;
pub type UserId = Uuid;
pub type EntityId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub tenant_slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub tenant: TenantContext,
    pub user: UserContext,
    pub request_id: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    pub name: String,
    pub title: String,
    pub version: String,
    pub category: ModuleCategory,
    pub summary: String,
    pub icon: String,
    pub permissions: Vec<String>,
    pub entities: Vec<EntityDefinition>,
    pub views: Vec<ViewDefinition>,
    pub actions: Vec<ActionDefinition>,
    pub events: EventManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleCategory {
    Sales,
    Inventory,
    Accounting,
    Crm,
    HumanResources,
    Productivity,
    Administration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDefinition {
    pub name: String,
    pub title: String,
    pub description: String,
    pub fields: Vec<FieldDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub label: String,
    pub field_type: FieldType,
    pub required: bool,
    pub help: Option<String>,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    LongText,
    Email,
    Phone,
    Number,
    Money,
    Date,
    DateTime,
    Boolean,
    Select,
    Relation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDefinition {
    pub name: String,
    pub title: String,
    pub entity: String,
    pub view_type: ViewType,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewType {
    List,
    Form,
    Kanban,
    Dashboard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDefinition {
    pub name: String,
    pub label: String,
    pub entity: Option<String>,
    pub permission: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventManifest {
    pub publishes: Vec<String>,
    pub subscribes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRecord {
    pub id: EntityId,
    pub tenant_id: TenantId,
    pub module_name: String,
    pub entity_type: String,
    pub data: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ModuleRequest {
    GetManifest,
    GetViews,
    ValidateAction { action: String, entity: Option<EntityRecord>, input: Value },
    ExecuteAction { action: String, entity_id: Option<EntityId>, input: Value },
    HandleEvent { topic: String, payload: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleEnvelope {
    pub context: ExecutionContext,
    pub request: ModuleRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case")]
pub enum ModuleResponse {
    Ok(Value),
    ValidationErrors(Vec<ValidationError>),
    PermissionDenied(String),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: Option<String>,
    pub message: String,
}

pub type OxidResult<T> = Result<T, OxidError>;

#[derive(Debug, thiserror::Error)]
pub enum OxidError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("runtime error: {0}")]
    Runtime(String),
}
