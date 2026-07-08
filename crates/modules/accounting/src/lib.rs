use oxiderp_sdk::ModuleResponse;

pub fn module_name() -> &'static str { "accounting" }

pub fn handle_json_request(_input: &str) -> ModuleResponse {
    ModuleResponse::Ok(serde_json::json!({"module":"accounting","message":"Accounting module ready"}))
}
