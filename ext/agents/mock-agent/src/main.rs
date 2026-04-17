// D-03: mock-agent is a standalone ACP test binary that speaks raw JSON-RPC over stdio.
// It constructs protocol messages manually — there is no typed JSON-RPC envelope struct
// because the mock intentionally exercises the raw wire format.
#![allow(clippy::disallowed_types)]

use serde_json::{Value, json};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    reject_resume: bool,
    support_resume: bool,
    /// When set, session/resume and session/load return this ID instead of the sent one.
    recovery_new_id: Option<String>,
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
            reject_resume: false,
            support_resume: false,
            recovery_new_id: None,
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
            reject_resume: options["reject_resume"].as_bool().unwrap_or(false),
            support_resume: options["support_resume"].as_bool().unwrap_or(false),
            recovery_new_id: options["recovery_new_id"].as_str().map(String::from),
            echo_prefix: options["echo_prefix"]
                .as_str()
                .unwrap_or("Echo")
                .to_string(),
            echo_mcp_count: options["echo_mcp_count"].as_bool().unwrap_or(false),
        }
    }
}

fn opts() -> &'static AgentOptions {
    AGENT_OPTIONS
        .get()
        .expect("initialize must be called first")
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    // Simulate real-world agents (e.g. Node.js) that emit non-JSON startup noise to stdout.
    // The host's reader loop must skip these lines instead of terminating.
    if std::env::args().any(|a| a == "--noisy-startup") {
        use tokio::io::AsyncWriteExt;
        stdout
            .write_all(b"[npm warn] some startup noise\n")
            .await
            .ok();
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
                let Some(parts) = extract_prompt_parts(&msg) else {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32602, "message": "session/prompt requires 'prompt' (array of content parts)" }
                    });
                    write_message(&mut stdout, &resp).await;
                    continue;
                };
                let think = THINK_ENABLED.load(Ordering::SeqCst);
                handle_session_prompt(
                    &mut stdout,
                    id,
                    &sid,
                    &parts,
                    opts().request_permission,
                    think,
                    &mut lines,
                )
                .await;

                let count = PROMPT_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
                if let Some(limit) = opts().exit_after
                    && count >= limit
                {
                    std::process::exit(1);
                }
            }
            "session/cancel" => {
                handle_session_cancel(&mut stdout, id).await;
            }
            "session/load" => {
                handle_session_load(&mut stdout, id, opts().reject_load, &mut session_id).await;
            }
            "session/resume" => {
                handle_session_resume(&mut stdout, id, opts().reject_resume, &mut session_id).await;
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
    writer
        .write_all(line.as_bytes())
        .await
        .expect("failed to write");
    writer.flush().await.expect("failed to flush");
}

fn extract_prompt_parts(msg: &Value) -> Option<Vec<Value>> {
    let prompt = msg["params"]["prompt"].as_array()?;
    if prompt.is_empty() {
        return None;
    }
    Some(prompt.clone())
}

async fn handle_initialize(stdout: &mut tokio::io::Stdout, id: Option<Value>, msg: &Value) {
    let params = &msg["params"];
    let think = params["options"]["thinking"].as_bool().unwrap_or(true);
    THINK_ENABLED.store(think, Ordering::SeqCst);

    let _ = AGENT_OPTIONS.set(AgentOptions::from_initialize_params(params));

    let session_caps = if opts().support_resume {
        json!({ "resume": {} })
    } else {
        json!({})
    };

    const DEFAULTS_YAML: &str = include_str!("../defaults.yaml");
    let defaults: serde_json::Value =
        serde_yaml::from_str(DEFAULTS_YAML).expect("embedded defaults.yaml must be valid YAML");

    let resp = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": 2,
            "agentCapabilities": {
                "loadSession": true,
                "mcpCapabilities": { "http": true, "sse": true },
                "promptCapabilities": { "embeddedContext": true },
                "sessionCapabilities": session_caps
            },
            "defaults": defaults
        }
    });
    write_message(stdout, &resp).await;

    let commands_update = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": "__global__",
            "update": {
                "sessionUpdate": "available_commands_update",
                "commands": [
                    { "name": "help", "description": "Show available commands" },
                    { "name": "status", "description": "Show agent status" }
                ]
            }
        }
    });
    write_message(stdout, &commands_update).await;
}

