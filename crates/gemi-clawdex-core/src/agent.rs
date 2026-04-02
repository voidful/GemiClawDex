// GemiClawdex — Agent Loop
//
// Core execution loop: prompt → API → parse tool calls → execute → loop.
// Supports Gemini GenerateContent protocol as the primary adapter.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::SandboxPolicy;
use crate::providers::ProviderProfile;
use crate::tools::{builtin_tools, ToolContext, Tool};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Maximum number of agent turns before forced stop (safety limit).
const DEFAULT_MAX_TURNS: usize = 10;

/// Options controlling a single agent execution run.
#[derive(Clone, Debug)]
pub struct AgentRunOptions {
    pub provider: ProviderProfile,
    pub workspace_root: PathBuf,
    pub sandbox: SandboxPolicy,
    pub initial_prompt: String,
    pub max_turns: usize,
    pub api_key: String,
}

impl AgentRunOptions {
    pub fn with_defaults(provider: ProviderProfile, workspace: PathBuf, prompt: String, api_key: String) -> Self {
        Self {
            sandbox: SandboxPolicy::WorkspaceWrite,
            workspace_root: workspace,
            initial_prompt: prompt,
            max_turns: DEFAULT_MAX_TURNS,
            provider,
            api_key,
        }
    }
}

// ---------------------------------------------------------------------------
// Agent state & messages
// ---------------------------------------------------------------------------

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

/// The result of a complete agent run.
#[derive(Clone, Debug, Serialize)]
pub struct AgentRunResult {
    pub turns_used: usize,
    pub final_response: String,
    pub messages: Vec<AgentMessage>,
    pub tool_invocations: Vec<ToolInvocationRecord>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolInvocationRecord {
    pub turn: usize,
    pub tool_name: String,
    pub arguments: Value,
    pub result: Value,
}

// ---------------------------------------------------------------------------
// Agent execution
// ---------------------------------------------------------------------------

/// Run the agent loop to completion.
pub async fn run_agent(options: AgentRunOptions) -> anyhow::Result<AgentRunResult> {
    let tools = builtin_tools();
    let tool_ctx = ToolContext {
        workspace_root: options.workspace_root.clone(),
        sandbox: options.sandbox.clone(),
    };

    let mut messages = vec![AgentMessage {
        role: "user".to_string(),
        content: options.initial_prompt.clone(),
        tool_calls: None,
        tool_call_id: None,
    }];

    let mut tool_records = Vec::new();
    let mut final_response = String::new();

    for turn in 0..options.max_turns {
        // 1. Call the provider API
        let response = call_provider_api(&options, &messages, &tools).await?;

        // 2. Check if the model wants to call tools
        if let Some(ref calls) = response.tool_calls {
            messages.push(response.clone());

            // 3. Execute each tool call
            for call in calls {
                let result = execute_tool_call(call, &tools, &tool_ctx).await;

                tool_records.push(ToolInvocationRecord {
                    turn,
                    tool_name: call.name.clone(),
                    arguments: call.arguments.clone(),
                    result: result.clone(),
                });

                // 4. Feed tool result back as a message
                messages.push(AgentMessage {
                    role: "tool".to_string(),
                    content: serde_json::to_string(&result).unwrap_or_default(),
                    tool_calls: None,
                    tool_call_id: Some(call.id.clone()),
                });
            }
            // Continue the loop — model will process tool results
        } else {
            // No tool calls — this is the final text response
            final_response = response.content.clone();
            messages.push(response);
            break;
        }
    }

    Ok(AgentRunResult {
        turns_used: tool_records.len().max(1),
        final_response,
        messages,
        tool_invocations: tool_records,
    })
}

// ---------------------------------------------------------------------------
// Provider API adapters
// ---------------------------------------------------------------------------

/// Send messages to the provider and parse the response.
/// Currently implements the Gemini GenerateContent protocol.
async fn call_provider_api(
    options: &AgentRunOptions,
    messages: &[AgentMessage],
    tools: &[Box<dyn Tool>],
) -> anyhow::Result<AgentMessage> {
    match options.provider.family {
        crate::providers::ProviderFamily::Gemini => {
            call_gemini_api(options, messages, tools).await
        }
        crate::providers::ProviderFamily::OpenAiCompatible => {
            call_openai_api(options, messages, tools).await
        }
        crate::providers::ProviderFamily::Anthropic => {
            call_anthropic_api(options, messages, tools).await
        }
    }
}

/// Gemini GenerateContent API adapter.
async fn call_gemini_api(
    options: &AgentRunOptions,
    messages: &[AgentMessage],
    tools: &[Box<dyn Tool>],
) -> anyhow::Result<AgentMessage> {
    let url = format!(
        "{}/v1beta/models/{}:generateContent?key={}",
        options.provider.api_base, options.provider.model, options.api_key
    );

    // Convert messages to Gemini format
    let contents: Vec<Value> = messages
        .iter()
        .map(|msg| {
            let role = match msg.role.as_str() {
                "user" => "user",
                "assistant" | "model" => "model",
                "tool" => "user", // tool results go back as user messages in Gemini
                _ => "user",
            };
            serde_json::json!({
                "role": role,
                "parts": [{ "text": msg.content }]
            })
        })
        .collect();

    // Build tool declarations for Gemini format
    let tool_decls: Vec<Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name(),
                "description": t.description(),
                "parameters": t.parameters_schema()
            })
        })
        .collect();

    let body = serde_json::json!({
        "contents": contents,
        "tools": [{ "functionDeclarations": tool_decls }],
        "generationConfig": {
            "temperature": 0.2,
            "maxOutputTokens": 8192
        }
    });

    let client = reqwest::Client::new();
    let resp = client.post(&url).json(&body).send().await?;
    let status = resp.status();
    let resp_text = resp.text().await?;

    if !status.is_success() {
        anyhow::bail!("Gemini API error ({}): {}", status, resp_text);
    }

    let resp_json: Value = serde_json::from_str(&resp_text)?;
    parse_gemini_response(&resp_json)
}

