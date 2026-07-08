use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use oxiderp_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    inner: Arc<RwLock<Store>>,
}

struct Store {
    tenant: TenantContext,
    modules: HashMap<String, ModuleManifest>,
    enabled_modules: Vec<String>,
    entities: HashMap<String, Vec<EntityRecord>>,
    events: Vec<EventRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventRecord {
    id: Uuid,
    topic: String,
    module_name: String,
    payload: Value,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateEntityRequest {
    data: Value,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let state = AppState {
        inner: Arc::new(RwLock::new(Store::demo())),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/modules", get(list_modules))
        .route("/api/modules/:name", get(get_module))
        .route("/api/entities/:module/:entity", get(list_entities).post(create_entity))
        .route("/api/events", get(list_events))
        .route("/api/actions/:module/:action", post(run_action))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = "0.0.0.0:8080".parse()?;
    tracing::info!(%addr, "OxidERP core server started");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../frontend/index.html"))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "OxidERP", "version": env!("CARGO_PKG_VERSION") }))
}

async fn list_modules(State(state): State<AppState>) -> Json<Value> {
    let store = state.inner.read().await;
    let modules: Vec<_> = store.modules.values().cloned().collect();
    Json(json!({
        "tenant": store.tenant,
        "enabled": store.enabled_modules,
        "modules": modules
    }))
}

async fn get_module(Path(name): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    let store = state.inner.read().await;
    match store.modules.get(&name) {
        Some(module) => (StatusCode::OK, Json(json!(module))).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "module not found"}))).into_response(),
    }
}

async fn list_entities(
    Path((module, entity)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Json<Value> {
    let store = state.inner.read().await;
    let key = format!("{module}.{entity}");
    Json(json!({
        "records": store.entities.get(&key).cloned().unwrap_or_default()
    }))
}

async fn create_entity(
    Path((module, entity)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(req): Json<CreateEntityRequest>,
) -> impl IntoResponse {
    let mut store = state.inner.write().await;
    if !store.enabled_modules.contains(&module) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "module is not enabled"}))).into_response();
    }

    let now = Utc::now();
    let record = EntityRecord {
        id: Uuid::new_v4(),
        tenant_id: store.tenant.tenant_id,
        module_name: module.clone(),
        entity_type: entity.clone(),
        data: req.data,
        created_at: now,
        updated_at: now,
    };
    let key = format!("{module}.{entity}");
    store.entities.entry(key).or_default().push(record.clone());
    store.events.push(EventRecord {
        id: Uuid::new_v4(),
        topic: format!("{module}.{entity}.created"),
        module_name: module,
        payload: json!({ "entity_id": record.id, "entity_type": entity }),
        created_at: now.to_rfc3339(),
    });

    (StatusCode::CREATED, Json(json!(record))).into_response()
}

async fn list_events(State(state): State<AppState>) -> Json<Value> {
    let store = state.inner.read().await;
    Json(json!({ "events": store.events }))
}

async fn run_action(
    Path((module, action)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> impl IntoResponse {
    let mut store = state.inner.write().await;
    if !store.modules.contains_key(&module) {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "module not found"}))).into_response();
    }
    let now = Utc::now();
    store.events.push(EventRecord {
        id: Uuid::new_v4(),
        topic: format!("{module}.{action}.executed"),
        module_name: module.clone(),
        payload: input.clone(),
        created_at: now.to_rfc3339(),
    });
    (StatusCode::OK, Json(json!({ "ok": true, "module": module, "action": action, "input": input }))).into_response()
}

impl Store {
    fn demo() -> Self {
        let tenant = TenantContext {
            tenant_id: Uuid::new_v4(),
            tenant_slug: "demo-company".to_string(),
        };
        let modules = vec![crm_manifest(), sales_manifest(), inventory_manifest(), accounting_manifest()]
            .into_iter()
            .map(|m| (m.name.clone(), m))
            .collect::<HashMap<_, _>>();
        Self {
            tenant,
            modules,
            enabled_modules: vec!["crm".into(), "sales".into(), "inventory".into(), "accounting".into()],
            entities: HashMap::new(),
            events: vec![],
        }
    }
}

fn field(name: &str, label: &str, field_type: FieldType, required: bool, options: &[&str]) -> FieldDefinition {
    FieldDefinition {
        name: name.into(),
        label: label.into(),
        field_type,
        required,
        help: None,
        options: options.iter().map(|s| s.to_string()).collect(),
    }
}