async fn handle_session_new<W: AsyncWrite + Unpin>(
    stdout: &mut W,
    id: Option<Value>,
    msg: &Value,
) -> String {
    let params = &msg["params"];

    if !params.get("cwd").is_some_and(serde_json::Value::is_string) {
        let resp = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": "session/new requires 'cwd' (string)" }
        });
        write_message(stdout, &resp).await;
        return String::new();
    }

    if !params
        .get("mcpServers")
        .is_some_and(serde_json::Value::is_array)
    {
        let resp = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": "session/new requires 'mcpServers' (array)" }
        });
        write_message(stdout, &resp).await;
        return String::new();
    }

    let count = params["mcpServers"].as_array().map(Vec::len).unwrap_or(0);
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
    parts: &[Value],
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
                        "content": { "type": "text", "text": thought }
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

    // Echo each content part back with an appropriate agent_message_chunk.
    for part in parts {
        let part_type = part["type"].as_str().unwrap_or("");
        let content = match part_type {
            "text" => {
                let text = part["text"].as_str().unwrap_or("");
                json!({ "type": "text", "text": format!("{prefix}: {text}") })
            }
            "image" => {
                json!({ "type": "image", "url": part["url"] })
            }
            "file" => {
                json!({
                    "type": "file",
                    "url": part["url"],
                    "filename": part["filename"],
                    "mimeType": part["mimeType"]
                })
            }
            "audio" => {
                json!({
                    "type": "audio",
                    "url": part["url"],
                    "mimeType": part["mimeType"]
                })
            }
            _ => {
                // Unknown part type — echo as text description.
                json!({ "type": "text", "text": format!("{prefix}: [unknown part type '{part_type}']") })
            }
        };

        let chunk = json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": session_id,
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": content
                }
            }
        });
        write_message(stdout, &chunk).await;
    }

    // Build the result summary string.
    let mut result_content = if parts.len() == 1 && parts[0]["type"].as_str() == Some("text") {
        let text = parts[0]["text"].as_str().unwrap_or("");
        format!("{prefix}: {text}")
    } else {
        let noun = if parts.len() == 1 { "part" } else { "parts" };
        format!("Echoed {} content {noun}", parts.len())
    };

    if echo_mcp_count {
        let mcp = MCP_SERVER_COUNT.load(Ordering::SeqCst);
        result_content.push_str(&format!(" [mcp:{mcp}]"));
    }

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

    let resp = json!({ "jsonrpc": "2.0", "id": id, "result": { "stopReason": "end_turn" } });
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
        let sid = opts()
            .recovery_new_id
            .clone()
            .or_else(|| session_id.clone())
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

