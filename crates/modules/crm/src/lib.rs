use oxiderp_sdk::ModuleResponse;

pub fn module_name() -> &'static str { "crm" }

pub fn handle_json_request(_input: &str) -> ModuleResponse {
    ModuleResponse::Ok(serde_json::json!({"module":"crm","message":"CRM module ready"}))
}
