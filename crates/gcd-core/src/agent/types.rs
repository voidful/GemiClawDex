use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::SandboxPolicy;
use crate::plugins::PluginDefinition;
use crate::providers::ProviderProfile;

#[async_trait::async_trait]
pub trait ApprovalHandler: Send + Sync + std::fmt::Debug {
    async fn request_approval(&self, call: &ToolCall) -> (bool, bool);
}

const DEFAULT_MAX_TURNS: usize = 10;
const CHARS_PER_TOKEN: usize = 4;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PermissionLevel {
    Suggest,
    AutoEdit,
    FullAuto,
}

impl PermissionLevel {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "suggest" => Some(Self::Suggest),
            "auto-edit" | "auto_edit" => Some(Self::AutoEdit),
            "full-auto" | "full_auto" => Some(Self::FullAuto),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Suggest => "suggest",
            Self::AutoEdit => "auto-edit",
            Self::FullAuto => "full-auto",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TokenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub api_calls: usize,
}

impl TokenUsage {
    pub(crate) fn add(&mut self, prompt: usize, completion: usize) {
        self.prompt_tokens += prompt;
        self.completion_tokens += completion;
        self.total_tokens += prompt + completion;
        self.api_calls += 1;
    }

    pub(crate) fn estimate_tokens(text: &str) -> usize {
        text.len() / CHARS_PER_TOKEN
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IdeContext {
    pub active_file: Option<String>,
    pub cursor_line: Option<usize>,
    pub open_files: Option<Vec<String>>,
    pub selected_text: Option<String>,
    pub browser_state: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AgentRunOptions {
    pub provider: ProviderProfile,
    pub workspace_root: PathBuf,
    pub sandbox: SandboxPolicy,
    pub initial_prompt: String,
    pub max_turns: usize,
    pub api_key: String,
    pub permission: PermissionLevel,
    pub streaming: bool,
    pub auto_git: bool,
    pub planning: bool,
    /// Fallback providers tried in order if primary fails (429/500/503/timeout)
    pub fallback_providers: Vec<(ProviderProfile, String)>,
    /// Loaded plugin definitions inherited by delegated sub-agents.
    pub plugin_definitions: Vec<PluginDefinition>,
    /// Current sub-agent nesting depth for bounded delegation.
    pub coordinator_depth: usize,
    /// Suppress runtime progress logs for nested/sub-agent runs.
    pub quiet: bool,
    /// IDE context from the frontend extension or web application.
    pub ide_context: Option<IdeContext>,
    /// Optional async handler for dynamically approving tool executions.
    pub approval_handler: Option<std::sync::Arc<dyn ApprovalHandler>>,
}

impl AgentRunOptions {
    pub fn with_defaults(
        provider: ProviderProfile,
        workspace: PathBuf,
        prompt: String,
        api_key: String,
    ) -> Self {
        Self {
            sandbox: SandboxPolicy::WorkspaceWrite,
            workspace_root: workspace,
            initial_prompt: prompt,
            max_turns: DEFAULT_MAX_TURNS,
            provider,
            api_key,
            permission: PermissionLevel::AutoEdit,
            streaming: true,
            auto_git: false,
            planning: false,
            fallback_providers: Vec::new(),
            plugin_definitions: Vec::new(),
            coordinator_depth: 0,
            quiet: false,
            ide_context: None,
            approval_handler: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    ExecutionPrepared {
        mode: String,
        workspace: String,
        provider_id: String,
        provider_label: String,
        protocol: String,
        sandbox: String,
        trust: String,
        active_command: Option<String>,
        active_skill: Option<String>,
        prompt: String,
        attachment_count: usize,
        pending_shell_command_count: usize,
        source_session_id: Option<String>,
        source_mode: Option<String>,
    },
    CheckpointSaved {
        path: String,
    },
    RunStarted {
        provider_id: String,
        model: String,
        protocol: String,
        sandbox: String,
        permission: String,
        max_turns: usize,
        planning: bool,
        streaming: bool,
    },
    TurnStarted {
        turn: usize,
        max_turns: usize,
        message_count: usize,
    },
    ContextCompacted {
        turn: usize,
        estimated_tokens: usize,
        budget: usize,
    },
    ProviderCalled {
        turn: usize,
        protocol: String,
        message_count: usize,
    },
    AssistantMessage {
        turn: usize,
        content: String,
        tool_call_count: usize,
    },
    ToolCallRequested {
        turn: usize,
        call_id: String,
        tool_name: String,
        arguments: Value,
    },
    ToolCallDenied {
        turn: usize,
        call_id: String,
        tool_name: String,
    },
    ToolCallCompleted {
        turn: usize,
        call_id: String,
        tool_name: String,
        result: Value,
    },
    CoordinatorStarted {
        depth: usize,
        execution_mode: String,
        task_count: usize,
        max_concurrency: usize,
    },
    CoordinatorBatchStarted {
        depth: usize,
        batch: usize,
        total_batches: usize,
        tasks: Vec<String>,
    },
    CoordinatorTaskStarted {
        depth: usize,
        batch: usize,
        task_name: String,
        depends_on: Vec<String>,
    },
    CoordinatorTaskBlocked {
        depth: usize,
        batch: usize,
        task_name: String,
        blocked_by: Vec<String>,
    },
    CoordinatorTaskCompleted {
        depth: usize,
        batch: usize,
        task_name: String,
        status: String,
        turns_used: usize,
        tool_call_count: usize,
        total_tokens: usize,
        summary: String,
    },
    CoordinatorCompleted {
        depth: usize,
        execution_mode: String,
        completed_count: usize,
        failed_count: usize,
        blocked_count: usize,
        total_tokens: usize,
        api_calls: usize,
    },
    RunCompleted {
        turns_used: usize,
        tool_invocation_count: usize,
        total_tokens: usize,
        api_calls: usize,
        final_response: String,
    },
    SessionPersisted {
        action: String,
        id: String,
        turn_count: usize,
    },
    ProviderFallback {
        turn: usize,
        from_provider: String,
        to_provider: String,
        reason: String,
    },
    ArtifactUpdated {
        path: String,
        artifact_type: String,
        summary: String,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentRunResult {
    pub turns_used: usize,
    pub final_response: String,
    pub messages: Vec<AgentMessage>,
    pub tool_invocations: Vec<ToolInvocationRecord>,
    pub token_usage: TokenUsage,
    pub events: Vec<AgentEvent>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolInvocationRecord {
    pub turn: usize,
    pub tool_name: String,
    pub arguments: Value,
    pub result: Value,
}