async fn handle_session_resume(
    stdout: &mut tokio::io::Stdout,
    id: Option<Value>,
    reject: bool,
    session_id: &mut Option<String>,
) {
    if reject {
        let resp = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32000, "message": "Session resume rejected" }
        });
        write_message(stdout, &resp).await;
    } else {
        let sid = opts()
            .recovery_new_id
            .clone()
            .or_else(|| session_id.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    async fn collect_parts_output(parts: Vec<Value>, think: bool) -> Vec<Value> {
        use tokio::io::AsyncBufReadExt;

        let (reader, mut writer) = tokio::io::duplex(8192);

        let write_handle = tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let buf = BufReader::new(stdin);
            let mut lines = buf.lines();
            handle_session_prompt(
                &mut writer,
                Some(json!(1)),
                "test-session",
                &parts,
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

    fn text_parts(text: &str) -> Vec<Value> {
        vec![json!({"type": "text", "text": text})]
    }

    #[tokio::test]
    async fn no_thought_chunks_when_think_disabled() {
        // Single text part: 1 echo chunk + result_notif + rpc_response = 3 messages
        let msgs = collect_parts_output(text_parts("hello"), false).await;
        assert_eq!(msgs.len(), 3);
        for msg in &msgs {
            if let Some(params) = msg.get("params") {
                let update_type = params
                    .get("update")
                    .and_then(|u| u.get("sessionUpdate"))
                    .and_then(|t| t.as_str());
                assert_ne!(update_type, Some("agent_thought_chunk"),);
            }
        }
    }

    #[tokio::test]
    async fn thought_chunks_emitted_when_think_enabled() {
        // Single text part: 2 thoughts + 1 echo chunk + result_notif + rpc_response = 5 messages
        let msgs = collect_parts_output(text_parts("hello"), true).await;
        assert_eq!(msgs.len(), 5);

        let t1 = &msgs[0]["params"]["update"];
        assert_eq!(t1["sessionUpdate"], "agent_thought_chunk");
        assert_eq!(t1["content"]["type"], "text");
        assert_eq!(t1["content"]["text"], "Analyzing your message...");

        let t2 = &msgs[1]["params"]["update"];
        assert_eq!(t2["sessionUpdate"], "agent_thought_chunk");
        assert_eq!(t2["content"]["type"], "text");
        assert_eq!(t2["content"]["text"], "Formulating response...");
    }

    #[tokio::test]
    async fn echo_still_works_after_thoughts() {
        let msgs = collect_parts_output(text_parts("test msg"), true).await;

        let echo_chunk = &msgs[2]["params"]["update"];
        assert_eq!(echo_chunk["sessionUpdate"], "agent_message_chunk");
        assert_eq!(echo_chunk["content"]["type"], "text");
        assert_eq!(echo_chunk["content"]["text"], "Echo: test msg");

        let result = &msgs[3]["params"]["update"];
        assert_eq!(result["sessionUpdate"], "result");
        assert_eq!(result["content"], "Echo: test msg");
    }

    #[test]
    fn extract_prompt_parts_valid_array() {
        let msg = json!({
            "params": {
                "prompt": [{"type": "text", "text": "hello world"}]
            }
        });
        let parts = extract_prompt_parts(&msg).expect("should extract parts");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "hello world");
    }

    #[test]
    fn extract_prompt_parts_missing_prompt() {
        let msg = json!({ "params": {} });
        assert!(extract_prompt_parts(&msg).is_none());
    }

    #[test]
    fn extract_prompt_parts_old_message_format_rejected() {
        let msg = json!({
            "params": {
                "message": {"role": "user", "content": [{"type": "text", "text": "hello"}]}
            }
        });
        assert!(extract_prompt_parts(&msg).is_none());
    }

    #[test]
    fn extract_prompt_parts_prompt_not_array() {
        let msg = json!({
            "params": {
                "prompt": "hello"
            }
        });
        assert!(extract_prompt_parts(&msg).is_none());
    }

    #[test]
    fn extract_prompt_parts_wrapped_message_format_rejected() {
        let msg = json!({
            "params": {
                "prompt": [{"role": "user", "content": [{"type": "text", "text": "hello"}]}]
            }
        });
        // Parts are returned (the array is valid), but they won't match known types when echoed.
        let parts = extract_prompt_parts(&msg).expect("should extract parts");
        assert_eq!(parts.len(), 1);
    }

    #[test]
    fn when_prompt_has_image_part_then_extracts_it() {
        let msg = json!({
            "params": {
                "prompt": [{"type": "image", "url": "https://example.com/img.png"}]
            }
        });
        let parts = extract_prompt_parts(&msg).expect("should extract parts");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["type"], "image");
        assert_eq!(parts[0]["url"], "https://example.com/img.png");
    }

    #[test]
    fn when_prompt_has_mixed_parts_then_extracts_all() {
        let msg = json!({
            "params": {
                "prompt": [
                    {"type": "text", "text": "look at this"},
                    {"type": "image", "url": "https://example.com/img.png"},
                    {"type": "file", "url": "https://example.com/doc.pdf", "filename": "doc.pdf", "mimeType": "application/pdf"}
                ]
            }
        });
        let parts = extract_prompt_parts(&msg).expect("should extract parts");
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["type"], "image");
        assert_eq!(parts[2]["type"], "file");
    }

    #[tokio::test]
    async fn when_image_part_then_echoes_image_chunk() {
        let parts = vec![json!({"type": "image", "url": "https://example.com/img.png"})];
        let msgs = collect_parts_output(parts, false).await;
        // 1 image echo chunk + result_notif + rpc_response = 3 messages
        assert_eq!(msgs.len(), 3);
        let echo = &msgs[0]["params"]["update"];
        assert_eq!(echo["sessionUpdate"], "agent_message_chunk");
        assert_eq!(echo["content"]["type"], "image");
        assert_eq!(echo["content"]["url"], "https://example.com/img.png");

        let result = &msgs[1]["params"]["update"];
        assert_eq!(result["sessionUpdate"], "result");
        assert_eq!(result["content"], "Echoed 1 content part");
    }

    #[tokio::test]
    async fn when_mixed_parts_then_echoes_all_and_summarizes() {
        let parts = vec![
            json!({"type": "text", "text": "check this out"}),
            json!({"type": "image", "url": "https://example.com/pic.jpg"}),
        ];
        let msgs = collect_parts_output(parts, false).await;
        // 2 echo chunks + result_notif + rpc_response = 4 messages
        assert_eq!(msgs.len(), 4);

        let text_chunk = &msgs[0]["params"]["update"];
        assert_eq!(text_chunk["sessionUpdate"], "agent_message_chunk");
        assert_eq!(text_chunk["content"]["type"], "text");
        assert_eq!(text_chunk["content"]["text"], "Echo: check this out");

        let image_chunk = &msgs[1]["params"]["update"];
        assert_eq!(image_chunk["sessionUpdate"], "agent_message_chunk");
        assert_eq!(image_chunk["content"]["type"], "image");

        let result = &msgs[2]["params"]["update"];
        assert_eq!(result["sessionUpdate"], "result");
        assert_eq!(result["content"], "Echoed 2 content parts");
    }

    #[tokio::test]
    async fn when_file_part_then_echoes_file_chunk() {
        let parts = vec![json!({
            "type": "file",
            "url": "https://example.com/report.pdf",
            "filename": "report.pdf",
            "mimeType": "application/pdf"
        })];
        let msgs = collect_parts_output(parts, false).await;
        assert_eq!(msgs.len(), 3);
        let echo = &msgs[0]["params"]["update"];
        assert_eq!(echo["content"]["type"], "file");
        assert_eq!(echo["content"]["url"], "https://example.com/report.pdf");
        assert_eq!(echo["content"]["filename"], "report.pdf");
        assert_eq!(echo["content"]["mimeType"], "application/pdf");
    }

    #[tokio::test]
    async fn when_audio_part_then_echoes_audio_chunk() {
        let parts = vec![json!({
            "type": "audio",
            "url": "https://example.com/clip.mp3",
            "mimeType": "audio/mpeg"
        })];
        let msgs = collect_parts_output(parts, false).await;
        assert_eq!(msgs.len(), 3);
        let echo = &msgs[0]["params"]["update"];
        assert_eq!(echo["content"]["type"], "audio");
        assert_eq!(echo["content"]["url"], "https://example.com/clip.mp3");
        assert_eq!(echo["content"]["mimeType"], "audio/mpeg");
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
        }))
        .await;
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0]["result"]["sessionId"].is_string());
    }

    #[tokio::test]
    async fn session_new_missing_cwd_returns_error() {
        let msgs = collect_session_new_output(json!({
            "mcpServers": []
        }))
        .await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["error"]["code"], -32602);
        assert!(
            msgs[0]["error"]["message"]
                .as_str()
                .unwrap()
                .contains("cwd")
        );
    }

    #[tokio::test]
    async fn session_new_missing_mcp_servers_returns_error() {
        let msgs = collect_session_new_output(json!({
            "cwd": "/workspace"
        }))
        .await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["error"]["code"], -32602);
        assert!(
            msgs[0]["error"]["message"]
                .as_str()
                .unwrap()
                .contains("mcpServers")
        );
    }

    #[tokio::test]
    async fn session_new_cwd_not_string_returns_error() {
        let msgs = collect_session_new_output(json!({
            "cwd": 123,
            "mcpServers": []
        }))
        .await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["error"]["code"], -32602);
    }
}
