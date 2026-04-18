use std::collections::HashMap;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::formatting::escape_html;

const COLLAPSED_MAX_LEN: usize = 60;
const LARGE_VALUE_THRESHOLD: usize = 200;

enum ArgsSummary {
    /// No args to display.
    None,
    /// Single short arg — display inline: `key: value`
    Collapsed(String),
    /// Multiple args or long values — display as `key: val, key: val`
    Expanded(String),
}

/// Format tool input args into a display summary.
///
/// Rules:
/// - `null`/empty/non-object → `None`
/// - 1 arg with value ≤ 60 chars → `Collapsed("key: value")`
/// - Multiple args or any value > 60 chars → `Expanded("key: val, key: val")`
/// - String values > 200 chars → show `(N.Nkb)` instead of content
#[allow(clippy::disallowed_types)]
fn format_tool_args(input: &Option<serde_json::Value>) -> ArgsSummary {
    let Some(serde_json::Value::Object(map)) = input else {
        return ArgsSummary::None;
    };
    if map.is_empty() {
        return ArgsSummary::None;
    }

    let pairs: Vec<(String, String)> = map
        .iter()
        .map(|(k, v)| (k.clone(), format_arg_value(v)))
        .collect();

    if pairs.len() == 1 {
        let summary = format!("{}: {}", pairs[0].0, pairs[0].1);
        if summary.len() <= COLLAPSED_MAX_LEN {
            return ArgsSummary::Collapsed(summary);
        }
    }

    let summary = pairs
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join(", ");
    ArgsSummary::Expanded(summary)
}

#[allow(clippy::disallowed_types)]
fn format_arg_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => {
            if s.len() > LARGE_VALUE_THRESHOLD {
                let kb = s.len() as f64 / 1024.0;
                format!("({kb:.1}kb)")
            } else {
                s.clone()
            }
        }
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Array(arr) => format!("[{} items]", arr.len()),
        serde_json::Value::Object(obj) => format!("{{{} keys}}", obj.len()),
    }
}

pub struct ThoughtTrack {
    pub msg_id: i32,
    pub started_at: Instant,
    pub buffer: String,
    pub debounce_handle: Option<JoinHandle<()>>,
    pub suppressed: bool,
}

pub struct ResponseTrack {
    pub msg_id: i32,
    pub buffer: String,
    pub last_edit: Instant,
    /// Debounce handle for the first response send — lets chunks accumulate
    /// before creating the Telegram message.
    pub debounce_handle: Option<JoinHandle<()>>,
}

pub enum ToolCallStatus {
    Started,
    InProgress,
    Completed,
    Failed(Option<String>),
}

impl ToolCallStatus {
    /// A terminal status cannot regress to an earlier state.
    /// Once a tool call is `Completed` or `Failed`, subsequent
    /// `in_progress` heartbeats are stale and must be discarded.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed(_))
    }
}

pub struct ToolCallTrack {
    pub name: String,
    pub status: ToolCallStatus,
    // D-03: tool input schema is tool-defined — no fixed Rust type possible.
    #[allow(clippy::disallowed_types)]
    pub input: Option<serde_json::Value>,
}

pub enum TurnPhase {
    Active,
    Finalizing(JoinHandle<()>),
}

pub struct ChatTurn {
    pub message_id: String,
    pub phase: TurnPhase,
    pub thought: Option<ThoughtTrack>,
    pub response: Option<ResponseTrack>,
    pub tool_calls: HashMap<String, ToolCallTrack>,
    /// Telegram message ID for the single combined tools message in this turn.
    pub tools_msg_id: i32,
    /// Insertion-ordered tool_call_ids so the combined message preserves call order.
    pub tool_call_order: Vec<String>,
    /// Incremented on each render — drives dot animation for in-progress tools.
    pub render_tick: u32,
    pub last_result_was_error: bool,
    /// Last time the tools message was edited — used for edit cooldown.
    pub last_tools_edit: Instant,
}

impl ChatTurn {
    pub fn new(message_id: String) -> Self {
        Self {
            message_id,
            phase: TurnPhase::Active,
            thought: None,
            response: None,
            tool_calls: HashMap::new(),
            tools_msg_id: 0,
            tool_call_order: Vec::new(),
            render_tick: 0,
            last_result_was_error: false,
            last_tools_edit: Instant::now() - Duration::from_secs(60),
        }
    }

