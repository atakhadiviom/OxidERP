use oxiderp_sdk::ModuleResponse;

pub fn module_name() -> &'static str { "inventory" }

pub fn handle_json_request(_input: &str) -> ModuleResponse {
    ModuleResponse::Ok(serde_json::json!({"module":"inventory","message":"Inventory module ready"}))
}
