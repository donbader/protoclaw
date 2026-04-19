use std::cell::{Cell, RefCell};
use std::rc::Rc;

use agent_client_protocol::SessionResumeCapabilities;
use agent_client_protocol::{AgentCapabilities, McpCapabilities, PromptCapabilities};
use agent_client_protocol::{
    AgentSideConnection, AvailableCommand, AvailableCommandsUpdate, ContentBlock, ContentChunk,
    Error, InitializeRequest, InitializeResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
    PermissionOption, PermissionOptionKind, PromptRequest, PromptResponse,
    RequestPermissionRequest, ResumeSessionRequest, ResumeSessionResponse, SessionNotification,
    SessionUpdate, StopReason, TextContent, ToolCallId, ToolCallUpdate, ToolCallUpdateFields,
};
use agent_client_protocol::{AuthenticateRequest, AuthenticateResponse, CancelNotification};
use agent_client_protocol::{Client, SessionCapabilities};
use async_trait::async_trait;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const DEFAULTS_YAML: &str = include_str!("../defaults.yaml");

#[derive(Debug)]
struct AgentOptions {
    exit_after: Option<usize>,
    thinking_time_ms: Option<u64>,
    thinking_enabled: bool,
    request_permission: bool,
    reject_load: bool,
    reject_resume: bool,
    support_resume: bool,
    recovery_new_id: Option<String>,
    echo_prefix: String,
    echo_mcp_count: bool,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            exit_after: None,
            thinking_time_ms: None,
            thinking_enabled: true,
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
    #[allow(clippy::disallowed_types)]
    fn from_meta(meta: Option<&serde_json::Map<String, serde_json::Value>>) -> Self {
        let empty = serde_json::Map::new();
        let opts_map = meta
            .and_then(|m| m.get("options"))
            .and_then(|v| v.as_object());
        let opts = opts_map.unwrap_or(&empty);

        let get_bool = |key: &str| opts.get(key).and_then(serde_json::Value::as_bool);
        let get_u64 = |key: &str| opts.get(key).and_then(serde_json::Value::as_u64);
        let get_str = |key: &str| opts.get(key).and_then(|v| v.as_str()).map(String::from);

        Self {
            exit_after: get_u64("exit_after").map(|v| v as usize),
            thinking_time_ms: get_u64("thinking_time_ms"),
            thinking_enabled: get_bool("thinking").unwrap_or(false),
            request_permission: get_bool("request_permission").unwrap_or(false),
            reject_load: get_bool("reject_load").unwrap_or(false),
            reject_resume: get_bool("reject_resume").unwrap_or(false),
            support_resume: get_bool("support_resume").unwrap_or(false),
            recovery_new_id: get_str("recovery_new_id"),
            echo_prefix: get_str("echo_prefix").unwrap_or_else(|| "Echo".to_string()),
            echo_mcp_count: get_bool("echo_mcp_count").unwrap_or(false),
        }
    }
}

type ConnCell = Rc<std::cell::OnceCell<AgentSideConnection>>;

struct MockHandler {
    conn: ConnCell,
    opts: RefCell<AgentOptions>,
    session_id: RefCell<Option<String>>,
    mcp_server_count: Cell<usize>,
    prompt_count: Cell<usize>,
}

impl MockHandler {
    fn new(conn: ConnCell) -> Self {
        Self {
            conn,
            opts: RefCell::new(AgentOptions::default()),
            session_id: RefCell::new(None),
            mcp_server_count: Cell::new(0),
            prompt_count: Cell::new(0),
        }
    }

    async fn send_notification(&self, notif: SessionNotification) {
        if let Some(conn) = self.conn.get() {
            let _ = conn.session_notification(notif).await;
        }
    }
}

