use eatmcp::mcp::protocol::{CallToolResult, JsonRpcRequest, JsonRpcResponse, RequestId};

#[test]
fn test_json_rpc_request_parse() {
    let json_data = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let req: JsonRpcRequest = serde_json::from_str(json_data).expect("Failed to parse request");
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, RequestId::Int(1));
    assert_eq!(req.method, "tools/list");
}

#[test]
fn test_json_rpc_response_serialization() {
    let resp = JsonRpcResponse::success(RequestId::Str("req-123".to_string()), serde_json::json!({ "status": "ok" }));
    let json_str = serde_json::to_string(&resp).expect("Failed to serialize response");
    assert!(json_str.contains(r#""jsonrpc":"2.0""#));
    assert!(json_str.contains(r#""id":"req-123""#));
    assert!(json_str.contains(r#""status":"ok""#));
}

#[test]
fn test_tool_names_are_valid_snake_case() {
    let tools = eatmcp::mcp::tools::list_tools();
    assert!(!tools.is_empty());
    for tool in tools {
        assert!(
            tool.name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "Tool name '{}' must be valid snake_case without dots",
            tool.name
        );
    }
}

