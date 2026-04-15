// GemiClawDex — Provider Adapters
//
// Dispatches API calls to the correct protocol adapter (Gemini, OpenAI, Anthropic).
// Includes fallback logic for retryable errors and tool call execution.

mod anthropic;
mod gemini;
mod openai;
mod sse;

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{json, Value};

use crate::providers::ProviderProtocol;
use crate::tools::{Tool, ToolContext};

use super::types::{AgentEvent, AgentMessage, AgentRunOptions, ToolCall};

/// Default request timeout for all provider API calls (5 minutes).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
/// Default connect timeout (10 seconds).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Shared HTTP client — reuses connections across all API calls.
fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .pool_max_idle_per_host(4)
            .build()
            .expect("failed to build HTTP client")
    })
}

/// Attempt to call the primary provider, falling back to alternates on retryable errors.
pub(super) async fn call_provider_api(
    options: &AgentRunOptions,
    messages: &[AgentMessage],
    tools: &[Box<dyn Tool>],
    previous_response_id: Option<&str>,
    turn: usize,
    events: &mut Vec<AgentEvent>,
) -> anyhow::Result<(AgentMessage, (usize, usize), Option<String>)> {
    // Try the primary provider first
    let result = call_provider_api_inner(options, messages, tools, previous_response_id).await;

    match &result {
        Ok(_) => result,
        Err(err) => {
            let err_str = err.to_string();
            // Only fallback on retryable errors (rate limit, server error, timeout)
            if !is_retryable_error(&err_str) || options.fallback_providers.is_empty() {
                return result;
            }

            eprintln!(
                "\x1b[33m⚠️  Provider '{}' failed: {}. Trying fallbacks...\x1b[0m",
                options.provider.id, err_str
            );

            for (fallback_provider, fallback_key) in &options.fallback_providers {
                let mut fallback_options = options.clone();
                fallback_options.provider = fallback_provider.clone();
                fallback_options.api_key = fallback_key.clone();

                events.push(AgentEvent::ProviderFallback {
                    turn,
                    from_provider: options.provider.id.clone(),
                    to_provider: fallback_provider.id.clone(),
                    reason: err_str.clone(),
                });

                eprintln!(
                    "\x1b[2m🔄 Falling back to provider '{}'...\x1b[0m",
                    fallback_provider.id
                );

                match call_provider_api_inner(
                    &fallback_options,
                    messages,
                    tools,
                    previous_response_id,
                )
                .await
                {
                    Ok(r) => return Ok(r),
                    Err(fallback_err) => {
                        eprintln!(
                            "\x1b[33m⚠️  Fallback '{}' also failed: {}\x1b[0m",
                            fallback_provider.id, fallback_err
                        );
                    }
                }
            }

            // All fallbacks exhausted — return original error
            result
        }
    }
}

fn is_retryable_error(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("429")
        || lower.contains("rate limit")
        || lower.contains("500")
        || lower.contains("502")
        || lower.contains("503")
        || lower.contains("service unavailable")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection refused")
}

async fn call_provider_api_inner(
    options: &AgentRunOptions,
    messages: &[AgentMessage],
    tools: &[Box<dyn Tool>],
    previous_response_id: Option<&str>,
) -> anyhow::Result<(AgentMessage, (usize, usize), Option<String>)> {
    match options.provider.protocol {
        ProviderProtocol::GeminiGenerateContent => {
            gemini::call_gemini_api(options, messages, tools).await
        }
        ProviderProtocol::OpenAiResponses => {
            openai::call_openai_responses_api(options, messages, tools, previous_response_id).await
        }
        ProviderProtocol::OpenAiChatCompletions => {
            openai::call_openai_chat_api(options, messages, tools).await
        }
        ProviderProtocol::AnthropicMessages => {
            anthropic::call_anthropic_api(options, messages, tools).await
        }
    }
}

pub(super) async fn execute_tool_call(
    call: &ToolCall,
    tools: &[Box<dyn Tool>],
    ctx: &ToolContext,
) -> ToolExecutionOutcome {
    let tool = tools.iter().find(|tool| tool.name() == call.name);
    match tool {
        Some(tool) => match tool.execute(call.arguments.clone(), ctx).await {
            Ok(mut result) => {
                let emitted_events = tool.take_output_events(&mut result);
                ToolExecutionOutcome {
                    result,
                    emitted_events,
                }
            }
            Err(error) => ToolExecutionOutcome {
                result: json!({ "error": error.to_string() }),
                emitted_events: Vec::new(),
            },
        },
        None => ToolExecutionOutcome {
            result: json!({ "error": format!("Unknown tool: {}", call.name) }),
            emitted_events: Vec::new(),
        },
    }
}