    /// Render all tracked tool calls as a combined HTML text block.
    ///
    /// Layout per tool:
    /// ```text
    /// 🔧 tool_name ...        (loading — dots animate)
    ///    path: /src/main.rs
    ///
    /// 🔧 tool_name ✅         (completed)
    ///    path: /src/main.rs
    ///
    /// 🔧 tool_name ❌         (failed)
    ///    error message
    /// ```
    pub fn render_tools_text(&mut self) -> String {
        self.render_tick = self.render_tick.wrapping_add(1);
        let dots = match self.render_tick % 3 {
            0 => ".",
            1 => "..",
            _ => "...",
        };

        let mut blocks = Vec::with_capacity(self.tool_call_order.len());
        for id in &self.tool_call_order {
            let Some(track) = self.tool_calls.get(id) else {
                continue;
            };
            let name = escape_html(&track.name);

            let (status_suffix, detail_line) = match &track.status {
                ToolCallStatus::Started | ToolCallStatus::InProgress => {
                    let detail = match format_tool_args(&track.input) {
                        ArgsSummary::None => String::new(),
                        ArgsSummary::Collapsed(s) | ArgsSummary::Expanded(s) => {
                            format!("\n   {}", escape_html(&s))
                        }
                    };
                    (dots.to_string(), detail)
                }
                ToolCallStatus::Completed => {
                    let detail = match format_tool_args(&track.input) {
                        ArgsSummary::None => String::new(),
                        ArgsSummary::Collapsed(s) | ArgsSummary::Expanded(s) => {
                            format!("\n   {}", escape_html(&s))
                        }
                    };
                    ("✅".to_string(), detail)
                }
                ToolCallStatus::Failed(output) => {
                    let detail = match output {
                        Some(err) => format!("\n   <pre>{}</pre>", escape_html(err)),
                        None => String::new(),
                    };
                    ("❌".to_string(), detail)
                }
            };

            blocks.push(format!(
                "🔧 <code>{name}</code> {status_suffix}{detail_line}"
            ));
        }
        blocks.join("\n")
    }

    pub fn append_response(&mut self, text: &str, msg_id: i32) {
        match &mut self.response {
            Some(track) => {
                track.buffer.push_str(text);
            }
            None => {
                self.response = Some(ResponseTrack {
                    msg_id,
                    buffer: text.to_string(),
                    last_edit: Instant::now(),
                    debounce_handle: None,
                });
            }
        }
    }

    pub fn can_edit_response(&mut self, cooldown: Duration) -> bool {
        match &mut self.response {
            Some(track) => {
                if track.last_edit.elapsed() < cooldown {
                    return false;
                }
                track.last_edit = Instant::now();
                true
            }
            None => false,
        }
    }

    pub fn can_edit_tools(&mut self, cooldown: Duration) -> bool {
        if self.last_tools_edit.elapsed() < cooldown {
            return false;
        }
        self.last_tools_edit = Instant::now();
        true
    }

    pub fn append_thought(&mut self, text: &str, msg_id: i32, origin_time: Option<Instant>) {
        match &mut self.thought {
            Some(track) => {
                track.buffer.push_str(text);
            }
            None => {
                self.thought = Some(ThoughtTrack {
                    msg_id,
                    started_at: origin_time.unwrap_or_else(Instant::now),
                    buffer: text.to_string(),
                    debounce_handle: None,
                    suppressed: false,
                });
            }
        }
    }

    pub fn begin_finalizing(&mut self, handle: JoinHandle<()>) {
        if let TurnPhase::Finalizing(old) = &self.phase {
            old.abort();
        }
        self.phase = TurnPhase::Finalizing(handle);
    }

    pub fn take_response_for_finalize(&mut self) -> Option<(String, i32)> {
        self.response.as_ref().map(|r| (r.buffer.clone(), r.msg_id))
    }

    pub fn is_finalizing(&self) -> bool {
        matches!(self.phase, TurnPhase::Finalizing(_))
    }

    pub fn is_different_turn(&self, message_id: &str) -> bool {
        self.message_id != message_id
    }

    pub fn collapse_thought(&mut self) -> Option<(i32, f32)> {
        let track = self.thought.take()?;
        if let Some(h) = &track.debounce_handle {
            h.abort();
        }
        Some((track.msg_id, track.started_at.elapsed().as_secs_f32()))
    }

