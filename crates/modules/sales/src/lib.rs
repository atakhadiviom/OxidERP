use oxiderp_sdk::ModuleResponse;

pub fn module_name() -> &'static str { "sales" }

pub fn handle_json_request(_input: &str) -> ModuleResponse {
    ModuleResponse::Ok(serde_json::json!({"module":"sales","message":"Sales module ready"}))
}
