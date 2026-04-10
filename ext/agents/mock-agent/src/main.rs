use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

static PROMPT_COUNT: AtomicUsize = AtomicUsize::new(0);
static THINK_ENABLED: AtomicBool = AtomicBool::new(true);
static AGENT_OPTIONS: OnceLock<AgentOptions> = OnceLock::new();
static MCP_SERVER_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
struct AgentOptions {
    exit_after: Option<usize>,
    thinking_time_ms: Option<u64>,
    request_permission: bool,
    reject_load: bool,
    echo_prefix: String,
    echo_mcp_count: bool,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            exit_after: None,
            thinking_time_ms: None,
            request_permission: false,
            reject_load: false,
            echo_prefix: "Echo".to_string(),
            echo_mcp_count: false,
        }
    }
}

impl AgentOptions {
    fn from_initialize_params(params: &Value) -> Self {
        let options = &params["options"];
        Self {
            exit_after: options["exit_after"].as_u64().map(|v| v as usize),
            thinking_time_ms: options["thinking_time_ms"].as_u64(),
            request_permission: options["request_permission"].as_bool().unwrap_or(false),
            reject_load: options["reject_load"].as_bool().unwrap_or(false),
            echo_prefix: options["echo_prefix"]
                .as_str()
                .unwrap_or("Echo")
                .to_string(),
            echo_mcp_count: options["echo_mcp_count"].as_bool().unwrap_or(false),
        }
    }
}

fn opts() -> &'static AgentOptions {
    AGENT_OPTIONS.get().expect("initialize must be called first")
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    // Simulate real-world agents (e.g. Node.js) that emit non-JSON startup noise to stdout.
    // The host's reader loop must skip these lines instead of terminating.
    if std::env::args().any(|a| a == "--noisy-startup") {
        use tokio::io::AsyncWriteExt;
        stdout.write_all(b"[npm warn] some startup noise\n").await.ok();
        stdout.write_all(b"Loading agent v1.2.3...\n").await.ok();
        stdout.flush().await.ok();
    }

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

        match method {
            "initialize" => {
                handle_initialize(&mut stdout, id, &msg).await;
            }
            "session/new" => {
                session_id = Some(handle_session_new(&mut stdout, id, &msg).await);
            }
            "session/prompt" => {
                let sid = session_id.clone().unwrap_or_else(|| "unknown".to_string());
                let user_msg = match extract_prompt_message(&msg) {
                    Some(m) => m,
                    None => {
                        let resp = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32602, "message": "session/prompt requires 'prompt' (array of content parts)" }
                        });
                        write_message(&mut stdout, &resp).await;
                        continue;
                    }
                };
                let think = THINK_ENABLED.load(Ordering::SeqCst);
                handle_session_prompt(
                    &mut stdout,
                    id,
                    &sid,
                    &user_msg,
                    opts().request_permission,
                    think,
                    &mut lines,
                )
                .await;

                let count = PROMPT_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
                if let Some(limit) = opts().exit_after {
                    if count >= limit {
                        std::process::exit(1);
                    }
                }
            }
            "session/cancel" => {
                handle_session_cancel(&mut stdout, id).await;
            }
            "session/load" => {
                handle_session_load(&mut stdout, id, opts().reject_load, &mut session_id).await;
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

async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, msg: &Value) {
    let mut line = serde_json::to_string(msg).expect("failed to serialize");
    line.push('\n');
    writer.write_all(line.as_bytes()).await.expect("failed to write");
    writer.flush().await.expect("failed to flush");
}

fn extract_prompt_message(msg: &Value) -> Option<String> {
    // ACP wire format: prompt is a flat array of content parts
    // e.g. {"prompt": [{"type": "text", "text": "hello"}]}
    let prompt = msg["params"]["prompt"].as_array()?;
    let first = prompt.first()?;
    if first["type"].as_str()? != "text" {
        return None;
    }
    first["text"].as_str().map(|s| s.to_string())
}