#[async_trait(?Send)]
impl agent_client_protocol::Agent for MockHandler {
    async fn initialize(
        &self,
        args: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        let new_opts = AgentOptions::from_meta(args.meta.as_ref());
        *self.opts.borrow_mut() = new_opts;

        let support_resume = self.opts.borrow().support_resume;

        #[allow(clippy::disallowed_types)]
        let defaults: serde_json::Value =
            serde_yaml::from_str(DEFAULTS_YAML).expect("defaults.yaml must be valid YAML");

        let session_caps = {
            let caps = SessionCapabilities::new();
            if support_resume {
                caps.resume(SessionResumeCapabilities::new())
            } else {
                caps
            }
        };

        let agent_caps = AgentCapabilities::new()
            .load_session(true)
            .mcp_capabilities(McpCapabilities::new().http(true).sse(true))
            .prompt_capabilities(PromptCapabilities::new().embedded_context(true))
            .session_capabilities(session_caps);

        let mut meta = serde_json::Map::new();
        meta.insert("defaults".to_string(), defaults);

        let resp = InitializeResponse::new(args.protocol_version)
            .agent_capabilities(agent_caps)
            .meta(meta);

        self.send_notification(SessionNotification::new(
            "__global__",
            SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(vec![
                AvailableCommand::new("help", "Show available commands"),
                AvailableCommand::new("status", "Show agent status"),
            ])),
        ))
        .await;

