use async_trait::async_trait;
use protoclaw_sdk_tool::{Tool, ToolSdkError, ToolServer};

struct SystemInfoTool;

#[async_trait]
impl Tool for SystemInfoTool {
    fn name(&self) -> &str {
        "system-info"
    }

    fn description(&self) -> &str {
        "Returns system information about the protoclaw host"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(&self, _input: serde_json::Value) -> Result<serde_json::Value, ToolSdkError> {
        Ok(serde_json::json!({
            "hostname": std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into()),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "protoclaw_version": env!("CARGO_PKG_VERSION")
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ToolServer::new(vec![Box::new(SystemInfoTool)]).serve_stdio().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name() {
        let tool = SystemInfoTool;
        assert_eq!(tool.name(), "system-info");
    }

    #[test]
    fn tool_description_not_empty() {
        let tool = SystemInfoTool;
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn tool_input_schema_is_object() {
        let tool = SystemInfoTool;
        let schema = tool.input_schema();
        assert!(schema.is_object());
        assert_eq!(schema["type"], "object");
    }

    #[tokio::test]
    async fn tool_execute_returns_hostname() {
        let tool = SystemInfoTool;
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.get("hostname").is_some());
    }

    #[tokio::test]
    async fn tool_execute_returns_os() {
        let tool = SystemInfoTool;
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.get("os").is_some());
        assert!(!result["os"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn tool_execute_returns_arch() {
        let tool = SystemInfoTool;
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.get("arch").is_some());
    }

    #[tokio::test]
    async fn tool_execute_returns_version() {
        let tool = SystemInfoTool;
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.get("protoclaw_version").is_some());
    }
}