async fn handle_initialize(stdout: &mut tokio::io::Stdout, id: Option<Value>, msg: &Value) {
    let params = &msg["params"];
    let think = params["options"]["thinking"].as_bool().unwrap_or(true);
    THINK_ENABLED.store(think, Ordering::SeqCst);

    let _ = AGENT_OPTIONS.set(AgentOptions::from_initialize_params(params));

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

async fn handle_session_new<W: AsyncWrite + Unpin>(stdout: &mut W, id: Option<Value>, msg: &Value) -> String {
    let params = &msg["params"];

    if !params.get("cwd").is_some_and(|v| v.is_string()) {
        let resp = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": "session/new requires 'cwd' (string)" }
        });
        write_message(stdout, &resp).await;
        return String::new();
    }

    if !params.get("mcpServers").is_some_and(|v| v.is_array()) {
        let resp = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": "session/new requires 'mcpServers' (array)" }
        });
        write_message(stdout, &resp).await;
        return String::new();
    }

    let count = params["mcpServers"].as_array().map(|a| a.len()).unwrap_or(0);
    MCP_SERVER_COUNT.store(count, Ordering::SeqCst);

    let sid = uuid::Uuid::new_v4().to_string();
    let resp = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "sessionId": sid }
    });
    write_message(stdout, &resp).await;
    sid
}

