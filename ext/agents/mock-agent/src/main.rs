use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

static PROMPT_COUNT: AtomicUsize = AtomicUsize::new(0);
static THINK_ENABLED: AtomicBool = AtomicBool::new(true);
static AGENT_OPTIONS: OnceLock<AgentOptions> = OnceLock::new();

#[derive(Debug)]
struct AgentOptions {
    exit_after: Option<usize>,
    delay_ms: Option<u64>,
    request_permission: bool,
    reject_load: bool,
    echo_prefix: String,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            exit_after: None,
            delay_ms: None,
            request_permission: false,
            reject_load: false,
            echo_prefix: "Echo".to_string(),
        }
    }
}

impl AgentOptions {
    fn from_initialize_params(params: &Value) -> Self {
        let options = &params["options"];
        Self {
            exit_after: options["exit_after"].as_u64().map(|v| v as usize),
            delay_ms: options["delay_ms"].as_u64(),
            request_permission: options["request_permission"].as_bool().unwrap_or(false),
            reject_load: options["reject_load"].as_bool().unwrap_or(false),
            echo_prefix: options["echo_prefix"]
                .as_str()
                .unwrap_or("Echo")
                .to_string(),
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

        if AGENT_OPTIONS.get().is_some() {
            if let Some(ms) = opts().delay_ms {
                tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
            }
        }

        match method {
            "initialize" => {
                handle_initialize(&mut stdout, id, &msg).await;
            }
            "session/new" => {
                session_id = Some(handle_session_new(&mut stdout, id).await);
            }
            "session/prompt" => {
                let sid = session_id.clone().unwrap_or_else(|| "unknown".to_string());
                let user_msg = extract_prompt_message(&msg);
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

fn extract_prompt_message(msg: &Value) -> String {
    msg["params"]["prompt"]
        .as_str()
        .or_else(|| msg["params"]["message"].as_str())
        .or_else(|| msg["params"]["message"]["content"].as_str())
        .unwrap_or("")
        .to_string()
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
                    "type": "agent_thought_chunk",
                    "content": thought
                }
            });
            write_message(stdout, &chunk).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    }

    let prefix = AGENT_OPTIONS
        .get()
        .map(|o| o.echo_prefix.as_str())
        .unwrap_or("Echo");

    let chunk1 = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": { "sessionId": session_id, "type": "agent_message_chunk", "content": format!("{prefix}: ") }
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
            "content": format!("{prefix}: {user_msg}")
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
                assert_ne!(
                    params.get("type").and_then(|t| t.as_str()),
                    Some("agent_thought_chunk"),
                );
            }
        }
    }

    #[tokio::test]
    async fn thought_chunks_emitted_when_think_enabled() {
        let msgs = collect_prompt_output("hello", true).await;
        assert_eq!(msgs.len(), 6);

        let t1 = &msgs[0]["params"];
        assert_eq!(t1["type"], "agent_thought_chunk");
        assert_eq!(t1["content"], "Analyzing your message...");

        let t2 = &msgs[1]["params"];
        assert_eq!(t2["type"], "agent_thought_chunk");
        assert_eq!(t2["content"], "Formulating response...");
    }

    #[tokio::test]
    async fn echo_still_works_after_thoughts() {
        let msgs = collect_prompt_output("test msg", true).await;

        let echo_chunk = &msgs[2]["params"];
        assert_eq!(echo_chunk["type"], "agent_message_chunk");
        assert_eq!(echo_chunk["content"], "Echo: ");

        let echo_content = &msgs[3]["params"];
        assert_eq!(echo_content["type"], "agent_message_chunk");
        assert_eq!(echo_content["content"], "test msg");

        let result = &msgs[4]["params"];
        assert_eq!(result["type"], "result");
        assert_eq!(result["content"], "Echo: test msg");
    }
}
