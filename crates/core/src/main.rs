use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use oxiderp_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::{collections::HashMap, env, net::SocketAddr, process};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

const DEMO_PASSWORD: &str = "admin123";

#[derive(Clone)]
struct AppState {
    db: PgPool,
    modules: HashMap<String, ModuleManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventRecord {
    id: Uuid,
    topic: String,
    module_name: String,
    payload: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateEntityRequest {
    data: Value,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if env::args().any(|arg| arg == "--healthcheck") {
        run_healthcheck().await;
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://oxiderp:oxiderp_dev_password@127.0.0.1:5432/oxiderp".to_string());
    let db = PgPoolOptions::new().max_connections(10).connect(&database_url).await?;
    migrate(&db).await?;
    seed(&db).await?;

    let state = AppState {
        db,
        modules: vec![crm_manifest(), sales_manifest(), inventory_manifest(), accounting_manifest()]
            .into_iter()
            .map(|m| (m.name.clone(), m))
            .collect(),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/rust", get(rust_index))
        .route("/logo.svg", get(logo))
        .route("/api/health", get(health))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        .route("/api/modules", get(list_modules))
        .route("/api/modules/:name", get(get_module))
        .route("/api/modules/:name/install", post(install_module))
        .route("/api/modules/:name/uninstall", post(uninstall_module))
        .route("/api/entities/:module/:entity", get(list_entities).post(create_entity))
        .route("/api/entities/:module/:entity/:id", put(update_entity).delete(delete_entity))
        .route("/api/events", get(list_events))
        .route("/api/actions/:module/:action", post(run_action))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let bind = env::var("OXIDERP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let addr: SocketAddr = bind.parse()?;
    tracing::info!(%addr, "OxidERP core server started");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn run_healthcheck() -> ! {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://oxiderp:oxiderp_dev_password@127.0.0.1:5432/oxiderp".to_string());
    match PgPoolOptions::new().max_connections(1).connect(&database_url).await {
        Ok(db) if sqlx::query("SELECT 1").fetch_one(&db).await.is_ok() => process::exit(0),
        _ => process::exit(1),
    }
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../frontend/index.html"))
}

async fn rust_index() -> Html<&'static str> {
    Html(include_str!("../../../frontend/rust-index.html"))
}

async fn logo() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/svg+xml")], include_str!("../../../frontend/logo.svg"))
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let db_ok = sqlx::query("SELECT 1").fetch_one(&state.db).await.is_ok();
    Json(json!({ "status": "ok", "service": "OxidERP", "version": env!("CARGO_PKG_VERSION"), "database": db_ok }))
}

async fn login(State(state): State<AppState>, Json(req): Json<LoginRequest>) -> impl IntoResponse {
    match sqlx::query("SELECT id, email, display_name, password_hash FROM users WHERE email = $1")
        .bind(req.email.to_lowercase())
        .fetch_optional(&state.db)
        .await
    {
        Ok(Some(row)) => {
            let hash: String = row.get("password_hash");
            if hash_password(&req.password) == hash {
                let token = Uuid::new_v4();
                let user_id: Uuid = row.get("id");
                let _ = sqlx::query("INSERT INTO sessions (token, user_id) VALUES ($1, $2)")
                    .bind(token)
                    .bind(user_id)
                    .execute(&state.db)
                    .await;
                return (StatusCode::OK, Json(json!({
                    "token": token,
                    "user": { "id": user_id, "email": row.get::<String,_>("email"), "display_name": row.get::<String,_>("display_name") }
                }))).into_response();
            }
            (StatusCode::UNAUTHORIZED, Json(json!({"error":"invalid credentials"}))).into_response()
        }
        _ => (StatusCode::UNAUTHORIZED, Json(json!({"error":"invalid credentials"}))).into_response(),
    }
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    match auth_user(&state.db, &headers).await {
        Ok(user) => (StatusCode::OK, Json(json!(user))).into_response(),
        Err(resp) => resp,
    }
}

async fn list_modules(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let auth = require_auth(&state.db, &headers).await;
    if let Err(resp) = auth { return resp; }
    let tenant = demo_tenant(&state.db).await;
    let enabled = enabled_modules(&state.db).await.unwrap_or_default();
    let modules: Vec<_> = state.modules.values().cloned().map(|m| json!({
        "installed": enabled.contains(&m.name),
        "manifest": m,
        "name": m.name,
        "title": m.title,
        "version": m.version,
        "category": m.category,
        "summary": m.summary,
        "icon": m.icon,
        "permissions": m.permissions,
        "entities": m.entities,
        "views": m.views,
        "actions": m.actions,
        "events": m.events
    })).collect();
    (StatusCode::OK, Json(json!({ "tenant": tenant, "enabled": enabled, "modules": modules }))).into_response()
}

async fn get_module(Path(name): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    match state.modules.get(&name) {
        Some(module) => (StatusCode::OK, Json(json!(module))).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "module not found"}))).into_response(),
    }
}