async fn handle_session_prompt<W: AsyncWrite + Unpin>(
    stdout: &mut W,
    id: Option<Value>,
    session_id: &str,
    user_msg: &str,
    request_permission: bool,
    think: bool,
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

    if think {
        for thought in ["Analyzing your message...", "Formulating response..."] {
            let chunk = json!({
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": {
                    "sessionId": session_id,
                    "update": {
                        "sessionUpdate": "agent_thought_chunk",
                        "content": thought
                    }
                }
            });
            write_message(stdout, &chunk).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }

        if let Some(ms) = AGENT_OPTIONS.get().and_then(|o| o.thinking_time_ms) {
            tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
        }
    }

    let prefix = AGENT_OPTIONS
        .get()
        .map(|o| o.echo_prefix.as_str())
        .unwrap_or("Echo");

    let echo_mcp_count = AGENT_OPTIONS
        .get()
        .map(|o| o.echo_mcp_count)
        .unwrap_or(false);

    let chunk1 = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": { "sessionId": session_id, "update": { "sessionUpdate": "agent_message_chunk", "content": format!("{prefix}: ") } }
    });
    write_message(stdout, &chunk1).await;

    let chunk2 = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": { "sessionId": session_id, "update": { "sessionUpdate": "agent_message_chunk", "content": user_msg } }
    });
    write_message(stdout, &chunk2).await;

    let result_content = if echo_mcp_count {
        let count = MCP_SERVER_COUNT.load(Ordering::SeqCst);
        format!("{prefix}: {user_msg} [mcp:{count}]")
    } else {
        format!("{prefix}: {user_msg}")
    };

    let result_notif = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": session_id,
            "update": {
                "sessionUpdate": "result",
                "content": result_content
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    async fn collect_prompt_output(user_msg: &str, think: bool) -> Vec<Value> {
        use tokio::io::AsyncBufReadExt;

        let (reader, mut writer) = tokio::io::duplex(8192);

        let msg = user_msg.to_string();
        let write_handle = tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let buf = BufReader::new(stdin);
            let mut lines = buf.lines();
            handle_session_prompt(
                &mut writer,
                Some(json!(1)),
                "test-session",
                &msg,
                false,
                think,
                &mut lines,
            )
            .await;
            drop(writer);
        });

        let buf_reader = tokio::io::BufReader::new(reader);
        let mut read_lines = buf_reader.lines();
        let mut messages = Vec::new();
        while let Ok(Some(line)) = read_lines.next_line().await {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                messages.push(v);
            }
        }
        write_handle.await.unwrap();
        messages
    }

    #[tokio::test]
    async fn no_thought_chunks_when_think_disabled() {
        let msgs = collect_prompt_output("hello", false).await;
        assert_eq!(msgs.len(), 4);
        for msg in &msgs {
            if let Some(params) = msg.get("params") {
                let update_type = params.get("update")
                    .and_then(|u| u.get("sessionUpdate"))
                    .and_then(|t| t.as_str());
                assert_ne!(
                    update_type,
                    Some("agent_thought_chunk"),
                );
            }
        }
    }

    #[tokio::test]
    async fn thought_chunks_emitted_when_think_enabled() {
        let msgs = collect_prompt_output("hello", true).await;
        assert_eq!(msgs.len(), 6);

        let t1 = &msgs[0]["params"]["update"];
        assert_eq!(t1["sessionUpdate"], "agent_thought_chunk");
        assert_eq!(t1["content"], "Analyzing your message...");

        let t2 = &msgs[1]["params"]["update"];
        assert_eq!(t2["sessionUpdate"], "agent_thought_chunk");
        assert_eq!(t2["content"], "Formulating response...");
    }

    #[tokio::test]
    async fn echo_still_works_after_thoughts() {
        let msgs = collect_prompt_output("test msg", true).await;

        let echo_chunk = &msgs[2]["params"]["update"];
        assert_eq!(echo_chunk["sessionUpdate"], "agent_message_chunk");
        assert_eq!(echo_chunk["content"], "Echo: ");

        let echo_content = &msgs[3]["params"]["update"];
        assert_eq!(echo_content["sessionUpdate"], "agent_message_chunk");
        assert_eq!(echo_content["content"], "test msg");

        let result = &msgs[4]["params"]["update"];
        assert_eq!(result["sessionUpdate"], "result");
        assert_eq!(result["content"], "Echo: test msg");
    }

    #[test]
    fn extract_prompt_message_valid_array() {
        let msg = json!({
            "params": {
                "prompt": [{"type": "text", "text": "hello world"}]
            }
        });
        assert_eq!(extract_prompt_message(&msg), Some("hello world".to_string()));
    }

    #[test]
    fn extract_prompt_message_missing_prompt() {
        let msg = json!({ "params": {} });
        assert_eq!(extract_prompt_message(&msg), None);
    }

    #[test]
    fn extract_prompt_message_old_message_format_rejected() {
        let msg = json!({
            "params": {
                "message": {"role": "user", "content": [{"type": "text", "text": "hello"}]}
            }
        });
        assert_eq!(extract_prompt_message(&msg), None);
    }

    #[test]
    fn extract_prompt_message_prompt_not_array() {
        let msg = json!({
            "params": {
                "prompt": "hello"
            }
        });
        assert_eq!(extract_prompt_message(&msg), None);
    }

    #[test]
    fn extract_prompt_message_wrapped_message_format_rejected() {
        let msg = json!({
            "params": {
                "prompt": [{"role": "user", "content": [{"type": "text", "text": "hello"}]}]
            }
        });
        assert_eq!(extract_prompt_message(&msg), None);
    }

    #[test]
    fn extract_prompt_message_non_text_type_rejected() {
        let msg = json!({
            "params": {
                "prompt": [{"type": "image", "data": "base64data", "mimeType": "image/png"}]
            }
        });
        assert_eq!(extract_prompt_message(&msg), None);
    }

    async fn collect_session_new_output(params: Value) -> Vec<Value> {
        use tokio::io::AsyncBufReadExt;

        let (reader, mut writer) = tokio::io::duplex(8192);
        let msg = json!({ "jsonrpc": "2.0", "id": 1, "method": "session/new", "params": params });

        let write_handle = tokio::spawn(async move {
            handle_session_new(&mut writer, Some(json!(1)), &msg).await;
            drop(writer);
        });

        let buf_reader = tokio::io::BufReader::new(reader);
        let mut read_lines = buf_reader.lines();
        let mut messages = Vec::new();
        while let Ok(Some(line)) = read_lines.next_line().await {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                messages.push(v);
            }
        }
        write_handle.await.unwrap();
        messages
    }

    #[tokio::test]
    async fn session_new_valid_params() {
        let msgs = collect_session_new_output(json!({
            "cwd": "/workspace",
            "mcpServers": []
        })).await;
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0]["result"]["sessionId"].is_string());
    }

    #[tokio::test]
    async fn session_new_missing_cwd_returns_error() {
        let msgs = collect_session_new_output(json!({
            "mcpServers": []
        })).await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["error"]["code"], -32602);
        assert!(msgs[0]["error"]["message"].as_str().unwrap().contains("cwd"));
    }

    #[tokio::test]
    async fn session_new_missing_mcp_servers_returns_error() {
        let msgs = collect_session_new_output(json!({
            "cwd": "/workspace"
        })).await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["error"]["code"], -32602);
        assert!(msgs[0]["error"]["message"].as_str().unwrap().contains("mcpServers"));
    }

    #[tokio::test]
    async fn session_new_cwd_not_string_returns_error() {
        let msgs = collect_session_new_output(json!({
            "cwd": 123,
            "mcpServers": []
        })).await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["error"]["code"], -32602);
    }
}
