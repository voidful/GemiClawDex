use std::fs;

use serde_json::Value;

use crate::config::SandboxPolicy;

use super::common::simple_diff;
use super::{Tool, ToolContext};

pub struct WriteFileTool;

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Create or overwrite a file, subject to the active sandbox policy. Returns a diff of changes."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative or absolute path to write" },
                "content": { "type": "string", "description": "Content to write" },
                "is_artifact": { "type": "boolean", "description": "Set to true if creating an artifact." },
                "artifact_metadata": {
                    "type": "object",
                    "properties": {
                        "artifact_type": { "type": "string" },
                        "summary": { "type": "string" }
                    }
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        if matches!(ctx.sandbox, SandboxPolicy::ReadOnly) {
            anyhow::bail!("write_file is blocked by read-only sandbox policy");
        }

        let path_str = params["path"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");
        let path = if matches!(ctx.sandbox, SandboxPolicy::Off) {
            ctx.resolve_path(path_str)
        } else {
            ctx.workspace_path(&ctx.resolve_path(path_str))?
        };

        let diff = if path.exists() {
            let old = fs::read_to_string(&path).unwrap_or_default();
            simple_diff(&old, content)
        } else {
            format!("+++ new file: {}\n", path.display())
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;

        let mut res = serde_json::json!({
            "path": path.display().to_string(),
            "bytes_written": content.len(),
            "diff_preview": diff
        });

        if params.get("is_artifact").and_then(|v| v.as_bool()).unwrap_or(false) {
            let meta = params.get("artifact_metadata").cloned().unwrap_or(serde_json::Value::Null);
            let artifact_type = meta.get("artifact_type").and_then(|v| v.as_str()).unwrap_or("other").to_string();
            let summary = meta.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
            
            res = super::embed_tool_events(res, vec![
                crate::agent::AgentEvent::ArtifactUpdated {
                    path: path.display().to_string(),
                    artifact_type,
                    summary,
                }
            ]);
        }

        Ok(res)
    }
}