/// Parse a Gemini generateContent response into an AgentMessage.
fn parse_gemini_response(resp: &Value) -> anyhow::Result<AgentMessage> {
    let candidate = &resp["candidates"][0];
    let parts = candidate["content"]["parts"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No parts in Gemini response"))?;

    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for (idx, part) in parts.iter().enumerate() {
        if let Some(text) = part["text"].as_str() {
            text_parts.push(text.to_string());
        }
        if let Some(fc) = part.get("functionCall") {
            let name = fc["name"].as_str().unwrap_or("").to_string();
            let args = fc.get("args").cloned().unwrap_or(Value::Object(Default::default()));
            tool_calls.push(ToolCall {
                id: format!("call_{}", idx),
                name,
                arguments: args,
            });
        }
    }

    Ok(AgentMessage {
        role: "assistant".to_string(),
        content: text_parts.join(""),
        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
        tool_call_id: None,
    })
}

/// OpenAI Chat/Responses API adapter (scaffold).
async fn call_openai_api(
    options: &AgentRunOptions,
    messages: &[AgentMessage],
    _tools: &[Box<dyn Tool>],
) -> anyhow::Result<AgentMessage> {
    let url = format!("{}/chat/completions", options.provider.api_base);

    let msgs: Vec<Value> = messages
        .iter()
        .map(|msg| {
            serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            })
        })
        .collect();

    let body = serde_json::json!({
        "model": options.provider.model,
        "messages": msgs,
        "temperature": 0.2,
        "max_tokens": 4096,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", options.api_key))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let resp_text = resp.text().await?;

    if !status.is_success() {
        anyhow::bail!("OpenAI API error ({}): {}", status, resp_text);
    }

    let resp_json: Value = serde_json::from_str(&resp_text)?;
    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(AgentMessage {
        role: "assistant".to_string(),
        content,
        tool_calls: None,
        tool_call_id: None,
    })
}

/// Anthropic Messages API adapter (scaffold).
async fn call_anthropic_api(
    options: &AgentRunOptions,
    messages: &[AgentMessage],
    _tools: &[Box<dyn Tool>],
) -> anyhow::Result<AgentMessage> {
    let url = format!("{}/v1/messages", options.provider.api_base);

    let msgs: Vec<Value> = messages
        .iter()
        .map(|msg| {
            serde_json::json!({
                "role": if msg.role == "assistant" { "assistant" } else { "user" },
                "content": msg.content,
            })
        })
        .collect();

    let body = serde_json::json!({
        "model": options.provider.model,
        "messages": msgs,
        "max_tokens": 4096,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("x-api-key", &options.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let resp_text = resp.text().await?;

    if !status.is_success() {
        anyhow::bail!("Anthropic API error ({}): {}", status, resp_text);
    }

    let resp_json: Value = serde_json::from_str(&resp_text)?;
    let content = resp_json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(AgentMessage {
        role: "assistant".to_string(),
        content,
        tool_calls: None,
        tool_call_id: None,
    })
}

// ---------------------------------------------------------------------------
// Tool dispatch
// ---------------------------------------------------------------------------

async fn execute_tool_call(
    call: &ToolCall,
    tools: &[Box<dyn Tool>],
    ctx: &ToolContext,
) -> Value {
    let tool = tools.iter().find(|t| t.name() == call.name);

    match tool {
        Some(t) => match t.execute(call.arguments.clone(), ctx).await {
            Ok(result) => result,
            Err(e) => serde_json::json!({ "error": e.to_string() }),
        },
        None => serde_json::json!({ "error": format!("Unknown tool: {}", call.name) }),
    }
}
