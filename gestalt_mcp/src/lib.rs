use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::State,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    // Placeholder for Agent/Tools state
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
    pub id: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

pub async fn start_server() -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(AppState {}));

    let app = Router::new()
        .route("/mcp", post(handle_mcp_request))
        .route("/sse", get(sse_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("MCP Server listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_mcp_request(
    State(_state): State<Arc<Mutex<AppState>>>,
    Json(payload): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    // Basic dispatcher
    println!("Received MCP Request: {:?}", payload.method);

    let result = match payload.method.as_str() {
        "initialize" => serde_json::json!({
            "protocolVersion": "0.1.0",
            "serverInfo": {
                "name": "neural-link-mcp",
                "version": "0.1.0"
            }
        }),
        "tools/list" => serde_json::json!({
            "tools": [
                {
                    "name": "echo",
                    "description": "Echoes back the input",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "message": { "type": "string" }
                        }
                    }
                }
            ]
        }),
        "tools/call" => {
            if let Some(params) = &payload.params {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if name == "echo" {
                    let args = params.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
                    let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("hello");
                    serde_json::json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Echo: {}", message)
                            }
                        ]
                    })
                } else {
                     return Json(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Tool not found: {}", name),
                            data: None,
                        }),
                        id: payload.id,
                    })
                }
            } else {
                 return Json(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Missing params".to_string(),
                        data: None,
                    }),
                    id: payload.id,
                })
            }
        },
        _ => return Json(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
            id: payload.id,
        }),
    };

    Json(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id: payload.id,
    })
}

pub async fn start_stdio_server() -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    eprintln!("MCP Stdio Server Started"); // Log to stderr so it doesn't break JSON-RPC

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() { continue; }

        let req: Result<JsonRpcRequest, _> = serde_json::from_str(&line);
        match req {
            Ok(req) => {
                // Manually handle logic since we can't reuse Axum handler directly easily without mock request
                // For MVP reuse logic via refactoring or duplicate simple dispatch
                // Duplicating simple dispatch for now to verify
                let response = process_rpc_request(req).await;
                let resp_str = serde_json::to_string(&response)?;
                stdout.write_all(resp_str.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
            Err(e) => {
                eprintln!("Failed to parse JSON: {}", e);
            }
        }
    }
    Ok(())
}

async fn process_rpc_request(payload: JsonRpcRequest) -> JsonRpcResponse {
     let result = match payload.method.as_str() {
        "initialize" => serde_json::json!({
            "protocolVersion": "0.1.0",
            "serverInfo": {
                "name": "neural-link-mcp",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {}
            }
        }),
        "tools/list" => serde_json::json!({
            "tools": [
                {
                    "name": "echo",
                    "description": "Echoes back the input",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "message": { "type": "string" }
                        }
                    }
                }
            ]
        }),
        "tools/call" => {
             // Logic repeated for Stdio
             if let Some(params) = &payload.params {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if name == "echo" {
                    let args = params.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
                    let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("hello");
                    serde_json::json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Echo: {}", message)
                            }
                        ]
                    })
                } else {
                    return JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Tool not found: {}", name),
                            data: None,
                        }),
                        id: payload.id,
                    };
                }
            } else {
                 return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Missing params".to_string(),
                        data: None,
                    }),
                    id: payload.id,
                };
            }
        }
        _ => return JsonRpcResponse {
             jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
            id: payload.id,
        },
    };

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id: payload.id,
    }
}


async fn sse_handler() -> impl axum::response::IntoResponse {
    "SSE Not implemented yet"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_tool() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "echo",
                "arguments": {
                    "message": "Hello MCP"
                }
            })),
            id: Some(serde_json::json!(1)),
        };

        let resp = process_rpc_request(req).await;

        assert!(resp.error.is_none());
        assert_eq!(resp.id, Some(serde_json::json!(1)));

        let result = resp.result.expect("Result should be present");
        let content = result.get("content").expect("Content should be present");
        let text = content[0].get("text").expect("Text should be present").as_str().expect("Should be string");

        assert_eq!(text, "Echo: Hello MCP");
    }
}