fn crm_manifest() -> ModuleManifest {
    ModuleManifest {
        name: "crm".into(),
        title: "CRM".into(),
        version: "0.1.0".into(),
        category: ModuleCategory::Crm,
        summary: "Manage leads, contacts, companies, opportunities, and activities.".into(),
        icon: "🤝".into(),
        permissions: vec!["crm.lead.read".into(), "crm.lead.write".into()],
        entities: vec![EntityDefinition {
            name: "lead".into(),
            title: "Lead".into(),
            description: "Potential customer or opportunity.".into(),
            fields: vec![
                field("name", "Lead Name", FieldType::Text, true, &[]),
                field("email", "Email", FieldType::Email, false, &[]),
                field("phone", "Phone", FieldType::Phone, false, &[]),
                field("stage", "Stage", FieldType::Select, true, &["new", "qualified", "proposal", "won", "lost"]),
                field("expected_revenue", "Expected Revenue", FieldType::Money, false, &[]),
            ],
        }],
        views: vec![
            ViewDefinition { name: "lead_list".into(), title: "Leads".into(), entity: "lead".into(), view_type: ViewType::List, fields: vec!["name".into(), "stage".into(), "email".into(), "expected_revenue".into()] },
            ViewDefinition { name: "lead_form".into(), title: "Lead Form".into(), entity: "lead".into(), view_type: ViewType::Form, fields: vec!["name".into(), "email".into(), "phone".into(), "stage".into(), "expected_revenue".into()] },
        ],
        actions: vec![ActionDefinition { name: "convert_lead".into(), label: "Convert Lead".into(), entity: Some("lead".into()), permission: "crm.lead.write".into() }],
        events: EventManifest { publishes: vec!["crm.lead.created".into()], subscribes: vec![] },
    }
}

fn sales_manifest() -> ModuleManifest {
    ModuleManifest { name: "sales".into(), title: "Sales".into(), version: "0.1.0".into(), category: ModuleCategory::Sales, summary: "Quotes, sales orders, customers, and workflows.".into(), icon: "🧾".into(), permissions: vec!["sales.order.read".into(), "sales.order.write".into()], entities: vec![EntityDefinition { name: "order".into(), title: "Sales Order".into(), description: "Customer order.".into(), fields: vec![field("customer", "Customer", FieldType::Text, true, &[]), field("status", "Status", FieldType::Select, true, &["draft", "confirmed", "delivered", "cancelled"]), field("total", "Total", FieldType::Money, true, &[])] }], views: vec![ViewDefinition { name: "order_list".into(), title: "Orders".into(), entity: "order".into(), view_type: ViewType::List, fields: vec!["customer".into(), "status".into(), "total".into()] }], actions: vec![ActionDefinition { name: "confirm_order".into(), label: "Confirm Order".into(), entity: Some("order".into()), permission: "sales.order.write".into() }], events: EventManifest { publishes: vec!["sales.order.confirmed".into()], subscribes: vec![] } }
}

fn inventory_manifest() -> ModuleManifest {
    ModuleManifest { name: "inventory".into(), title: "Inventory".into(), version: "0.1.0".into(), category: ModuleCategory::Inventory, summary: "Products, warehouses, stock moves, and balances.".into(), icon: "📦".into(), permissions: vec!["inventory.product.read".into(), "inventory.product.write".into()], entities: vec![EntityDefinition { name: "product".into(), title: "Product".into(), description: "Sellable or stockable product.".into(), fields: vec![field("sku", "SKU", FieldType::Text, true, &[]), field("name", "Product Name", FieldType::Text, true, &[]), field("quantity", "Quantity", FieldType::Number, true, &[]), field("price", "Price", FieldType::Money, false, &[])] }], views: vec![ViewDefinition { name: "product_list".into(), title: "Products".into(), entity: "product".into(), view_type: ViewType::List, fields: vec!["sku".into(), "name".into(), "quantity".into(), "price".into()] }], actions: vec![ActionDefinition { name: "adjust_stock".into(), label: "Adjust Stock".into(), entity: Some("product".into()), permission: "inventory.product.write".into() }], events: EventManifest { publishes: vec!["inventory.stock.adjusted".into()], subscribes: vec!["sales.order.confirmed".into()] } }
}

fn accounting_manifest() -> ModuleManifest {
    ModuleManifest { name: "accounting".into(), title: "Accounting".into(), version: "0.1.0".into(), category: ModuleCategory::Accounting, summary: "Chart of accounts, journals, and double-entry bookkeeping.".into(), icon: "📚".into(), permissions: vec!["accounting.journal.read".into(), "accounting.journal.write".into()], entities: vec![EntityDefinition { name: "journal_entry".into(), title: "Journal Entry".into(), description: "Balanced debit/credit transaction.".into(), fields: vec![field("reference", "Reference", FieldType::Text, true, &[]), field("date", "Date", FieldType::Date, true, &[]), field("debit", "Debit", FieldType::Money, true, &[]), field("credit", "Credit", FieldType::Money, true, &[])] }], views: vec![ViewDefinition { name: "journal_list".into(), title: "Journal Entries".into(), entity: "journal_entry".into(), view_type: ViewType::List, fields: vec!["reference".into(), "date".into(), "debit".into(), "credit".into()] }], actions: vec![ActionDefinition { name: "post_journal".into(), label: "Post Journal".into(), entity: Some("journal_entry".into()), permission: "accounting.journal.write".into() }], events: EventManifest { publishes: vec!["accounting.journal.posted".into()], subscribes: vec!["sales.order.confirmed".into()] } }
}