    pub fn cleanup(&mut self) {
        if let TurnPhase::Finalizing(handle) = &self.phase {
            handle.abort();
        }
        if let Some(ref track) = self.thought
            && let Some(ref h) = track.debounce_handle
        {
            h.abort();
        }
        if let Some(ref track) = self.response
            && let Some(ref h) = track.debounce_handle
        {
            h.abort();
        }
        self.thought = None;
        self.response = None;
        self.tool_calls.clear();
        self.tools_msg_id = 0;
        self.tool_call_order.clear();
        self.render_tick = 0;
        self.last_result_was_error = false;
        self.phase = TurnPhase::Active;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn given_no_turn_when_thought_arrives_then_turn_created() {
        let turn = ChatTurn::new("msg-1".to_string());
        assert_eq!(turn.message_id, "msg-1");
        assert!(matches!(turn.phase, TurnPhase::Active));
        assert!(turn.thought.is_none());
        assert!(turn.response.is_none());
    }

    #[rstest]
    fn when_thought_appended_then_buffer_accumulates() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_thought("hello ", 42, None);
        turn.append_thought("world", 42, None);
        let track = turn.thought.as_ref().unwrap();
        assert_eq!(track.buffer, "hello world");
        assert_eq!(track.msg_id, 42);
    }

    #[rstest]
    fn when_response_appended_then_buffer_accumulates() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("hello ", 100);
        turn.append_response("world", 100);
        let track = turn.response.as_ref().unwrap();
        assert_eq!(track.buffer, "hello world");
        assert_eq!(track.msg_id, 100);
    }

    #[rstest]
    fn when_can_edit_checked_within_cooldown_then_false() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("text", 100);
        // last_edit is Instant::now(), so within 1s cooldown
        assert!(!turn.can_edit_response(Duration::from_millis(1000)));
    }

    #[rstest]
    #[tokio::test]
    async fn given_response_after_cooldown_when_can_edit_then_true() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.response = Some(ResponseTrack {
            msg_id: 100,
            buffer: "text".to_string(),
            last_edit: Instant::now() - Duration::from_secs(2),
            debounce_handle: None,
        });
        assert!(turn.can_edit_response(Duration::from_millis(1000)));
    }

    #[rstest]
    #[tokio::test]
    async fn when_result_received_then_phase_becomes_finalizing() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("hello world", 100);
        let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(10)).await });
        turn.begin_finalizing(handle);
        assert!(matches!(turn.phase, TurnPhase::Finalizing(_)));
    }

    #[rstest]
    fn when_finalized_then_response_buffer_returned() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("hello world", 100);
        let (response_text, response_msg_id) = turn.take_response_for_finalize().unwrap();
        assert_eq!(response_text, "hello world");
        assert_eq!(response_msg_id, 100);
    }

    #[rstest]
    #[tokio::test]
    async fn when_cleanup_called_then_handles_aborted_and_state_cleared() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_thought("thinking", 42, None);
        turn.append_response("text", 100);
        let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(10)).await });
        turn.begin_finalizing(handle);
        turn.cleanup();
        assert!(turn.thought.is_none());
        assert!(turn.response.is_none());
        assert!(matches!(turn.phase, TurnPhase::Active));
    }

    #[rstest]
    fn given_different_message_id_when_checked_then_is_new_turn() {
        let turn = ChatTurn::new("msg-1".to_string());
        assert!(turn.is_different_turn("msg-2"));
    }

    #[rstest]
    fn given_same_message_id_when_checked_then_not_new_turn() {
        let turn = ChatTurn::new("msg-1".to_string());
        assert!(!turn.is_different_turn("msg-1"));
    }

    #[rstest]
    fn when_stale_result_checked_then_detected() {
        let turn = ChatTurn::new("msg-2".to_string());
        assert!(turn.is_different_turn("msg-1"));
    }

    #[rstest]
    fn when_thought_collapsed_then_returns_elapsed_and_clears() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.thought = Some(ThoughtTrack {
            msg_id: 42,
            started_at: Instant::now(),
            buffer: "thinking...".to_string(),
            debounce_handle: None,
            suppressed: false,
        });
        let collapsed = turn.collapse_thought();
        assert!(collapsed.is_some());
        let (msg_id, elapsed_secs) = collapsed.unwrap();
        assert_eq!(msg_id, 42);
        assert!(elapsed_secs < 1.0);
        assert!(turn.thought.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn given_finalizing_turn_when_cleanup_without_take_then_response_lost() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("full response text", 100);
        let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(10)).await });
        turn.begin_finalizing(handle);

        // BUG: calling cleanup without take_response_for_finalize loses the buffer
        turn.cleanup();
        assert!(
            turn.take_response_for_finalize().is_none(),
            "response data lost after cleanup without take"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn given_finalizing_turn_when_take_before_cleanup_then_response_preserved() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("full response text", 100);
        let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(10)).await });
        turn.begin_finalizing(handle);

        // CORRECT: take response before cleanup preserves the data
        let data = turn.take_response_for_finalize();
        assert!(data.is_some(), "response must be available before cleanup");
        let (text, msg_id) = data.unwrap();
        assert_eq!(text, "full response text");
        assert_eq!(msg_id, 100);

        turn.cleanup();
        assert!(turn.take_response_for_finalize().is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn given_active_turn_with_response_when_new_turn_forces_cleanup_then_response_preserved()
    {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("partial response from agent", 100);

        // Simulate rapid-fire: new turn arrives while old turn is Active with buffered response
        assert!(turn.is_different_turn("msg-2"));

        // CORRECT pattern: take before cleanup
        let data = turn.take_response_for_finalize();
        assert!(data.is_some());
        let (text, _) = data.unwrap();
        assert_eq!(text, "partial response from agent");

        turn.cleanup();
    }

    #[rstest]
    #[tokio::test]
    async fn given_late_chunk_when_finalizing_then_buffer_grows() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("hello ", 100);
        let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(10)).await });
        turn.begin_finalizing(handle);
        turn.append_response("world", 100);
        let (text, _) = turn.take_response_for_finalize().unwrap();
        assert_eq!(text, "hello world");
        assert!(matches!(turn.phase, TurnPhase::Finalizing(_)));
    }

    #[rstest]
    fn when_render_tools_text_then_preserves_insertion_order() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "read_file".to_string(),
                status: ToolCallStatus::Started,
                input: None,
            },
        );
        turn.tool_call_order.push("tc-2".to_string());
        turn.tool_calls.insert(
            "tc-2".to_string(),
            ToolCallTrack {
                name: "write_file".to_string(),
                status: ToolCallStatus::Completed,
                input: None,
            },
        );
        let text = turn.render_tools_text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].contains("read_file"),
            "first line should be read_file, got: {}",
            lines[0]
        );
        assert!(
            lines[1].contains("write_file"),
            "second line should be write_file, got: {}",
            lines[1]
        );
    }

    #[rstest]
    #[case::started(ToolCallStatus::Started, ".")]
    #[case::in_progress(ToolCallStatus::InProgress, ".")]
    #[case::completed(ToolCallStatus::Completed, "✅")]
    #[case::failed_no_output(ToolCallStatus::Failed(None), "❌")]
    fn when_render_tools_text_then_status_suffix_correct(
        #[case] status: ToolCallStatus,
        #[case] expected_suffix: &str,
    ) {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "my_tool".to_string(),
                status,
                input: None,
            },
        );
        let text = turn.render_tools_text();
        assert!(
            text.starts_with("🔧"),
            "all tools must start with 🔧, got: {text}"
        );
        assert!(
            text.contains(expected_suffix),
            "expected suffix {expected_suffix}, got: {text}"
        );
    }

    #[rstest]
    fn when_render_tools_text_failed_with_output_then_error_shown() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "fs_read".to_string(),
                status: ToolCallStatus::Failed(Some("path not found".to_string())),
                input: None,
            },
        );
        let text = turn.render_tools_text();
        assert!(
            text.contains("❌"),
            "failed status must use ❌, got: {text}"
        );
        assert!(
            text.contains("<pre>path not found</pre>"),
            "error output must be in <pre>, got: {text}"
        );
    }

    #[rstest]
    fn when_render_tools_text_then_html_in_name_escaped() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "<script>alert(1)</script>".to_string(),
                status: ToolCallStatus::Completed,
                input: None,
            },
        );
        let text = turn.render_tools_text();
        assert!(
            !text.contains("<script>"),
            "HTML in tool name must be escaped, got: {text}"
        );
        assert!(
            text.contains("&lt;script&gt;"),
            "angle brackets must be escaped, got: {text}"
        );
    }

    #[rstest]
    fn when_render_tools_text_failed_with_html_in_output_then_escaped() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "tool".to_string(),
                status: ToolCallStatus::Failed(Some("<b>bad</b>".to_string())),
                input: None,
            },
        );
        let text = turn.render_tools_text();
        assert!(
            !text.contains("<b>bad</b>"),
            "HTML in error output must be escaped, got: {text}"
        );
        assert!(
            text.contains("&lt;b&gt;bad&lt;/b&gt;"),
            "angle brackets must be escaped, got: {text}"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_cleanup_called_then_tool_fields_cleared() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tools_msg_id = 42;
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "tool".to_string(),
                status: ToolCallStatus::Completed,
                input: None,
            },
        );
        turn.cleanup();
        assert_eq!(turn.tools_msg_id, 0);
        assert!(turn.tool_call_order.is_empty());
        assert!(turn.tool_calls.is_empty());
    }

    #[rstest]
    fn when_tool_has_single_short_arg_then_shown_on_detail_line() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "read_file".to_string(),
                status: ToolCallStatus::Completed,
                input: Some(serde_json::json!({"path": "/src/main.rs"})),
            },
        );
        let text = turn.render_tools_text();
        assert!(text.starts_with("🔧"), "must start with 🔧, got: {text}");
        assert!(text.contains("✅"), "completed must show ✅, got: {text}");
        assert!(
            text.contains("path: /src/main.rs"),
            "arg must appear on detail line, got: {text}"
        );
    }

    #[rstest]
    fn when_tool_has_multiple_args_then_shown_on_detail_line() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "edit_file".to_string(),
                status: ToolCallStatus::Completed,
                input: Some(serde_json::json!({"path": "/src/lib.rs", "old": "fn main()", "new": "fn run()"})),
            },
        );
        let text = turn.render_tools_text();
        assert!(
            text.contains("\n   "),
            "args must be on indented detail line, got: {text}"
        );
        assert!(text.contains("path:"), "must contain arg keys, got: {text}");
    }

    #[rstest]
    fn when_tool_has_large_string_arg_then_shows_size() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        let large_content = "x".repeat(500);
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "write_file".to_string(),
                status: ToolCallStatus::Completed,
                input: Some(serde_json::json!({"path": "/src/lib.rs", "content": large_content})),
            },
        );
        let text = turn.render_tools_text();
        assert!(
            text.contains("kb)"),
            "large string values must show size in kb, got: {text}"
        );
        assert!(
            !text.contains(&large_content),
            "large content must not appear verbatim"
        );
    }

    #[rstest]
    fn when_tool_has_no_input_then_no_detail_line() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "system_info".to_string(),
                status: ToolCallStatus::Completed,
                input: None,
            },
        );
        let text = turn.render_tools_text();
        assert!(
            !text.contains('\n'),
            "no-input tool must be single line, got: {text}"
        );
    }

    #[rstest]
    fn when_render_called_multiple_times_then_dots_cycle() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.tool_call_order.push("tc-1".to_string());
        turn.tool_calls.insert(
            "tc-1".to_string(),
            ToolCallTrack {
                name: "slow_tool".to_string(),
                status: ToolCallStatus::InProgress,
                input: None,
            },
        );
        let t1 = turn.render_tools_text();
        let t2 = turn.render_tools_text();
        let t3 = turn.render_tools_text();
        assert_ne!(t1, t2, "dots must change between renders");
        assert_ne!(t2, t3, "dots must change between renders");
        let t4 = turn.render_tools_text();
        assert_eq!(t1, t4, "dots must cycle every 3 renders");
    }

    #[rstest]
    #[case::started(ToolCallStatus::Started, false)]
    #[case::in_progress(ToolCallStatus::InProgress, false)]
    #[case::completed(ToolCallStatus::Completed, true)]
    #[case::failed_none(ToolCallStatus::Failed(None), true)]
    #[case::failed_some(ToolCallStatus::Failed(Some("err".into())), true)]
    fn when_status_checked_then_terminal_correct(
        #[case] status: ToolCallStatus,
        #[case] expected: bool,
    ) {
        assert_eq!(status.is_terminal(), expected);
    }

    #[rstest]
    fn given_active_turn_when_is_finalizing_then_false() {
        let turn = ChatTurn::new("msg-1".to_string());
        assert!(!turn.is_finalizing());
    }

    #[rstest]
    #[tokio::test]
    async fn given_finalizing_turn_when_is_finalizing_then_true() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(10)).await });
        turn.begin_finalizing(handle);
        assert!(turn.is_finalizing());
    }

    #[rstest]
    fn when_can_edit_tools_within_cooldown_then_false() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        assert!(turn.can_edit_tools(Duration::from_millis(1000)));
        assert!(!turn.can_edit_tools(Duration::from_millis(1000)));
    }

    #[rstest]
    #[tokio::test]
    async fn given_tools_edit_after_cooldown_when_can_edit_tools_then_true() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.last_tools_edit = Instant::now() - Duration::from_secs(2);
        assert!(turn.can_edit_tools(Duration::from_millis(1000)));
    }
}
