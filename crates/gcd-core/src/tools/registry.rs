use serde_json::Value;

use super::apply_patch::ApplyPatchTool;
use super::browser_subagent::BrowserSubagentTool;
use super::coordinator::CoordinatorTool;
use super::fetch_url::FetchUrlTool;
use super::list_dir::ListDirTool;
use super::memory_tool::MemoryTool;
use super::read_file::ReadFileTool;
use super::search_files::SearchFilesTool;
use super::session_search::SessionSearchTool;
use super::shell::ShellTool;
use super::skill_manager::SkillManagerTool;
use super::write_file::WriteFileTool;
use super::Tool;

/// Returns the hardcoded built-in tools (no plugins or MCP).
pub fn builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ReadFileTool),
        Box::new(WriteFileTool),
        Box::new(ListDirTool),
        Box::new(ShellTool),
        Box::new(SearchFilesTool),
        Box::new(FetchUrlTool),
        Box::new(ApplyPatchTool),
        Box::new(CoordinatorTool),
        Box::new(MemoryTool),
        Box::new(SkillManagerTool),
        Box::new(SessionSearchTool),
        Box::new(BrowserSubagentTool),
    ]
}

/// Merges built-in tools with dynamically loaded tools (plugins + MCP).
pub fn all_tools(
    plugin_tools: Vec<Box<dyn Tool>>,
    mcp_tools: Vec<Box<dyn Tool>>,
) -> Vec<Box<dyn Tool>> {
    let mut tools = builtin_tools();
    tools.extend(plugin_tools);
    tools.extend(mcp_tools);
    tools
}

pub fn tools_declaration(tools: &[Box<dyn Tool>]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "name": tool.name(),
                "description": tool.description(),
                "parameters": tool.parameters_schema()
            })
        })
        .collect()
}
