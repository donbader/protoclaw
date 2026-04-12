use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::error::ChannelSdkError;
use crate::trait_def::Channel;
use protoclaw_sdk_types::{
    ChannelInitializeParams, ChannelInitializeResult, ChannelRequestPermission, ChannelSendMessage,
    DeliverMessage, SessionCreated,
};

/// JSON-RPC stdio harness that drives a [`Channel`] implementation.
///
/// Handles line-delimited JSON framing, the initialize handshake, and
/// bidirectional message routing between stdin/stdout and the channel.
pub struct ChannelHarness<C: Channel> {
    channel: C,
}

impl<C: Channel> ChannelHarness<C> {
    /// Wrap a [`Channel`] implementation for harness-driven execution.
    pub fn new(channel: C) -> Self {
        Self { channel }
    }

    /// Run the harness event loop over real stdio (stdin/stdout).
    pub async fn run_stdio(self) -> Result<(), ChannelSdkError> {
        self.run(tokio::io::stdin(), tokio::io::stdout()).await
    }

    /// Run the harness event loop over the given async reader and writer.
    pub async fn run<R, W>(mut self, reader: R, mut writer: W) -> Result<(), ChannelSdkError>
    where
        R: tokio::io::AsyncRead + Unpin + Send,
        W: tokio::io::AsyncWrite + Unpin + Send,
    {
        let mut lines = BufReader::new(reader).lines();
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<ChannelSendMessage>(64);

        loop {
            tokio::select! {
                line_result = lines.next_line() => {
                    match line_result {
                        Ok(Some(line)) => {
                            if line.trim().is_empty() {
                                continue;
                            }
                            let msg: serde_json::Value = match serde_json::from_str(&line) {
                                Ok(v) => v,
                                Err(_) => continue,
                            };
                            if let Some(response) = self.dispatch(msg, &outbound_tx).await? {
                                Self::write_line(&mut writer, &response).await?;
                            }
                        }
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
                Some(send_msg) = outbound_rx.recv() => {
                    let params = serde_json::to_value(&send_msg).unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "failed to serialize channel/sendMessage params, using null");
                        serde_json::Value::default()
                    });
                    let notification = serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "channel/sendMessage",
                        "params": params,
                    });
                    Self::write_line(&mut writer, &notification).await?;
                }
            }
        }

        Ok(())
    }

    async fn write_line<W: tokio::io::AsyncWrite + Unpin>(
        writer: &mut W,
        msg: &serde_json::Value,
    ) -> Result<(), ChannelSdkError> {
        let mut line = serde_json::to_vec(msg)?;
        line.push(b'\n');
        writer.write_all(&line).await?;
        writer.flush().await?;
        Ok(())
    }

    async fn dispatch(
        &mut self,
        msg: serde_json::Value,
        outbound_tx: &mpsc::Sender<ChannelSendMessage>,
    ) -> Result<Option<serde_json::Value>, ChannelSdkError> {
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = msg.get("id").cloned();
        let params = msg
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        match method {
            "initialize" => {
                let caps = self.channel.capabilities();
                let result = ChannelInitializeResult {
                    protocol_version: 1,
                    capabilities: caps,
                };
                if let Ok(init_params) = serde_json::from_value::<ChannelInitializeParams>(params) {
                    if init_params.protocol_version != 1 {
                        tracing::warn!(
                            protocol_version = init_params.protocol_version,
                            "channel received unexpected protocol version; expected 1"
                        );
                    }
                    self.channel.on_initialize(init_params).await?;
                }
                self.channel.on_ready(outbound_tx.clone()).await?;
                if let Some(req_id) = id {
                    return Ok(Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": serde_json::to_value(&result)?,
                    })));
                }
            }
            "channel/deliverMessage" => {
                if let Ok(deliver) = serde_json::from_value::<DeliverMessage>(params) {
                    self.channel.deliver_message(deliver).await?;
                }
            }
            "channel/sessionCreated" => {
                if let Ok(msg) = serde_json::from_value::<SessionCreated>(params) {
                    self.channel.on_session_created(msg).await?;
                }
            }
            "channel/requestPermission" => {
                if let Ok(req) = serde_json::from_value::<ChannelRequestPermission>(params) {
                    let resp = self.channel.request_permission(req).await?;
                    if let Some(req_id) = id {
                        return Ok(Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": req_id,
                            "result": serde_json::to_value(&resp)?,
                        })));
                    }
                }
            }
            _ => {
                let result = self.channel.handle_unknown(method, params).await;
                if let Some(req_id) = id {
                    return Ok(Some(match result {
                        Ok(val) => serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": req_id,
                            "result": val,
                        }),
                        Err(e) => serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": req_id,
                            "error": {"code": -32601, "message": e.to_string()},
                        }),
                    }));
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protoclaw_sdk_types::{ChannelCapabilities, PermissionResponse};
    use rstest::rstest;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct TestChannel {
        on_ready_called: Arc<Mutex<bool>>,
        delivered: Arc<Mutex<Vec<DeliverMessage>>>,
        permission_response: PermissionResponse,
    }

    impl TestChannel {
        fn new() -> Self {
            Self {
                on_ready_called: Arc::new(Mutex::new(false)),
                delivered: Arc::new(Mutex::new(Vec::new())),
                permission_response: PermissionResponse {
                    request_id: String::new(),
                    option_id: "allow".into(),
                },
            }
        }
    }

    impl Channel for TestChannel {
        fn capabilities(&self) -> ChannelCapabilities {
            ChannelCapabilities {
                streaming: true,
                rich_text: false,
            }
        }

        async fn on_ready(
            &mut self,
            _outbound: mpsc::Sender<ChannelSendMessage>,
        ) -> Result<(), ChannelSdkError> {
            *self.on_ready_called.lock().unwrap() = true;
            Ok(())
        }

        async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
            self.delivered.lock().unwrap().push(msg);
            Ok(())
        }

        async fn request_permission(
            &mut self,
            req: ChannelRequestPermission,
        ) -> Result<PermissionResponse, ChannelSdkError> {
            Ok(PermissionResponse {
                request_id: req.request_id,
                ..self.permission_response.clone()
            })
        }
    }

    fn make_jsonrpc_request(id: u64, method: &str, params: serde_json::Value) -> String {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        format!("{}\n", serde_json::to_string(&msg).unwrap())
    }

    fn make_jsonrpc_notification(method: &str, params: serde_json::Value) -> String {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        format!("{}\n", serde_json::to_string(&msg).unwrap())
    }

    fn parse_responses(output: &[u8]) -> Vec<serde_json::Value> {
        let text = String::from_utf8_lossy(output);
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    #[tokio::test]
    async fn when_channel_harness_created_then_constructs_successfully() {
        let ch = TestChannel::new();
        let _harness = ChannelHarness::new(ch);
    }

    #[tokio::test]
    async fn when_initialize_request_received_then_harness_responds_with_capabilities_and_calls_on_ready()
     {
        let ch = TestChannel::new();
        let on_ready_called = ch.on_ready_called.clone();

        let input =
            make_jsonrpc_request(1, "initialize", serde_json::json!({"protocolVersion": 1}));
        let reader = std::io::Cursor::new(input.into_bytes());
        let mut output = Vec::new();

        let harness = ChannelHarness::new(ch);
        harness.run(reader, &mut output).await.unwrap();

        assert!(*on_ready_called.lock().unwrap());

        let responses = parse_responses(&output);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0]["id"], 1);
        assert_eq!(responses[0]["result"]["capabilities"]["streaming"], true);
        assert_eq!(responses[0]["result"]["capabilities"]["richText"], false);
        assert_eq!(responses[0]["result"]["protocolVersion"], 1);
    }

    #[tokio::test]
    async fn when_deliver_message_notification_received_then_harness_calls_channel_deliver_message()
    {
        let ch = TestChannel::new();
        let delivered = ch.delivered.clone();

        let mut input =
            make_jsonrpc_request(1, "initialize", serde_json::json!({"protocolVersion": 1}));
        input.push_str(&make_jsonrpc_notification(
            "channel/deliverMessage",
            serde_json::json!({"sessionId": "s1", "content": "hello"}),
        ));

        let reader = std::io::Cursor::new(input.into_bytes());
        let mut output = Vec::new();

        let harness = ChannelHarness::new(ch);
        harness.run(reader, &mut output).await.unwrap();

        let msgs = delivered.lock().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].session_id, "s1");
        assert_eq!(msgs[0].content, "hello");
    }

    #[tokio::test]
    async fn when_request_permission_received_then_harness_calls_channel_and_sends_response() {
        let ch = TestChannel::new();

        let mut input =
            make_jsonrpc_request(1, "initialize", serde_json::json!({"protocolVersion": 1}));
        input.push_str(&make_jsonrpc_request(
            2,
            "channel/requestPermission",
            serde_json::json!({
                "requestId": "perm-1",
                "sessionId": "s1",
                "description": "Allow?",
                "options": [{"optionId": "allow", "label": "Allow"}]
            }),
        ));

        let reader = std::io::Cursor::new(input.into_bytes());
        let mut output = Vec::new();

        let harness = ChannelHarness::new(ch);
        harness.run(reader, &mut output).await.unwrap();

        let responses = parse_responses(&output);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[1]["id"], 2);
        assert_eq!(responses[1]["result"]["requestId"], "perm-1");
        assert_eq!(responses[1]["result"]["optionId"], "allow");
    }

    #[tokio::test]
    async fn when_unknown_method_received_then_harness_calls_handle_unknown_and_returns_error() {
        let ch = TestChannel::new();

        let mut input =
            make_jsonrpc_request(1, "initialize", serde_json::json!({"protocolVersion": 1}));
        input.push_str(&make_jsonrpc_request(
            2,
            "custom/method",
            serde_json::json!({}),
        ));

        let reader = std::io::Cursor::new(input.into_bytes());
        let mut output = Vec::new();

        let harness = ChannelHarness::new(ch);
        harness.run(reader, &mut output).await.unwrap();

        let responses = parse_responses(&output);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[1]["id"], 2);
        assert!(
            responses[1]["error"]["message"]
                .as_str()
                .unwrap()
                .contains("custom/method")
        );
    }

    #[tokio::test]
    async fn when_reader_reaches_eof_then_harness_exits_cleanly() {
        let ch = TestChannel::new();
        let reader = std::io::Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let harness = ChannelHarness::new(ch);
        let result = harness.run(reader, &mut output).await;
        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn when_channel_tester_initialized_then_on_ready_called() {
        use crate::testing::ChannelTester;

        let ch = TestChannel::new();
        let on_ready_called = ch.on_ready_called.clone();
        let mut tester = ChannelTester::new(ch);

        tester.initialize(None).await.unwrap();
        assert!(*on_ready_called.lock().unwrap());
    }

    #[rstest]
    #[tokio::test]
    async fn when_channel_tester_delivers_message_then_channel_receives_it() {
        use crate::testing::ChannelTester;

        let ch = TestChannel::new();
        let delivered = ch.delivered.clone();
        let mut tester = ChannelTester::new(ch);
        tester.initialize(None).await.unwrap();

        tester
            .deliver(DeliverMessage {
                session_id: "s1".into(),
                content: serde_json::json!("test-msg"),
            })
            .await
            .unwrap();

        let msgs = delivered.lock().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].session_id, "s1");
    }
}