async fn install_module(Path(name): Path<String>, State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(resp) = require_auth(&state.db, &headers).await { return resp; }
    if !state.modules.contains_key(&name) {
        return (StatusCode::NOT_FOUND, Json(json!({"error":"module not found"}))).into_response();
    }
    let tenant = demo_tenant(&state.db).await;
    let _ = sqlx::query("INSERT INTO tenant_modules (tenant_id, module_name, installed) VALUES ($1,$2,true) ON CONFLICT (tenant_id,module_name) DO UPDATE SET installed=true, installed_at=now()")
        .bind(tenant.tenant_id).bind(&name).execute(&state.db).await;
    (StatusCode::OK, Json(json!({"ok":true,"installed":name}))).into_response()
}

async fn uninstall_module(Path(name): Path<String>, State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(resp) = require_auth(&state.db, &headers).await { return resp; }
    let tenant = demo_tenant(&state.db).await;
    let _ = sqlx::query("UPDATE tenant_modules SET installed=false WHERE tenant_id=$1 AND module_name=$2")
        .bind(tenant.tenant_id).bind(&name).execute(&state.db).await;
    (StatusCode::OK, Json(json!({"ok":true,"uninstalled":name}))).into_response()
}

async fn list_entities(Path((module, entity)): Path<(String, String)>, State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(resp) = require_auth(&state.db, &headers).await { return resp; }
    let tenant = demo_tenant(&state.db).await;
    let rows = sqlx::query("SELECT id, tenant_id, module_name, entity_type, data, created_at, updated_at FROM entities WHERE tenant_id=$1 AND module_name=$2 AND entity_type=$3 ORDER BY created_at DESC")
        .bind(tenant.tenant_id).bind(module).bind(entity).fetch_all(&state.db).await.unwrap_or_default();
    let records: Vec<EntityRecord> = rows.into_iter().map(|r| EntityRecord { id: r.get("id"), tenant_id: r.get("tenant_id"), module_name: r.get("module_name"), entity_type: r.get("entity_type"), data: r.get("data"), created_at: r.get("created_at"), updated_at: r.get("updated_at") }).collect();
    (StatusCode::OK, Json(json!({ "records": records }))).into_response()
}

