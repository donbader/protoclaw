use async_trait::async_trait;
use protoclaw_sdk_tool::{Tool, ToolSdkError, ToolServer};

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echoes the input message back as output"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo back"
                }
            },
            "required": ["message"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolSdkError> {
        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Ok(serde_json::json!({ "echo": message }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ToolServer::new(vec![Box::new(EchoTool)])
        .serve_stdio()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name() {
        let tool = EchoTool;
        assert_eq!(tool.name(), "echo");
    }

    #[test]
    fn tool_description_not_empty() {
        let tool = EchoTool;
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn tool_input_schema_is_object() {
        let tool = EchoTool;
        let schema = tool.input_schema();
        assert!(schema.is_object());
        assert_eq!(schema["type"], "object");
    }

    #[tokio::test]
    async fn execute_echoes_message() {
        let tool = EchoTool;
        let result = tool
            .execute(serde_json::json!({ "message": "hello world" }))
            .await
            .expect("execute should succeed");
        assert_eq!(result["echo"], "hello world");
    }

    #[tokio::test]
    async fn execute_empty_message() {
        let tool = EchoTool;
        let result = tool
            .execute(serde_json::json!({ "message": "" }))
            .await
            .expect("execute should succeed");
        assert_eq!(result["echo"], "");
    }

    #[tokio::test]
    async fn execute_missing_message_field_uses_empty() {
        let tool = EchoTool;
        let result = tool
            .execute(serde_json::json!({}))
            .await
            .expect("execute should succeed");
        assert_eq!(result["echo"], "");
    }
}