        Ok(resp)
    }

    async fn authenticate(
        &self,
        _args: AuthenticateRequest,
    ) -> agent_client_protocol::Result<AuthenticateResponse> {
        Err(Error::method_not_found())
    }

    async fn new_session(
        &self,
        args: NewSessionRequest,
    ) -> agent_client_protocol::Result<NewSessionResponse> {
        self.mcp_server_count.set(args.mcp_servers.len());
        let sid = uuid::Uuid::new_v4().to_string();
        *self.session_id.borrow_mut() = Some(sid.clone());
        Ok(NewSessionResponse::new(sid))
    }

    async fn prompt(&self, args: PromptRequest) -> agent_client_protocol::Result<PromptResponse> {
        let session_id = args.session_id.to_string();
        let parts = args.prompt;
        let count = self.prompt_count.get();

        let (request_permission, think, thinking_time_ms, prefix, echo_mcp_count) = {
            let opts = self.opts.borrow();
            (
                opts.request_permission,
                opts.thinking_enabled,
                opts.thinking_time_ms,
                opts.echo_prefix.clone(),
                opts.echo_mcp_count,
            )
        };

        if request_permission
            && count == 0
            && let Some(conn) = self.conn.get()
        {
            let tool_call = ToolCallUpdate::new(
                ToolCallId::new("perm-1"),
                ToolCallUpdateFields::new().title("shell: Run echo command"),
            );
            let req = RequestPermissionRequest::new(
                session_id.clone(),
                tool_call,
                vec![
                    PermissionOption::new(
                        "allow_once",
                        "Allow once",
                        PermissionOptionKind::AllowOnce,
                    ),
                    PermissionOption::new(
                        "reject_once",
                        "Reject",
                        PermissionOptionKind::RejectOnce,
                    ),
                ],
            );
            let _ = conn.request_permission(req).await;
        }

        if think {
            for thought in ["Analyzing your message...", "Formulating response..."] {
                self.send_notification(SessionNotification::new(
                    session_id.clone(),
                    SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(
                        TextContent::new(thought),
                    ))),
                ))
                .await;
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }

            if let Some(ms) = thinking_time_ms {
                tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
            }
        }

        for part in &parts {
            let echo_block = echo_content_block(part, &prefix);
            self.send_notification(SessionNotification::new(
                session_id.clone(),
                SessionUpdate::AgentMessageChunk(ContentChunk::new(echo_block)),
            ))
            .await;
        }

        let mut result_content = if parts.len() == 1 {
            if let ContentBlock::Text(t) = &parts[0] {
                format!("{prefix}: {}", t.text)
            } else {
                "Echoed 1 content part".to_string()
            }
        } else {
            format!("Echoed {} content parts", parts.len())
        };

        if echo_mcp_count {
            let mcp = self.mcp_server_count.get();
            result_content.push_str(&format!(" [mcp:{mcp}]"));
        }

        self.send_notification(SessionNotification::new(
            session_id.clone(),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(result_content),
            ))),
        ))
        .await;

        let new_count = count + 1;
        self.prompt_count.set(new_count);
        let exit_after = self.opts.borrow().exit_after;

        let resp = PromptResponse::new(StopReason::EndTurn);

        if let Some(limit) = exit_after
            && new_count >= limit
        {
            std::process::exit(1);
        }

        Ok(resp)
    }

    async fn cancel(&self, _args: CancelNotification) -> agent_client_protocol::Result<()> {
        Ok(())
    }

    async fn load_session(
        &self,
        args: LoadSessionRequest,
    ) -> agent_client_protocol::Result<LoadSessionResponse> {
        if self.opts.borrow().reject_load {
            return Err(Error::new(-32000, "Session load rejected"));
        }
        let sid = self
            .opts
            .borrow()
            .recovery_new_id
            .clone()
            .or_else(|| self.session_id.borrow().clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let _ = args;
        *self.session_id.borrow_mut() = Some(sid.clone());
        Ok(LoadSessionResponse::new())
    }

    #[allow(unreachable_patterns)]
    async fn resume_session(
        &self,
        args: ResumeSessionRequest,
    ) -> agent_client_protocol::Result<ResumeSessionResponse> {
        if self.opts.borrow().reject_resume {
            return Err(Error::new(-32000, "Session resume rejected"));
        }
        let sid = self
            .opts
            .borrow()
            .recovery_new_id
            .clone()
            .or_else(|| self.session_id.borrow().clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let _ = args;
        *self.session_id.borrow_mut() = Some(sid.clone());
        Ok(ResumeSessionResponse::new())
    }

    async fn list_sessions(
        &self,
        _args: ListSessionsRequest,
    ) -> agent_client_protocol::Result<ListSessionsResponse> {
        Ok(ListSessionsResponse::new(vec![]))
    }
}

fn echo_content_block(block: &ContentBlock, prefix: &str) -> ContentBlock {
    match block {
        ContentBlock::Text(t) => {
            ContentBlock::Text(TextContent::new(format!("{prefix}: {}", t.text)))
        }
        ContentBlock::Image(img) => ContentBlock::Image(img.clone()),
        ContentBlock::Audio(audio) => ContentBlock::Audio(audio.clone()),
        ContentBlock::ResourceLink(link) => ContentBlock::Text(TextContent::new(format!(
            "{prefix}: [resource {}]",
            link.uri
        ))),
        ContentBlock::Resource(res) => {
            let uri = match &res.resource {
                agent_client_protocol::EmbeddedResourceResource::TextResourceContents(r) => {
                    r.uri.clone()
                }
                agent_client_protocol::EmbeddedResourceResource::BlobResourceContents(r) => {
                    r.uri.clone()
                }
                _ => String::from("unknown"),
            };
            ContentBlock::Text(TextContent::new(format!("{prefix}: [resource {uri}]")))
        }
        _ => ContentBlock::Text(TextContent::new(format!("{prefix}: [unknown content]"))),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    if std::env::args().any(|a| a == "--noisy-startup") {
        use tokio::io::AsyncWriteExt;
        stdout
            .write_all(b"[npm warn] some startup noise\n")
            .await
            .ok();
        stdout.write_all(b"Loading agent v1.2.3...\n").await.ok();
        stdout.flush().await.ok();
    }

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let conn_cell: ConnCell = Rc::new(std::cell::OnceCell::new());
            let handler = Rc::new(MockHandler::new(Rc::clone(&conn_cell)));

            let (conn, io_task) = AgentSideConnection::new(
                Rc::clone(&handler),
                stdout.compat_write(),
                stdin.compat(),
                |fut| {
                    tokio::task::spawn_local(fut);
                },
            );

            conn_cell.set(conn).expect("conn_cell set once");

            io_task.await.ok();
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{
        Agent, AgentSideConnection, ClientSideConnection, NewSessionRequest, PromptRequest,
        SessionNotification,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    fn make_handler() -> Rc<MockHandler> {
        let conn_cell: ConnCell = Rc::new(std::cell::OnceCell::new());
        Rc::new(MockHandler::new(conn_cell))
    }

    struct CollectingClient {
        notifications: Rc<RefCell<Vec<SessionNotification>>>,
    }

    impl CollectingClient {
        fn new(notifications: Rc<RefCell<Vec<SessionNotification>>>) -> Self {
            Self { notifications }
        }
    }

    #[async_trait(?Send)]
    impl agent_client_protocol::Client for CollectingClient {
        async fn request_permission(
            &self,
            _args: agent_client_protocol::RequestPermissionRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::RequestPermissionResponse>
        {
            use agent_client_protocol::{
                PermissionOptionId, RequestPermissionOutcome, RequestPermissionResponse,
                SelectedPermissionOutcome,
            };
            Ok(RequestPermissionResponse::new(
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    PermissionOptionId::new("allow_once"),
                )),
            ))
        }

        async fn session_notification(
            &self,
            args: SessionNotification,
        ) -> agent_client_protocol::Result<()> {
            self.notifications.borrow_mut().push(args);
            Ok(())
        }
    }

    async fn make_connected_pair() -> (
        ClientSideConnection,
        Rc<RefCell<Vec<SessionNotification>>>,
        Rc<MockHandler>,
    ) {
        let (client_rx, agent_tx) = tokio::io::duplex(65536);
        let (agent_rx, client_tx) = tokio::io::duplex(65536);

        let conn_cell: ConnCell = Rc::new(std::cell::OnceCell::new());
        let handler = Rc::new(MockHandler::new(Rc::clone(&conn_cell)));

        let (agent_conn, agent_io) = AgentSideConnection::new(
            Rc::clone(&handler),
            agent_tx.compat_write(),
            agent_rx.compat(),
            |fut| {
                tokio::task::spawn_local(fut);
            },
        );

        conn_cell.set(agent_conn).expect("set once");

        let notifications: Rc<RefCell<Vec<SessionNotification>>> =
            Rc::new(RefCell::new(Vec::new()));
        let (client_conn, client_io) = ClientSideConnection::new(
            CollectingClient::new(Rc::clone(&notifications)),
            client_tx.compat_write(),
            client_rx.compat(),
            |fut| {
                tokio::task::spawn_local(fut);
            },
        );

        tokio::task::spawn_local(async move {
            agent_io.await.ok();
        });
        tokio::task::spawn_local(async move {
            client_io.await.ok();
        });

        (client_conn, notifications, handler)
    }

    #[tokio::test]
    async fn no_thought_chunks_when_think_disabled() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, notifications, _handler) = make_connected_pair().await;

                let init_req =
                    InitializeRequest::new(agent_client_protocol::ProtocolVersion::from(2u16));
                client.initialize(init_req).await.expect("initialize");

                let session = client
                    .new_session(NewSessionRequest::new("/workspace").mcp_servers(vec![]))
                    .await
                    .expect("new_session");

                client
                    .prompt(PromptRequest::new(
                        session.session_id.clone(),
                        vec![ContentBlock::Text(TextContent::new("hello"))],
                    ))
                    .await
                    .expect("prompt");

                let thought_count = notifications
                    .borrow()
                    .iter()
                    .filter(|n| matches!(n.update, SessionUpdate::AgentThoughtChunk(_)))
                    .count();

                assert_eq!(
                    thought_count, 0,
                    "no thoughts when thinking_enabled=false (default from no options)"
                );
            })
            .await;
    }

    #[tokio::test]
    async fn echo_prefix_in_text_response() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let handler = make_handler();
                *handler.opts.borrow_mut() = AgentOptions {
                    thinking_enabled: false,
                    echo_prefix: "Echo".to_string(),
                    ..AgentOptions::default()
                };

                let parts = vec![ContentBlock::Text(TextContent::new("hello"))];
                let block = echo_content_block(&parts[0], "Echo");
                match block {
                    ContentBlock::Text(t) => assert_eq!(t.text, "Echo: hello"),
                    _ => panic!("expected text"),
                }
            })
            .await;
    }

    #[tokio::test]
    async fn echo_image_block_unchanged() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                use agent_client_protocol::ImageContent;
                let img = ContentBlock::Image(ImageContent::new("base64data", "image/png"));
                let result = echo_content_block(&img, "Echo");
                match result {
                    ContentBlock::Image(i) => {
                        assert_eq!(i.data, "base64data");
                        assert_eq!(i.mime_type, "image/png");
                    }
                    _ => panic!("expected image"),
                }
            })
            .await;
    }

    #[tokio::test]
    async fn echo_audio_block_unchanged() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                use agent_client_protocol::AudioContent;
                let audio = ContentBlock::Audio(AudioContent::new("audio_data", "audio/mpeg"));
                let result = echo_content_block(&audio, "Echo");
                match result {
                    ContentBlock::Audio(a) => {
                        assert_eq!(a.data, "audio_data");
                        assert_eq!(a.mime_type, "audio/mpeg");
                    }
                    _ => panic!("expected audio"),
                }
            })
            .await;
    }

    #[tokio::test]
    async fn echo_resource_link_as_text() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                use agent_client_protocol::ResourceLink;
                let link = ContentBlock::ResourceLink(ResourceLink::new(
                    "report.pdf",
                    "https://example.com/report.pdf",
                ));
                let result = echo_content_block(&link, "Echo");
                match result {
                    ContentBlock::Text(t) => {
                        assert!(t.text.contains("report.pdf"), "should contain uri");
                    }
                    _ => panic!("expected text"),
                }
            })
            .await;
    }

    #[test]
    fn agent_options_from_meta_defaults() {
        let opts = AgentOptions::from_meta(None);
        assert!(!opts.thinking_enabled);
        assert_eq!(opts.echo_prefix, "Echo");
        assert!(!opts.request_permission);
        assert!(!opts.reject_load);
        assert!(!opts.support_resume);
    }

    #[test]
    fn agent_options_from_meta_custom() {
        let mut meta = serde_json::Map::new();
        let mut options = serde_json::Map::new();
        options.insert("exit_after".into(), serde_json::json!(3));
        options.insert("thinking".into(), serde_json::json!(false));
        options.insert("echo_prefix".into(), serde_json::json!("Bot"));
        options.insert("request_permission".into(), serde_json::json!(true));
        options.insert("support_resume".into(), serde_json::json!(true));
        meta.insert("options".into(), serde_json::Value::Object(options));

        let opts = AgentOptions::from_meta(Some(&meta));
        assert_eq!(opts.exit_after, Some(3));
        assert!(!opts.thinking_enabled);
        assert_eq!(opts.echo_prefix, "Bot");
        assert!(opts.request_permission);
        assert!(opts.support_resume);
    }

    #[tokio::test]
    async fn new_session_stores_session_id() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let handler = make_handler();
                let req = NewSessionRequest::new("/workspace").mcp_servers(vec![]);
                let resp = handler.new_session(req).await.expect("new_session");
                assert!(!resp.session_id.to_string().is_empty());
                assert_eq!(
                    handler.session_id.borrow().as_deref(),
                    Some(resp.session_id.to_string().as_str())
                );
            })
            .await;
    }

    #[tokio::test]
    async fn new_session_counts_mcp_servers() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                use agent_client_protocol::{McpServer, McpServerHttp};
                let handler = make_handler();
                let servers = vec![McpServer::Http(McpServerHttp::new(
                    "tools",
                    "http://127.0.0.1:1234/mcp",
                ))];
                let req = NewSessionRequest::new("/workspace").mcp_servers(servers);
                handler.new_session(req).await.expect("new_session");
                assert_eq!(handler.mcp_server_count.get(), 1);
            })
            .await;
    }

    #[tokio::test]
    async fn load_session_returns_ok_by_default() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                use agent_client_protocol::LoadSessionRequest;
                let handler = make_handler();
                *handler.session_id.borrow_mut() = Some("existing-sid".to_string());
                let req = LoadSessionRequest::new("existing-sid", "/workspace");
                let result = handler.load_session(req).await;
                assert!(result.is_ok());
            })
            .await;
    }

    #[tokio::test]
    async fn load_session_rejected_when_option_set() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                use agent_client_protocol::LoadSessionRequest;
                let handler = make_handler();
                handler.opts.borrow_mut().reject_load = true;
                let req = LoadSessionRequest::new("sid", "/workspace");
                let result = handler.load_session(req).await;
                assert!(result.is_err());
            })
            .await;
    }
}