async fn create_entity(Path((module, entity)): Path<(String, String)>, State(state): State<AppState>, headers: HeaderMap, Json(req): Json<CreateEntityRequest>) -> impl IntoResponse {
    if let Err(resp) = require_auth(&state.db, &headers).await { return resp; }
    if let Some(resp) = validate_entity_payload(&state, &module, &entity, &req.data) { return resp; }
    let tenant = demo_tenant(&state.db).await;
    let enabled = enabled_modules(&state.db).await.unwrap_or_default();
    if !enabled.contains(&module) { return (StatusCode::BAD_REQUEST, Json(json!({"error":"module is not installed"}))).into_response(); }
    let id = Uuid::new_v4();
    let now = Utc::now();
    let rec = sqlx::query("INSERT INTO entities (id, tenant_id, module_name, entity_type, data, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id, tenant_id, module_name, entity_type, data, created_at, updated_at")
        .bind(id).bind(tenant.tenant_id).bind(&module).bind(&entity).bind(req.data).bind(now).bind(now).fetch_one(&state.db).await;
    match rec {
        Ok(r) => {
            let record = EntityRecord { id: r.get("id"), tenant_id: r.get("tenant_id"), module_name: r.get("module_name"), entity_type: r.get("entity_type"), data: r.get("data"), created_at: r.get("created_at"), updated_at: r.get("updated_at") };
            add_event(&state.db, tenant.tenant_id, format!("{module}.{entity}.created"), module, json!({"entity_id": record.id, "entity_type": entity})).await;
            (StatusCode::CREATED, Json(json!(record))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn update_entity(Path((module, entity, id)): Path<(String, String, Uuid)>, State(state): State<AppState>, headers: HeaderMap, Json(req): Json<CreateEntityRequest>) -> impl IntoResponse {
    if let Err(resp) = require_auth(&state.db, &headers).await { return resp; }
    if let Some(resp) = validate_entity_payload(&state, &module, &entity, &req.data) { return resp; }
    let tenant = demo_tenant(&state.db).await;
    let now = Utc::now();
    let rec = sqlx::query("UPDATE entities SET data=$1, updated_at=$2 WHERE tenant_id=$3 AND module_name=$4 AND entity_type=$5 AND id=$6 RETURNING id, tenant_id, module_name, entity_type, data, created_at, updated_at")
        .bind(req.data).bind(now).bind(tenant.tenant_id).bind(&module).bind(&entity).bind(id).fetch_optional(&state.db).await;
    match rec {
        Ok(Some(r)) => {
            let record = EntityRecord { id: r.get("id"), tenant_id: r.get("tenant_id"), module_name: r.get("module_name"), entity_type: r.get("entity_type"), data: r.get("data"), created_at: r.get("created_at"), updated_at: r.get("updated_at") };
            add_event(&state.db, tenant.tenant_id, format!("{module}.{entity}.updated"), module, json!({"entity_id": record.id, "entity_type": entity})).await;
            (StatusCode::OK, Json(json!(record))).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error":"record not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn delete_entity(Path((module, entity, id)): Path<(String, String, Uuid)>, State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(resp) = require_auth(&state.db, &headers).await { return resp; }
    let tenant = demo_tenant(&state.db).await;
    match sqlx::query("DELETE FROM entities WHERE tenant_id=$1 AND module_name=$2 AND entity_type=$3 AND id=$4")
        .bind(tenant.tenant_id).bind(&module).bind(&entity).bind(id).execute(&state.db).await {
        Ok(done) if done.rows_affected() > 0 => {
            add_event(&state.db, tenant.tenant_id, format!("{module}.{entity}.deleted"), module, json!({"entity_id": id, "entity_type": entity})).await;
            (StatusCode::OK, Json(json!({"ok":true,"deleted":id}))).into_response()
        }
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({"error":"record not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

fn validate_entity_payload(state: &AppState, module: &str, entity: &str, data: &Value) -> Option<axum::response::Response> {
    let manifest = match state.modules.get(module) {
        Some(m) => m,
        None => return Some((StatusCode::NOT_FOUND, Json(json!({"error":"module not found"}))).into_response()),
    };
    let Some(def) = manifest.entities.iter().find(|e| e.name == entity) else {
        return Some((StatusCode::NOT_FOUND, Json(json!({"error":"entity not found"}))).into_response());
    };
    let mut errors = Vec::new();
    for f in &def.fields {
        let value = data.get(&f.name).cloned().unwrap_or(Value::Null);
        if f.required && (value.is_null() || value.as_str().map(|s| s.trim().is_empty()).unwrap_or(false)) {
            errors.push(json!({"field": f.name, "message": format!("{} is required", f.label)}));
        }
        if matches!(f.field_type, FieldType::Select) && !value.is_null() {
            if let Some(s) = value.as_str() {
                if !f.options.is_empty() && !f.options.contains(&s.to_string()) {
                    errors.push(json!({"field": f.name, "message": format!("{} must be one of: {}", f.label, f.options.join(", "))}));
                }
            }
        }
    }
    if module == "accounting" && entity == "journal_entry" {
        let debit = data.get("debit").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()).or_else(|| data.get("debit").and_then(|v| v.as_f64())).unwrap_or(0.0);
        let credit = data.get("credit").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()).or_else(|| data.get("credit").and_then(|v| v.as_f64())).unwrap_or(0.0);
        if (debit - credit).abs() > 0.001 {
            errors.push(json!({"field":"credit", "message":"Accounting entries must balance: debit must equal credit"}));
        }
    }
    if errors.is_empty() { None } else { Some((StatusCode::BAD_REQUEST, Json(json!({"errors": errors}))).into_response()) }
}

async fn list_events(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(resp) = require_auth(&state.db, &headers).await { return resp; }
    let tenant = demo_tenant(&state.db).await;
    let rows = sqlx::query("SELECT id, topic, module_name, payload, created_at FROM events WHERE tenant_id=$1 ORDER BY created_at DESC LIMIT 100")
        .bind(tenant.tenant_id).fetch_all(&state.db).await.unwrap_or_default();
    let events: Vec<EventRecord> = rows.into_iter().map(|r| EventRecord { id: r.get("id"), topic: r.get("topic"), module_name: r.get("module_name"), payload: r.get("payload"), created_at: r.get("created_at") }).collect();
    (StatusCode::OK, Json(json!({ "events": events }))).into_response()
}

async fn run_action(Path((module, action)): Path<(String, String)>, State(state): State<AppState>, headers: HeaderMap, Json(input): Json<Value>) -> impl IntoResponse {
    if let Err(resp) = require_auth(&state.db, &headers).await { return resp; }
    if !state.modules.contains_key(&module) { return (StatusCode::NOT_FOUND, Json(json!({"error":"module not found"}))).into_response(); }
    let tenant = demo_tenant(&state.db).await;
    add_event(&state.db, tenant.tenant_id, format!("{module}.{action}.executed"), module.clone(), input.clone()).await;
    (StatusCode::OK, Json(json!({ "ok": true, "module": module, "action": action, "input": input }))).into_response()
}

async fn require_auth(db: &PgPool, headers: &HeaderMap) -> Result<Uuid, axum::response::Response> {
    auth_user(db, headers).await.map(|u| u.user_id)
}

async fn auth_user(db: &PgPool, headers: &HeaderMap) -> Result<UserContext, axum::response::Response> {
    let token = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "));
    let Some(token) = token else { return Err((StatusCode::UNAUTHORIZED, Json(json!({"error":"login required"}))).into_response()); };
    let Ok(token_uuid) = Uuid::parse_str(token) else { return Err((StatusCode::UNAUTHORIZED, Json(json!({"error":"invalid token"}))).into_response()); };
    let row = sqlx::query("SELECT u.id, u.email, u.display_name FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token=$1")
        .bind(token_uuid).fetch_optional(db).await.map_err(|_| (StatusCode::UNAUTHORIZED, Json(json!({"error":"invalid session"}))).into_response())?;
    match row {
        Some(r) => Ok(UserContext { user_id: r.get("id"), email: r.get("email"), display_name: r.get("display_name"), roles: vec!["admin".into()], permissions: vec!["*".into()] }),
        None => Err((StatusCode::UNAUTHORIZED, Json(json!({"error":"invalid session"}))).into_response()),
    }
}

async fn demo_tenant(db: &PgPool) -> TenantContext {
    let row = sqlx::query("SELECT id, slug FROM tenants WHERE slug='demo-company'").fetch_one(db).await.unwrap();
    TenantContext { tenant_id: row.get("id"), tenant_slug: row.get("slug") }
}

async fn enabled_modules(db: &PgPool) -> anyhow::Result<Vec<String>> {
    let tenant = demo_tenant(db).await;
    let rows = sqlx::query("SELECT module_name FROM tenant_modules WHERE tenant_id=$1 AND installed=true ORDER BY module_name").bind(tenant.tenant_id).fetch_all(db).await?;
    Ok(rows.into_iter().map(|r| r.get("module_name")).collect())
}

async fn add_event(db: &PgPool, tenant_id: Uuid, topic: String, module_name: String, payload: Value) {
    let _ = sqlx::query("INSERT INTO events (id, tenant_id, topic, module_name, payload) VALUES ($1,$2,$3,$4,$5)")
        .bind(Uuid::new_v4()).bind(tenant_id).bind(topic).bind(module_name).bind(payload).execute(db).await;
}

fn hash_password(password: &str) -> String {
    format!("{:x}", Sha256::digest(format!("oxiderp:{password}").as_bytes()))
}

async fn migrate(db: &PgPool) -> anyhow::Result<()> {
    let sql = r#"
CREATE TABLE IF NOT EXISTS tenants (id uuid PRIMARY KEY, slug text UNIQUE NOT NULL, name text NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS users (id uuid PRIMARY KEY, tenant_id uuid NOT NULL REFERENCES tenants(id), email text UNIQUE NOT NULL, display_name text NOT NULL, password_hash text NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS sessions (token uuid PRIMARY KEY, user_id uuid NOT NULL REFERENCES users(id), created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS tenant_modules (tenant_id uuid NOT NULL REFERENCES tenants(id), module_name text NOT NULL, installed bool NOT NULL DEFAULT true, installed_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY (tenant_id, module_name));
CREATE TABLE IF NOT EXISTS entities (id uuid PRIMARY KEY, tenant_id uuid NOT NULL REFERENCES tenants(id), module_name text NOT NULL, entity_type text NOT NULL, data jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now());
CREATE INDEX IF NOT EXISTS idx_entities_tenant_module_entity ON entities (tenant_id, module_name, entity_type);
CREATE TABLE IF NOT EXISTS events (id uuid PRIMARY KEY, tenant_id uuid NOT NULL REFERENCES tenants(id), topic text NOT NULL, module_name text NOT NULL, payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
CREATE INDEX IF NOT EXISTS idx_events_tenant_created ON events (tenant_id, created_at DESC);
"#;
    for stmt in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        sqlx::query(stmt).execute(db).await?;
    }
    Ok(())
}

async fn seed(db: &PgPool) -> anyhow::Result<()> {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1,'demo-company','Demo Company') ON CONFLICT (slug) DO NOTHING")
        .bind(tenant_id).execute(db).await?;
    let tenant = demo_tenant(db).await;
    sqlx::query("INSERT INTO users (id, tenant_id, email, display_name, password_hash) VALUES ($1,$2,'admin@demo.com','Admin', $3) ON CONFLICT (email) DO NOTHING")
        .bind(user_id).bind(tenant.tenant_id).bind(hash_password(DEMO_PASSWORD)).execute(db).await?;
    for m in ["crm", "sales", "inventory", "accounting"] {
        sqlx::query("INSERT INTO tenant_modules (tenant_id, module_name, installed) VALUES ($1,$2,true) ON CONFLICT (tenant_id,module_name) DO NOTHING")
            .bind(tenant.tenant_id).bind(m).execute(db).await?;
    }
    Ok(())
}

fn field(name: &str, label: &str, field_type: FieldType, required: bool, options: &[&str]) -> FieldDefinition {
    FieldDefinition { name: name.into(), label: label.into(), field_type, required, help: None, options: options.iter().map(|s| s.to_string()).collect() }
}

fn crm_manifest() -> ModuleManifest {
    ModuleManifest { name: "crm".into(), title: "CRM".into(), version: "0.1.0".into(), category: ModuleCategory::Crm, summary: "Manage leads, contacts, companies, opportunities, and activities.".into(), icon: "🤝".into(), permissions: vec!["crm.lead.read".into(), "crm.lead.write".into()], entities: vec![EntityDefinition { name: "lead".into(), title: "Lead".into(), description: "Potential customer or opportunity.".into(), fields: vec![field("name", "Lead Name", FieldType::Text, true, &[]), field("email", "Email", FieldType::Email, false, &[]), field("phone", "Phone", FieldType::Phone, false, &[]), field("stage", "Stage", FieldType::Select, true, &["new", "qualified", "proposal", "won", "lost"]), field("expected_revenue", "Expected Revenue", FieldType::Money, false, &[])] }], views: vec![ViewDefinition { name: "lead_list".into(), title: "Leads".into(), entity: "lead".into(), view_type: ViewType::List, fields: vec!["name".into(), "stage".into(), "email".into(), "expected_revenue".into()] }, ViewDefinition { name: "lead_form".into(), title: "Lead Form".into(), entity: "lead".into(), view_type: ViewType::Form, fields: vec!["name".into(), "email".into(), "phone".into(), "stage".into(), "expected_revenue".into()] }], actions: vec![ActionDefinition { name: "convert_lead".into(), label: "Convert Lead".into(), entity: Some("lead".into()), permission: "crm.lead.write".into() }], events: EventManifest { publishes: vec!["crm.lead.created".into()], subscribes: vec![] } }
}
fn sales_manifest() -> ModuleManifest { ModuleManifest { name: "sales".into(), title: "Sales".into(), version: "0.1.0".into(), category: ModuleCategory::Sales, summary: "Quotes, sales orders, customers, and workflows.".into(), icon: "🧾".into(), permissions: vec!["sales.order.read".into(), "sales.order.write".into()], entities: vec![EntityDefinition { name: "order".into(), title: "Sales Order".into(), description: "Customer order.".into(), fields: vec![field("customer", "Customer", FieldType::Text, true, &[]), field("status", "Status", FieldType::Select, true, &["draft", "confirmed", "delivered", "cancelled"]), field("total", "Total", FieldType::Money, true, &[])] }], views: vec![ViewDefinition { name: "order_list".into(), title: "Orders".into(), entity: "order".into(), view_type: ViewType::List, fields: vec!["customer".into(), "status".into(), "total".into()] }], actions: vec![ActionDefinition { name: "confirm_order".into(), label: "Confirm Order".into(), entity: Some("order".into()), permission: "sales.order.write".into() }], events: EventManifest { publishes: vec!["sales.order.confirmed".into()], subscribes: vec![] } } }
fn inventory_manifest() -> ModuleManifest { ModuleManifest { name: "inventory".into(), title: "Inventory".into(), version: "0.1.0".into(), category: ModuleCategory::Inventory, summary: "Products, warehouses, stock moves, and balances.".into(), icon: "📦".into(), permissions: vec!["inventory.product.read".into(), "inventory.product.write".into()], entities: vec![EntityDefinition { name: "product".into(), title: "Product".into(), description: "Sellable or stockable product.".into(), fields: vec![field("sku", "SKU", FieldType::Text, true, &[]), field("name", "Product Name", FieldType::Text, true, &[]), field("quantity", "Quantity", FieldType::Number, true, &[]), field("price", "Price", FieldType::Money, false, &[])] }], views: vec![ViewDefinition { name: "product_list".into(), title: "Products".into(), entity: "product".into(), view_type: ViewType::List, fields: vec!["sku".into(), "name".into(), "quantity".into(), "price".into()] }], actions: vec![ActionDefinition { name: "adjust_stock".into(), label: "Adjust Stock".into(), entity: Some("product".into()), permission: "inventory.product.write".into() }], events: EventManifest { publishes: vec!["inventory.stock.adjusted".into()], subscribes: vec!["sales.order.confirmed".into()] } } }
fn accounting_manifest() -> ModuleManifest { ModuleManifest { name: "accounting".into(), title: "Accounting".into(), version: "0.1.0".into(), category: ModuleCategory::Accounting, summary: "Chart of accounts, journals, and double-entry bookkeeping.".into(), icon: "📚".into(), permissions: vec!["accounting.journal.read".into(), "accounting.journal.write".into()], entities: vec![EntityDefinition { name: "journal_entry".into(), title: "Journal Entry".into(), description: "Balanced debit/credit transaction.".into(), fields: vec![field("reference", "Reference", FieldType::Text, true, &[]), field("date", "Date", FieldType::Date, true, &[]), field("debit", "Debit", FieldType::Money, true, &[]), field("credit", "Credit", FieldType::Money, true, &[])] }], views: vec![ViewDefinition { name: "journal_list".into(), title: "Journal Entries".into(), entity: "journal_entry".into(), view_type: ViewType::List, fields: vec!["reference".into(), "date".into(), "debit".into(), "credit".into()] }], actions: vec![ActionDefinition { name: "post_journal".into(), label: "Post Journal".into(), entity: Some("journal_entry".into()), permission: "accounting.journal.write".into() }], events: EventManifest { publishes: vec!["accounting.journal.posted".into()], subscribes: vec!["sales.order.confirmed".into()] } } }