pub(super) struct ToolExecutionOutcome {
    pub result: Value,
    pub emitted_events: Vec<AgentEvent>,
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::{call_provider_api, execute_tool_call, openai};
    use crate::agent::{AgentEvent, AgentMessage, AgentRunOptions, ToolCall};
    use crate::config::SandboxPolicy;
    use crate::providers::{ProviderFamily, ProviderProfile, ProviderProtocol, ProviderSource};
    use crate::tools::{Tool, ToolContext};

    #[test]
    fn openai_response_input_uses_messages_on_initial_turn() {
        let messages = vec![AgentMessage {
            role: "user".to_string(),
            content: "review src/lib.rs".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }];

        let input = openai::openai_response_input(&messages, None);
        assert_eq!(
            input,
            vec![json!({
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": "review src/lib.rs"
                }]
            })]
        );
    }

    #[test]
    fn openai_response_input_uses_only_tool_outputs_on_follow_up_turns() {
        let messages = vec![
            AgentMessage {
                role: "user".to_string(),
                content: "review src/lib.rs".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
            AgentMessage {
                role: "assistant".to_string(),
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: json!({ "path": "src/lib.rs" }),
                }]),
                tool_call_id: None,
            },
            AgentMessage {
                role: "tool".to_string(),
                content: json!({ "content": "fn main() {}" }).to_string(),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
        ];

        let input = openai::openai_response_input(&messages, Some("resp_123"));
        assert_eq!(
            input,
            vec![json!({
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "{\"content\":\"fn main() {}\"}"
            })]
        );
    }

    #[test]
    fn parse_openai_response_extracts_text_and_tool_calls() {
        let response = json!({
            "id": "resp_123",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": "Need to inspect one file first."
                    }]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "read_file",
                    "arguments": "{\"path\":\"src/lib.rs\"}"
                }
            ]
        });

        let message = openai::parse_openai_response(&response).unwrap();
        assert_eq!(message.content, "Need to inspect one file first.");
        let tool_calls = message.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "read_file");
        assert_eq!(tool_calls[0].arguments, json!({ "path": "src/lib.rs" }));
    }

    #[tokio::test]
    async fn call_provider_api_supports_openai_responses_protocol() {
        let (base_url, server) = spawn_fake_server(
            "/responses",
            json!({
                "id": "resp_123",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": "response adapter ok"
                    }]
                }],
                "usage": {
                    "input_tokens": 12,
                    "output_tokens": 5
                }
            })
            .to_string(),
        )
        .await;

        let provider = provider_profile(
            ProviderFamily::OpenAiCompatible,
            ProviderProtocol::OpenAiResponses,
            base_url,
        );
        let mut options = AgentRunOptions::with_defaults(
            provider,
            std::env::temp_dir(),
            "say hi".to_string(),
            "test-key".to_string(),
        );
        options.streaming = false;
        let messages = vec![AgentMessage {
            role: "user".to_string(),
            content: "say hi".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }];
        let tools = Vec::new();

        let mut events = Vec::new();
        let (message, usage, response_id) =
            call_provider_api(&options, &messages, &tools, None, 1, &mut events)
                .await
                .unwrap();
        server.await.unwrap();

        assert_eq!(message.content, "response adapter ok");
        assert_eq!(usage, (12, 5));
        assert_eq!(response_id.as_deref(), Some("resp_123"));
    }

    #[tokio::test]
    async fn call_provider_api_supports_openai_chat_protocol() {
        let (base_url, server) = spawn_fake_server(
            "/chat/completions",
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "chat adapter ok"
                    }
                }],
                "usage": {
                    "prompt_tokens": 8,
                    "completion_tokens": 4
                }
            })
            .to_string(),
        )
        .await;

        let provider = provider_profile(
            ProviderFamily::OpenAiCompatible,
            ProviderProtocol::OpenAiChatCompletions,
            base_url,
        );
        let mut options = AgentRunOptions::with_defaults(
            provider,
            std::env::temp_dir(),
            "say hi".to_string(),
            "test-key".to_string(),
        );
        options.streaming = false;
        let messages = vec![AgentMessage {
            role: "user".to_string(),
            content: "say hi".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }];
        let tools = Vec::new();

        let mut events = Vec::new();
        let (message, usage, response_id) =
            call_provider_api(&options, &messages, &tools, None, 1, &mut events)
                .await
                .unwrap();
        server.await.unwrap();

        assert_eq!(message.content, "chat adapter ok");
        assert_eq!(usage, (8, 4));
        assert!(response_id.is_none());
    }

    #[tokio::test]
    async fn call_provider_api_supports_gemini_and_anthropic_protocols() {
        let (gemini_base, gemini_server) = spawn_fake_server(
            ":generateContent?key=test-key",
            json!({
                "candidates": [{
                    "content": {
                        "parts": [{
                            "text": "gemini adapter ok"
                        }]
                    }
                }],
                "usageMetadata": {
                    "promptTokenCount": 20,
                    "candidatesTokenCount": 6
                }
            })
            .to_string(),
        )
        .await;
        let gemini_provider = provider_profile(
            ProviderFamily::Gemini,
            ProviderProtocol::GeminiGenerateContent,
            gemini_base,
        );
        let mut gemini_options = AgentRunOptions::with_defaults(
            gemini_provider,
            std::env::temp_dir(),
            "say hi".to_string(),
            "test-key".to_string(),
        );
        gemini_options.streaming = false;
        let messages = vec![AgentMessage {
            role: "user".to_string(),
            content: "say hi".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }];
        let tools = Vec::new();
        let mut gemini_events = Vec::new();
        let (gemini_message, gemini_usage, _) = call_provider_api(
            &gemini_options,
            &messages,
            &tools,
            None,
            1,
            &mut gemini_events,
        )
        .await
        .unwrap();
        gemini_server.await.unwrap();
        assert_eq!(gemini_message.content, "gemini adapter ok");
        assert_eq!(gemini_usage, (20, 6));

        let (anthropic_base, anthropic_server) = spawn_fake_server(
            "/v1/messages",
            json!({
                "content": [{
                    "type": "text",
                    "text": "anthropic adapter ok"
                }],
                "usage": {
                    "input_tokens": 14,
                    "output_tokens": 7
                }
            })
            .to_string(),
        )
        .await;
        let anthropic_provider = provider_profile(
            ProviderFamily::Anthropic,
            ProviderProtocol::AnthropicMessages,
            anthropic_base,
        );
        let mut anthropic_options = AgentRunOptions::with_defaults(
            anthropic_provider,
            std::env::temp_dir(),
            "say hi".to_string(),
            "test-key".to_string(),
        );
        anthropic_options.streaming = false;
        let mut anthropic_events = Vec::new();
        let (anthropic_message, anthropic_usage, _) = call_provider_api(
            &anthropic_options,
            &messages,
            &tools,
            None,
            1,
            &mut anthropic_events,
        )
        .await
        .unwrap();
        anthropic_server.await.unwrap();
        assert_eq!(anthropic_message.content, "anthropic adapter ok");
        assert_eq!(anthropic_usage, (14, 7));
    }

    #[tokio::test]
    async fn execute_tool_call_collects_embedded_tool_events() {
        struct EventTool;

        #[async_trait]
        impl Tool for EventTool {
            fn name(&self) -> &str {
                "event_tool"
            }

            fn description(&self) -> &str {
                "Test tool"
            }

            fn parameters_schema(&self) -> Value {
                json!({
                    "type": "object",
                    "properties": {}
                })
            }

            async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<Value> {
                Ok(crate::tools::embed_tool_events(
                    json!({ "status": "ok" }),
                    vec![AgentEvent::CheckpointSaved {
                        path: "/tmp/tool-event".to_string(),
                    }],
                ))
            }

            fn take_output_events(&self, result: &mut Value) -> Vec<AgentEvent> {
                crate::tools::take_embedded_tool_events(result)
            }
        }

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "event_tool".to_string(),
            arguments: json!({}),
        };
        let ctx = ToolContext {
            workspace_root: std::env::temp_dir(),
            sandbox: SandboxPolicy::WorkspaceWrite,
            coordinator_provider: None,
            coordinator_api_key: None,
            coordinator_permission: None,
            coordinator_fallback_providers: Vec::new(),
            coordinator_plugin_definitions: Vec::new(),
            coordinator_depth: 0,
            coordinator_prompt_context: None,
            hooks: crate::hooks::HooksConfig::default(),
        };
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(EventTool)];

        let outcome = execute_tool_call(&tool_call, &tools, &ctx).await;
        assert_eq!(outcome.result, json!({ "status": "ok" }));
        assert_eq!(
            outcome.emitted_events,
            vec![AgentEvent::CheckpointSaved {
                path: "/tmp/tool-event".to_string(),
            }]
        );
    }

    fn provider_profile(
        family: ProviderFamily,
        protocol: ProviderProtocol,
        api_base: String,
    ) -> ProviderProfile {
        ProviderProfile {
            id: format!("test-{}", protocol.as_str()),
            family,
            protocol,
            label: "Test Provider".to_string(),
            api_base,
            api_key_env: "TEST_API_KEY".to_string(),
            model: "test-model".to_string(),
            best_for: "tests".to_string(),
            strengths: Vec::new(),
            supports_multimodal: false,
            supports_grounding: false,
            source: ProviderSource::BuiltIn,
        }
    }

    async fn spawn_fake_server(
        expected_path_fragment: &'static str,
        body: String,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 8192];
            let read = socket.read(&mut chunk).await.unwrap();
            request.extend_from_slice(&chunk[..read]);
            let text = String::from_utf8_lossy(&request);
            assert!(
                text.contains(expected_path_fragment),
                "request path mismatch: {}",
                text
            );

            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        (format!("http://{}", addr), handle)
    }
}
