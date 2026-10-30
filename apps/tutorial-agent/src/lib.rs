pub mod chapters;
pub mod runtime;
pub mod ui;

use a3s_code_core::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use serde_json::{Value, json};

pub struct ProjectLabel;

#[async_trait::async_trait]
impl Tool for ProjectLabel {
    fn name(&self) -> &str {
        "project_label"
    }
    fn description(&self) -> &str {
        "返回练习工程的固定标识，不读取文件或网络。"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{},"additionalProperties":false})
    }
    fn capabilities(&self, _: &Value) -> ToolCapabilities {
        ToolCapabilities::parallel_safe_read(1)
    }
    async fn execute(&self, args: &Value, _: &ToolContext) -> anyhow::Result<ToolOutput> {
        if !args.as_object().is_some_and(|map| map.is_empty()) {
            return Ok(ToolOutput::error("project_label 不接受参数"));
        }
        Ok(ToolOutput::success("tutorial-demo"))
    }
}
