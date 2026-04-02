// GemiClawdex — Tool System
//
// Defines the Tool trait and built-in tools for the agent loop.
// Inspired by gemini-cli's file/shell tools and codex's sandbox model.

use std::fs;

use std::path::{Path, PathBuf};
use std::process::Command;


use serde_json::Value;

use crate::config::SandboxPolicy;

// ---------------------------------------------------------------------------
// Tool trait
// ---------------------------------------------------------------------------

/// Every tool the agent can invoke implements this trait.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Machine-readable name used in function-call payloads.
    fn name(&self) -> &str;

    /// Human-readable one-liner shown in prompt tool descriptions.
    fn description(&self) -> &str;

    /// JSON Schema describing accepted parameters (used by the model).
    fn parameters_schema(&self) -> Value;

    /// Execute the tool and return a JSON result.
    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value>;
}

/// Shared context passed to every tool invocation.
#[derive(Clone, Debug)]
pub struct ToolContext {
    pub workspace_root: PathBuf,
    pub sandbox: SandboxPolicy,
}

impl ToolContext {
    /// Check whether a path is within the workspace boundary.
    pub fn is_within_workspace(&self, path: &Path) -> bool {
        let Ok(canonical) = fs::canonicalize(path) else {
            return false;
        };
        let Ok(root) = fs::canonicalize(&self.workspace_root) else {
            return false;
        };
        canonical.starts_with(root)
    }

    /// Return the absolute path resolved relative to workspace root.
    pub fn resolve_path(&self, raw: &str) -> PathBuf {
        let p = PathBuf::from(raw);
        if p.is_absolute() {
            p
        } else {
            self.workspace_root.join(p)
        }
    }
}

// ---------------------------------------------------------------------------
// Built-in tools
// ---------------------------------------------------------------------------

/// Read the contents of a file.
pub struct ReadFileTool;

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }

    fn description(&self) -> &str {
        "Read the full contents of a file within the workspace."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative or absolute path to read" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let path_str = params["path"].as_str().unwrap_or("");
        let path = ctx.resolve_path(path_str);

        if !ctx.is_within_workspace(&path) {
            anyhow::bail!("Path is outside the workspace: {}", path.display());
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))?;

        Ok(serde_json::json!({
            "path": path.display().to_string(),
            "content": content,
            "size_bytes": content.len()
        }))
    }
}

/// Write content to a file. Respects sandbox policy.
pub struct WriteFileTool;

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }

    fn description(&self) -> &str {
        "Create or overwrite a file within the workspace."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative or absolute path to write" },
                "content": { "type": "string", "description": "Content to write" }
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
        let path = ctx.resolve_path(path_str);

        if !matches!(ctx.sandbox, SandboxPolicy::Off) && !ctx.is_within_workspace(&path) {
            anyhow::bail!("Path is outside the workspace: {}", path.display());
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;

        Ok(serde_json::json!({
            "path": path.display().to_string(),
            "bytes_written": content.len()
        }))
    }
}

/// List the contents of a directory.
pub struct ListDirTool;

#[async_trait::async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str { "list_dir" }

    fn description(&self) -> &str {
        "List files and subdirectories within a given directory."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory path to list" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let path_str = params["path"].as_str().unwrap_or(".");
        let path = ctx.resolve_path(path_str);

        let mut entries = Vec::new();
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let ft = entry.file_type()?;
            entries.push(serde_json::json!({
                "name": name,
                "is_dir": ft.is_dir(),
                "is_file": ft.is_file(),
            }));
        }

        Ok(serde_json::json!({
            "path": path.display().to_string(),
            "entries": entries
        }))
    }
}

/// Execute a shell command. Respects sandbox policy.
pub struct ShellTool;

#[async_trait::async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str { "shell" }

    fn description(&self) -> &str {
        "Execute a shell command in the workspace directory."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Shell command to execute" }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        if matches!(ctx.sandbox, SandboxPolicy::ReadOnly | SandboxPolicy::Container) {
            anyhow::bail!("shell is blocked by sandbox policy: {}", ctx.sandbox.as_str());
        }

        let cmd_str = params["command"].as_str().unwrap_or("");
        if cmd_str.is_empty() {
            anyhow::bail!("command parameter is required");
        }

        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd_str)
            .current_dir(&ctx.workspace_root)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(serde_json::json!({
            "exit_code": output.status.code().unwrap_or(-1),
            "stdout": stdout,
            "stderr": stderr,
        }))
    }
}

/// Search for patterns within files (grep-like).
pub struct SearchFilesTool;

#[async_trait::async_trait]
impl Tool for SearchFilesTool {
    fn name(&self) -> &str { "search_files" }

    fn description(&self) -> &str {
        "Search for a text pattern in files within the workspace using grep."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Search pattern (regex supported)" },
                "path": { "type": "string", "description": "Directory to search in (default: workspace root)" },
                "include": { "type": "string", "description": "Glob pattern to filter files (e.g. '*.rs')" }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let pattern = params["pattern"].as_str().unwrap_or("");
        let search_path = params["path"].as_str().unwrap_or(".");
        let include = params["include"].as_str();

        let resolved = ctx.resolve_path(search_path);

        // Try ripgrep first, fall back to grep
        let mut cmd = if which_exists("rg") {
            let mut c = Command::new("rg");
            c.arg("--no-heading").arg("--line-number").arg("--max-count=50");
            if let Some(glob) = include {
                c.arg("--glob").arg(glob);
            }
            c.arg(pattern).arg(&resolved);
            c
        } else {
            let mut c = Command::new("grep");
            c.arg("-rn").arg("--max-count=50");
            if let Some(glob) = include {
                c.arg("--include").arg(glob);
            }
            c.arg(pattern).arg(&resolved);
            c
        };

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        let matches: Vec<Value> = stdout
            .lines()
            .take(50)
            .map(|line| serde_json::json!(line))
            .collect();

        Ok(serde_json::json!({
            "pattern": pattern,
            "match_count": matches.len(),
            "matches": matches,
        }))
    }
}

/// Return the default set of built-in tools.
pub fn builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ReadFileTool),
        Box::new(WriteFileTool),
        Box::new(ListDirTool),
        Box::new(ShellTool),
        Box::new(SearchFilesTool),
    ]
}

/// Produce the tool-declaration array for inclusion in API requests.
pub fn tools_declaration(tools: &[Box<dyn Tool>]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name(),
                "description": t.description(),
                "parameters": t.parameters_schema()
            })
        })
        .collect()
}

fn which_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
