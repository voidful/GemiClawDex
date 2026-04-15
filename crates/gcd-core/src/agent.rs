// GemiClawDex — Agent Loop
//
// Core execution loop: prompt → API → parse tool calls → execute → loop.
// Supports Gemini, OpenAI, and Anthropic with full tool calling.
// Features: permission prompt, streaming, context window management,
// token tracking, git integration, planning mode, MCP, plugins,
// coordinator sub-agents, provider fallback, and undercover mode.

mod adapters;
pub mod memory;
mod permissions;
mod runtime_support;
mod types;

use crate::mcp;
use crate::tools::{all_tools, ToolContext};
use adapters::{call_provider_api, execute_tool_call};
use permissions::{check_always_upgrade, needs_confirmation};
use runtime_support::{
    compact_messages, estimate_total_tokens, git_auto_commit_safe, CONTEXT_WINDOW_BUDGET,
};

pub use types::{
    AgentEvent, AgentMessage, AgentRunOptions, AgentRunResult, IdeContext, PermissionLevel, TokenUsage,
    ToolCall, ToolInvocationRecord,
};

/// Run the agent loop to completion.
///
/// `plugin_tools` are dynamically loaded from `.gcd/plugins/` and passed by the caller.
pub async fn run_agent(
    mut options: AgentRunOptions,
    plugin_tools: Vec<Box<dyn crate::tools::Tool>>,
) -> anyhow::Result<AgentRunResult> {
    // Load MCP tools from workspace configuration
    let mcp_tools = mcp::load_mcp_tools(&options.workspace_root);

    // Merge builtin + plugin + MCP tools
    let tools = all_tools(plugin_tools, mcp_tools);

    let hooks_config = crate::hooks::HooksConfig::load(
        &options.workspace_root.join(".config").join("gcd"),
        &options.workspace_root,
    );

    let tool_ctx = ToolContext {
        workspace_root: options.workspace_root.clone(),
        sandbox: options.sandbox.clone(),
        coordinator_provider: Some(options.provider.clone()),
        coordinator_api_key: Some(options.api_key.clone()),
        coordinator_permission: Some(options.permission.clone()),
        coordinator_fallback_providers: options.fallback_providers.clone(),
        coordinator_plugin_definitions: options.plugin_definitions.clone(),
        coordinator_depth: options.coordinator_depth,
        coordinator_prompt_context: inherited_prompt_context(&options.initial_prompt),
        hooks: hooks_config,
    };

    let initial_prompt = if options.planning {
        format!(
            "You are in PLANNING MODE. Before executing any changes:\n\
             1. Analyze the task thoroughly\n\
             2. List all files that need to be modified\n\
             3. Describe the changes for each file\n\
             4. Ask for user confirmation before proceeding\n\n\
             Task: {}",
            options.initial_prompt
        )
    } else {
        options.initial_prompt.clone()
    };

    let mut messages = vec![AgentMessage {
        role: "user".to_string(),
        content: initial_prompt,
        tool_calls: None,
        tool_call_id: None,
    }];
    let mut tool_records = Vec::new();
    let mut final_response = String::new();
    let mut token_usage = TokenUsage::default();
    let mut previous_response_id: Option<String> = None;
    let mut events = vec![AgentEvent::RunStarted {
        provider_id: options.provider.id.clone(),
        model: options.provider.model.clone(),
        protocol: options.provider.protocol.as_str().to_string(),
        sandbox: options.sandbox.as_str().to_string(),
        permission: options.permission.as_str().to_string(),
        max_turns: options.max_turns,
        planning: options.planning,
        streaming: options.streaming,
    }];

    let mut turns_executed = 0usize;
    for turn in 0..options.max_turns {
        let turn_number = turn + 1;
        turns_executed = turn_number;
        if !options.quiet {
            eprintln!(
                "\x1b[2m[Turn {}/{}] Calling {} ({})...\x1b[0m",
                turn_number,
                options.max_turns,
                options.provider.model,
                options.provider.family.as_str(),
            );
        }
        events.push(AgentEvent::TurnStarted {
            turn: turn_number,
            max_turns: options.max_turns,
            message_count: messages.len(),
        });

        let estimated_tokens = estimate_total_tokens(&messages);
        if estimated_tokens > CONTEXT_WINDOW_BUDGET {
            if !options.quiet {
                eprintln!(
                    "\x1b[33m⚡ Context window near limit (~{} tokens). Compacting...\x1b[0m",
                    estimated_tokens
                );
            }
            compact_messages(&mut messages);
            events.push(AgentEvent::ContextCompacted {
                turn: turn_number,
                estimated_tokens,
                budget: CONTEXT_WINDOW_BUDGET,
            });
        }

        events.push(AgentEvent::ProviderCalled {
            turn: turn_number,
            protocol: options.provider.protocol.as_str().to_string(),
            message_count: messages.len(),
        });

        // call_provider_api now supports fallback
        let (response, usage, response_id) = call_provider_api(
            &options,
            &messages,
            &tools,
            previous_response_id.as_deref(),
            turn_number,
            &mut events,
        )
        .await?;
        token_usage.add(usage.0, usage.1);
        previous_response_id = response_id;
        events.push(AgentEvent::AssistantMessage {
            turn: turn_number,
            content: response.content.clone(),
            tool_call_count: response.tool_calls.as_ref().map_or(0, Vec::len),
        });

        if let Some(calls) = &response.tool_calls {
            messages.push(response.clone());

            for call in calls {
                events.push(AgentEvent::ToolCallRequested {
                    turn: turn_number,
                    call_id: call.id.clone(),
                    tool_name: call.name.clone(),
                    arguments: call.arguments.clone(),
                });
                if needs_confirmation(&call.name, &options.permission) {
                    let (allowed, upgrade) = if let Some(handler) = &options.approval_handler {
                        handler.request_approval(call).await
                    } else {
                        check_always_upgrade(call)
                    };
                    if upgrade {
                        options.permission = PermissionLevel::FullAuto;
                    }
                    if !allowed {
                        if !options.quiet {
                            eprintln!("\x1b[31m✗ Denied: {}\x1b[0m", call.name);
                        }
                        events.push(AgentEvent::ToolCallDenied {
                            turn: turn_number,
                            call_id: call.id.clone(),
                            tool_name: call.name.clone(),
                        });
                        messages.push(AgentMessage {
                            role: "tool".to_string(),
                            content: serde_json::json!({"error": "User denied this tool call"})
                                .to_string(),
                            tool_calls: None,
                            tool_call_id: Some(call.id.clone()),
                        });
                        continue;
                    }
                }

                // Run PreToolUse hooks
                let hook_result = crate::hooks::run_pre_tool_hooks(
                    &tool_ctx.hooks,
                    &call.name,
                    &call.arguments,
                    &tool_ctx.workspace_root,
                );
                if let crate::hooks::PreToolHookResult::Deny { stderr, .. } = hook_result {
                    if !options.quiet {
                        eprintln!("\x1b[31m🪝 Hook denied: {}\x1b[0m", call.name);
                    }
                    events.push(AgentEvent::ToolCallDenied {
                        turn: turn_number,
                        call_id: call.id.clone(),
                        tool_name: call.name.clone(),
                    });
                    messages.push(AgentMessage {
                        role: "tool".to_string(),
                        content:
                            serde_json::json!({"error": format!("Hook denied: {}", stderr.trim())})
                                .to_string(),
                        tool_calls: None,
                        tool_call_id: Some(call.id.clone()),
                    });
                    continue;
                }

                if !options.quiet {
                    eprintln!("\x1b[32m⚡ Executing: {}\x1b[0m", call.name);
                }
                let tool_outcome = execute_tool_call(call, &tools, &tool_ctx).await;

                // Run PostToolUse hooks
                crate::hooks::run_post_tool_hooks(
                    &tool_ctx.hooks,
                    &call.name,
                    &call.arguments,
                    &tool_outcome.result,
                    &tool_ctx.workspace_root,
                );

                events.extend(tool_outcome.emitted_events.clone());

                events.push(AgentEvent::ToolCallCompleted {
                    turn: turn_number,
                    call_id: call.id.clone(),
                    tool_name: call.name.clone(),
                    result: tool_outcome.result.clone(),
                });

                tool_records.push(ToolInvocationRecord {
                    turn,
                    tool_name: call.name.clone(),
                    arguments: call.arguments.clone(),
                    result: tool_outcome.result.clone(),
                });

                messages.push(AgentMessage {
                    role: "tool".to_string(),
                    content: serde_json::to_string(&tool_outcome.result).unwrap_or_default(),
                    tool_calls: None,
                    tool_call_id: Some(call.id.clone()),
                });
            }
        } else {
            final_response = response.content.clone();
            messages.push(response);
            break;
        }
    }

    // Git auto-commit with undercover mode (sanitizes commit messages for public repos)
    if options.auto_git {
        git_auto_commit_safe(&options.workspace_root, &final_response);
    }

    if !options.quiet {
        eprintln!();
        eprintln!(
            "\x1b[2m📊 Token usage: {} prompt + {} completion = {} total ({} API calls)\x1b[0m",
            token_usage.prompt_tokens,
            token_usage.completion_tokens,
            token_usage.total_tokens,
            token_usage.api_calls,
        );
    }
    events.push(AgentEvent::RunCompleted {
        turns_used: turns_executed,
        tool_invocation_count: tool_records.len(),
        total_tokens: token_usage.total_tokens,
        api_calls: token_usage.api_calls,
        final_response: final_response.clone(),
    });

    Ok(AgentRunResult {
        turns_used: turns_executed,
        final_response,
        messages,
        tool_invocations: tool_records,
        token_usage,
        events,
    })
}

fn inherited_prompt_context(prompt: &str) -> Option<String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some((prefix, _)) = trimmed.split_once("\n\n# Task\n") {
        let prefix = prefix.trim();
        if !prefix.is_empty() {
            return Some(prefix.to_string());
        }
    }

    Some(trimmed.to_string())
}
