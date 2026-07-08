use leptos::*;
use oxiderp_sdk::ModuleManifest;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TenantDto {
    tenant_slug: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ModulesResponse {
    tenant: TenantDto,
    modules: Vec<ModuleManifest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LoginResponse {
    token: String,
}

#[component]
pub fn App() -> impl IntoView {
    let token = create_rw_signal(local_storage_get("oxiderp_token"));
    let tenant = create_rw_signal("demo-company".to_string());
    let modules = create_rw_signal(Vec::<ModuleManifest>::new());
    let error = create_rw_signal(String::new());

    let load_modules = move || {
        let token_value = token.get_untracked();
        spawn_local(async move {
            match api_get::<ModulesResponse>("/api/modules", token_value.as_deref()).await {
                Ok(data) => {
                    tenant.set(data.tenant.tenant_slug);
                    modules.set(data.modules);
                    error.set(String::new());
                }
                Err(err) => error.set(err),
            }
        });
    };

    create_effect(move |_| {
        if token.get().is_some() {
            load_modules();
        }
    });

    view! {
        <main class="rust-app-shell">
            <Show
                when=move || token.get().is_some()
                fallback=move || view! { <Login token=token error=error /> }
            >
                <Topbar tenant=tenant />
                <section class="control">
                    <div>
                        <div class="breadcrumbs">"OxidERP / Rust Frontend"</div>
                        <div class="title">"Apps"</div>
                        <div class="muted">"Leptos/WebAssembly frontend foundation"</div>
                    </div>
                    <div class="actions">
                        <button class="btn purple" on:click=move |_| load_modules()>"Refresh"</button>
                        <button class="btn secondary" on:click=move |_| {
                            local_storage_delete("oxiderp_token");
                            token.set(None);
                        }>"Logout"</button>
                    </div>
                </section>
                <section class="content">
                    <p class="status">{move || error.get()}</p>
                    <div class="app-grid">
                        <For
                            each=move || modules.get()
                            key=|module| module.name.clone()
                            children=move |module| view! { <ModuleCard module=module /> }
                        />
                    </div>
                </section>
            </Show>
        </main>
    }
}

#[component]
fn Login(token: RwSignal<Option<String>>, error: RwSignal<String>) -> impl IntoView {
    let email = create_node_ref::<html::Input>();
    let password = create_node_ref::<html::Input>();

    let submit = move |_| {
        let email_value = email.get().map(|i| i.value()).unwrap_or_default();
        let password_value = password.get().map(|i| i.value()).unwrap_or_default();
        spawn_local(async move {
            match login_request(email_value, password_value).await {
                Ok(response) => {
                    local_storage_set("oxiderp_token", &response.token);
                    token.set(Some(response.token));
                    error.set(String::new());
                }
                Err(err) => error.set(err),
            }
        });
    };

    view! {
        <div class="login">
            <div class="login-card">
                <div style="display:flex;align-items:center;gap:12px">
                    <img src="/logo.svg" width="56" height="56" alt="OxidERP logo" />
                    <h1>"OxidERP"</h1>
                </div>
                <p>"Sign in to your Rust-powered ERP workspace"</p>
                <input node_ref=email value="admin@demo.com" placeholder="Email" />
                <input node_ref=password value="admin123" type="password" placeholder="Password" />
                <button class="btn" on:click=submit>"Sign in"</button>
                <div class="hint">"Demo login: " <b>"admin@demo.com"</b> " / " <b>"admin123"</b></div>
                <p style="color:#b91c1c">{move || error.get()}</p>
            </div>
        </div>
    }
}

#[component]
fn Topbar(tenant: RwSignal<String>) -> impl IntoView {
    view! {
        <header class="topbar">
            <div class="launcher"><span></span><span></span><span></span><span></span><span></span><span></span><span></span><span></span><span></span></div>
            <div class="brand"><img class="brand-logo" src="/logo.svg" alt="OxidERP logo" />"OxidERP"</div>
            <div class="spacer"></div>
            <div class="user"><span>{move || tenant.get()}</span><div class="avatar">"A"</div></div>
        </header>
    }
}

#[component]
fn ModuleCard(module: ModuleManifest) -> impl IntoView {
    view! {
        <article class="app-card">
            <div class="app-icon">{module.icon}</div>
            <div class="app-title">{module.title}</div>
            <div class="app-desc">{module.summary}</div>
            <p><span class="badge">"Rust UI Ready"</span> <span class="status">{format!("v{}", module.version)}</span></p>
        </article>
    }
}

async fn login_request(email: String, password: String) -> Result<LoginResponse, String> {
    let body = serde_json::json!({ "email": email, "password": password }).to_string();
    api_post("/api/auth/login", None, body).await
}

async fn api_get<T: for<'de> Deserialize<'de>>(url: &str, token: Option<&str>) -> Result<T, String> {
    request("GET", url, token, None).await
}

async fn api_post<T: for<'de> Deserialize<'de>>(url: &str, token: Option<&str>, body: String) -> Result<T, String> {
    request("POST", url, token, Some(body)).await
}

async fn request<T: for<'de> Deserialize<'de>>(method: &str, url: &str, token: Option<&str>, body: Option<String>) -> Result<T, String> {
    let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
    let opts = web_sys::RequestInit::new();
    opts.set_method(method);
    opts.set_mode(web_sys::RequestMode::Cors);
    if let Some(body) = body {
        opts.set_body(&wasm_bindgen::JsValue::from_str(&body));
    }
    let request = web_sys::Request::new_with_str_and_init(url, &opts).map_err(|_| "request init failed".to_string())?;
    request.headers().set("content-type", "application/json").map_err(|_| "header failed".to_string())?;
    if let Some(token) = token {
        request.headers().set("authorization", &format!("Bearer {token}")).map_err(|_| "auth header failed".to_string())?;
    }
    let response_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await.map_err(|_| "network error".to_string())?;
    let response: web_sys::Response = response_value.dyn_into().map_err(|_| "invalid response".to_string())?;
    let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(|_| "body read failed".to_string())?).await.map_err(|_| "body await failed".to_string())?;
    let text = text.as_string().unwrap_or_default();
    if !response.ok() {
        return Err(text);
    }
    serde_json::from_str(&text).map_err(|err| format!("json error: {err}"))
}

fn local_storage_get(key: &str) -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item(key).ok()?
}

fn local_storage_set(key: &str, value: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.set_item(key, value);
    }
}

fn local_storage_delete(key: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.remove_item(key);
    }
}

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
