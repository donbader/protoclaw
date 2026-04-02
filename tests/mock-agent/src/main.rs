use serde_json::{json, Value};
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

static PROMPT_COUNT: AtomicUsize = AtomicUsize::new(0);

#[tokio::main]
async fn main() {
    let exit_after: Option<usize> = env::var("MOCK_AGENT_EXIT_AFTER")
        .ok()
        .and_then(|v| v.parse().ok());
    let delay_ms: Option<u64> = env::var("MOCK_AGENT_DELAY_MS")
        .ok()
        .and_then(|v| v.parse().ok());
    let request_permission = env::var("MOCK_AGENT_REQUEST_PERMISSION").ok().is_some_and(|v| v == "1");
    let reject_load = env::var("MOCK_AGENT_REJECT_LOAD").ok().is_some_and(|v| v == "1");

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    let mut session_id: Option<String> = None;

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = msg["method"].as_str().unwrap_or("");
        let id = msg.get("id").cloned();

        if let Some(ms) = delay_ms {
            tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
        }

        match method {
            "initialize" => {
                handle_initialize(&mut stdout, id).await;
            }
            "session/new" => {
                session_id = Some(handle_session_new(&mut stdout, id).await);
            }
            "session/prompt" => {
                let sid = session_id.clone().unwrap_or_else(|| "unknown".to_string());
                let user_msg = extract_prompt_message(&msg);
                handle_session_prompt(
                    &mut stdout,
                    id,
                    &sid,
                    &user_msg,
                    request_permission,
                    &mut lines,
                )
                .await;

                let count = PROMPT_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
                if let Some(limit) = exit_after {
                    if count >= limit {
                        std::process::exit(1);
                    }
                }
            }
            "session/cancel" => {
                handle_session_cancel(&mut stdout, id).await;
            }
            "session/load" => {
                handle_session_load(&mut stdout, id, reject_load, &mut session_id).await;
            }
            "session/close" => {
                handle_session_close(&mut stdout, id).await;
                break;
            }
            _ => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": "Method not found" }
                });
                write_message(&mut stdout, &resp).await;
            }
        }
    }
}

async fn write_message(stdout: &mut tokio::io::Stdout, msg: &Value) {
    let mut line = serde_json::to_string(msg).expect("failed to serialize");
    line.push('\n');
    stdout.write_all(line.as_bytes()).await.expect("failed to write");
    stdout.flush().await.expect("failed to flush");
}

fn extract_prompt_message(msg: &Value) -> String {
    msg["params"]["prompt"]
        .as_str()
        .or_else(|| msg["params"]["message"].as_str())
        .unwrap_or("")
        .to_string()
}

async fn handle_initialize(stdout: &mut tokio::io::Stdout, id: Option<Value>) {
    let resp = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": 1,
            "loadSession": true,
            "mcpCapabilities": { "http": true, "sse": true },
            "promptCapabilities": { "embeddedContext": true },
            "sessionCapabilities": {}
        }
    });
    write_message(stdout, &resp).await;
}

async fn handle_session_new(stdout: &mut tokio::io::Stdout, id: Option<Value>) -> String {
    let sid = uuid::Uuid::new_v4().to_string();
    let resp = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "sessionId": sid }
    });
    write_message(stdout, &resp).await;
    sid
}

async fn handle_session_prompt(
    stdout: &mut tokio::io::Stdout,
    id: Option<Value>,
    session_id: &str,
    user_msg: &str,
    request_permission: bool,
    lines: &mut tokio::io::Lines<BufReader<tokio::io::Stdin>>,
) {
    let count = PROMPT_COUNT.load(Ordering::SeqCst);

    if request_permission && count == 0 {
        let perm_req = json!({
            "jsonrpc": "2.0",
            "id": 9000,
            "method": "session/request_permission",
            "params": {
                "sessionId": session_id,
                "tool": "shell",
                "description": "Run echo command",
                "options": [
                    { "id": "allow_once", "label": "Allow once" },
                    { "id": "reject_once", "label": "Reject" }
                ]
            }
        });
        write_message(stdout, &perm_req).await;

        if let Ok(Some(resp_line)) = lines.next_line().await {
            let _resp: Value = serde_json::from_str(&resp_line).unwrap_or(json!(null));
        }
    }

    let chunk1 = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": { "sessionId": session_id, "type": "agent_message_chunk", "content": "Echo: " }
    });
    write_message(stdout, &chunk1).await;

    let chunk2 = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": { "sessionId": session_id, "type": "agent_message_chunk", "content": user_msg }
    });
    write_message(stdout, &chunk2).await;

    let result_notif = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": session_id,
            "type": "result",
            "content": format!("Echo: {user_msg}")
        }
    });
    write_message(stdout, &result_notif).await;

    let resp = json!({ "jsonrpc": "2.0", "id": id, "result": {} });
    write_message(stdout, &resp).await;
}

async fn handle_session_cancel(stdout: &mut tokio::io::Stdout, id: Option<Value>) {
    let resp = json!({ "jsonrpc": "2.0", "id": id, "result": {} });
    write_message(stdout, &resp).await;
}

async fn handle_session_load(
    stdout: &mut tokio::io::Stdout,
    id: Option<Value>,
    reject: bool,
    session_id: &mut Option<String>,
) {
    if reject {
        let resp = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32000, "message": "Session load rejected" }
        });
        write_message(stdout, &resp).await;
    } else {
        let sid = session_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        *session_id = Some(sid.clone());
        let resp = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "sessionId": sid }
        });
        write_message(stdout, &resp).await;
    }
}

async fn handle_session_close(stdout: &mut tokio::io::Stdout, id: Option<Value>) {
    let resp = json!({ "jsonrpc": "2.0", "id": id, "result": {} });
    write_message(stdout, &resp).await;
}
