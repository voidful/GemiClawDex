// GemiClawDex — Application facade
//
// Routes CLI commands to the appropriate subsystem.
// Rewritten to use serde-based output and thiserror for errors.

use std::env;
use std::io;
use std::path::{Path, PathBuf};

use crate::agent::{run_agent, AgentEvent, AgentRunOptions, PermissionLevel};
use crate::agent::memory::read_memory;
use crate::tools::build_memory_prompt_block;
use crate::commands::CommandCatalog;
use crate::config::{display_path, AppPaths, RuntimePreferences, SandboxPolicy};
use crate::instructions::InstructionBundle;
use crate::output::{
    inject_section_before_task, render_exec_output, render_provider_output, truncate_text,
    AppOutput, CheckpointEntryJson, CheckpointListJson, CountsJson, OverviewJson,
    ProviderDoctorJson, ProviderJson, ProviderListJson, ReloadJson, SessionEntryJson,
    SessionListJson, TrustSetJson, TrustStatusJson,
};
use crate::plugins::PluginCatalog;
use crate::prompt::{assemble_prompt, PromptAssembly, PromptRequest};
use crate::providers::{ProviderRegistry, ProviderScope};
use crate::session::{
    append_session_turn, fork_session, list_checkpoints, list_sessions, load_session,
    render_session_context, save_checkpoint, save_new_session, SessionExecutionData, SessionRecord,
    SessionReuseMode,
};
use crate::skills::SkillCatalog;
use crate::trust::{TrustRule, TrustRuleKind, TrustStore};
use crate::workspace::Workspace;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Runtime(#[from] anyhow::Error),
    #[error("{0}")]
    Message(String),
}

pub struct App {
    paths: AppPaths,
    preferences: RuntimePreferences,
}

impl App {
    pub fn new(current_dir: PathBuf) -> Self {
        Self {
            paths: AppPaths::detect(current_dir),
            preferences: RuntimePreferences::default(),
        }
    }

    pub async fn handle(&self, command: AppCommand) -> AppResult<AppOutput> {
        match command {
            AppCommand::Overview => self.overview(),
            AppCommand::ProvidersList => self.providers_list(),
            AppCommand::ProvidersCurrent => self.providers_current(),
            AppCommand::ProvidersShow { id } => self.providers_show(id),
            AppCommand::ProvidersUse { id, scope } => self.providers_use(id, scope),
            AppCommand::ProvidersDoctor { id } => self.providers_doctor(id),
            AppCommand::CommandsReload => self.commands_reload(),
            AppCommand::TrustStatus { path } => self.trust_status(path),
            AppCommand::TrustSet { path, kind } => self.trust_set(path, kind),
            AppCommand::CheckpointsList => self.checkpoints_list(),
            AppCommand::SessionsList => self.sessions_list(),
            AppCommand::SessionsShow { id } => self.sessions_show(id),
            AppCommand::SessionsFork { id } => self.sessions_fork(id),
            AppCommand::SessionsReplay { id, turn } => self.sessions_replay(id, turn),
            AppCommand::Exec(options) => self.exec(options).await,
        }
    }

    fn overview(&self) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        let providers = ProviderRegistry::load(&self.paths, &workspace, &trust)?;
        let current_provider =
            providers.current_profile(self.preferences.default_provider_id.as_deref())?;
        let commands = CommandCatalog::load(&self.paths, &workspace, &trust)?;
        let skills = SkillCatalog::load(&self.paths, &workspace, &trust)?;
        let instructions = InstructionBundle::load(&workspace, &trust)?;
        let sessions = list_sessions(&self.paths.sessions_dir())?;

        let lines = vec![
            "GemiClawDex".to_string(),
            format!("Workspace: {}", display_path(&workspace.root)),
            format!("Detected by: {}", workspace.detected_by),
            format!("Trust: {}", trust.status_label()),
            format!(
                "Current provider: {} ({})",
                current_provider.id, current_provider.label
            ),
            format!(
                "Sandbox default: {}",
                self.preferences.default_sandbox.as_str()
            ),
            format!("Registered providers: {}", providers.profiles().len()),
            format!("Instructions loaded: {}", instructions.sources.len()),
            format!("Custom commands loaded: {}", commands.commands.len()),
            format!("Skills loaded: {}", skills.skills.len()),
            format!("Saved sessions: {}", sessions.len()),
            "Use `gcd exec ...` to run a coding task.".to_string(),
        ];

        let json = OverviewJson {
            app: "GemiClawDex".to_string(),
            workspace: display_path(&workspace.root),
            detected_by: workspace.detected_by,
            trust: trust.status_label().to_string(),
            current_provider: ProviderJson::from(&current_provider),
            default_sandbox: self.preferences.default_sandbox.as_str().to_string(),
            counts: CountsJson {
                providers: providers.profiles().len(),
                instructions: instructions.sources.len(),
                commands: commands.commands.len(),
                skills: skills.skills.len(),
                sessions: sessions.len(),
            },
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn providers_list(&self) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        let providers = ProviderRegistry::load(&self.paths, &workspace, &trust)?;
        let current = providers.current_profile(self.preferences.default_provider_id.as_deref())?;

        let mut lines = vec![
            "Available providers".to_string(),
            format!("Current: {} ({})", current.id, current.label),
        ];
        for p in providers.profiles() {
            lines.push(format!(
                "  {} {} :: {} ({})",
                if p.id == current.id { "*" } else { " " },
                p.id,
                p.label,
                p.model
            ));
        }

        let json = ProviderListJson {
            current: ProviderJson::from(&current),
            active_global: providers.active_global().map(|s| s.to_string()),
            active_workspace: providers.active_workspace().map(|s| s.to_string()),
            providers: providers
                .profiles()
                .iter()
                .map(ProviderJson::from)
                .collect(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn providers_current(&self) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        let providers = ProviderRegistry::load(&self.paths, &workspace, &trust)?;
        let provider =
            providers.current_profile(self.preferences.default_provider_id.as_deref())?;
        Ok(render_provider_output(
            "Current provider",
            &provider,
            providers.active_global(),
            providers.active_workspace(),
        ))
    }

    fn providers_show(&self, id: String) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        let providers = ProviderRegistry::load(&self.paths, &workspace, &trust)?;
        let provider = providers
            .find(&id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("unknown provider: {}", id)))?;
        Ok(render_provider_output(
            "Provider details",
            &provider,
            providers.active_global(),
            providers.active_workspace(),
        ))
    }

    fn providers_use(&self, id: String, scope: ProviderScope) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        if scope == ProviderScope::Workspace && trust.restricts_project_config() {
            return Err(AppError::Message(
                "workspace-scoped provider switching is blocked for untrusted workspaces"
                    .to_string(),
            ));
        }
        let mut providers = ProviderRegistry::load(&self.paths, &workspace, &trust)?;
        let provider = providers.set_active(&id, scope)?;

        let lines = vec![
            "Provider updated".to_string(),
            format!("Scope: {}", scope.as_str()),
            format!("Provider: {} ({})", provider.id, provider.label),
        ];

        #[derive(serde::Serialize)]
        struct ProviderUseJson {
            scope: String,
            provider: ProviderJson,
        }
        let json = ProviderUseJson {
            scope: scope.as_str().to_string(),
            provider: ProviderJson::from(&provider),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn providers_doctor(&self, id: Option<String>) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        let providers = ProviderRegistry::load(&self.paths, &workspace, &trust)?;
        let report = providers.doctor(
            id.as_deref(),
            self.preferences.default_provider_id.as_deref(),
        )?;

        let mut lines = vec![
            "Provider doctor".to_string(),
            format!("Provider: {} ({})", report.profile.id, report.profile.label),
            format!("API key present: {}", report.api_key_present),
            format!("Active scope: {}", report.active_scope),
        ];
        if let Some(masked) = &report.masked_api_key {
            lines.push(format!("Masked key: {}", masked));
        }

        let json = ProviderDoctorJson {
            profile: ProviderJson::from(&report.profile),
            active_scope: report.active_scope.to_string(),
            api_key_present: report.api_key_present,
            masked_api_key: report.masked_api_key.clone(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn commands_reload(&self) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        let commands = CommandCatalog::load(&self.paths, &workspace, &trust)?;
        let skills = SkillCatalog::load(&self.paths, &workspace, &trust)?;

        let lines = vec![
            "Reload complete".to_string(),
            format!("Workspace: {}", display_path(&workspace.root)),
            format!("Commands: {}", commands.commands.len()),
            format!("Skills: {}", skills.skills.len()),
        ];

        let json = ReloadJson {
            workspace: display_path(&workspace.root),
            trust: trust.status_label().to_string(),
            commands: commands.commands.len(),
            skills: skills.skills.len(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn trust_status(&self, path: Option<PathBuf>) -> AppResult<AppOutput> {
        let target = match path {
            Some(p) => p,
            None => Workspace::discover(&self.paths.current_dir)?.root,
        };
        let trust = self.current_trust_state(&target)?;

        let lines = vec![
            "Trust status".to_string(),
            format!("Path: {}", display_path(&target)),
            format!("Status: {}", trust.status_label()),
        ];

        let json = TrustStatusJson {
            path: display_path(&target),
            status: trust.status_label().to_string(),
            matched_rule: trust.matched_path.as_ref().map(|p| display_path(p)),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn trust_set(&self, path: PathBuf, kind: TrustRuleKind) -> AppResult<AppOutput> {
        let path = if path.exists() {
            std::fs::canonicalize(path)?
        } else {
            path
        };
        let mut store = TrustStore::load(&self.paths.trust_store_path())?;
        store.upsert(TrustRule {
            kind: kind.clone(),
            path: path.clone(),
        });
        store.save(&self.paths.trust_store_path())?;

        let lines = vec![
            "Trust rule updated".to_string(),
            format!("Path: {}", display_path(&path)),
            format!("Rule: {}", kind.as_str()),
        ];

        let json = TrustSetJson {
            path: display_path(&path),
            rule: kind.as_str().to_string(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn checkpoints_list(&self) -> AppResult<AppOutput> {
        let checkpoints = list_checkpoints(&self.paths.checkpoints_dir())?;
        let mut lines = vec!["Checkpoints".to_string()];
        if checkpoints.is_empty() {
            lines.push("No checkpoints saved yet.".to_string());
        } else {
            for c in &checkpoints {
                lines.push(format!("- {} :: {}", c.file_name, c.summary));
            }
        }

        let json = CheckpointListJson {
            checkpoints: checkpoints.iter().map(CheckpointEntryJson::from).collect(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn sessions_list(&self) -> AppResult<AppOutput> {
        let sessions = list_sessions(&self.paths.sessions_dir())?;
        let mut lines = vec!["Sessions".to_string()];
        if sessions.is_empty() {
            lines.push("No sessions saved yet.".to_string());
        } else {
            for s in &sessions {
                lines.push(format!(
                    "- {} :: {} :: {} turns",
                    s.id, s.latest_summary, s.turn_count
                ));
            }
        }

        let json = SessionListJson {
            sessions: sessions.iter().map(SessionEntryJson::from).collect(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn sessions_show(&self, id: String) -> AppResult<AppOutput> {
        let record = load_session(&self.paths.sessions_dir(), &id)?;
        let latest = record
            .latest_turn()
            .ok_or_else(|| AppError::Message("session has no turns".to_string()))?;

        let lines = vec![
            "Session".to_string(),
            format!("ID: {}", record.id),
            format!("Workspace: {}", display_path(&record.workspace_root)),
            format!("Turns: {}", record.turn_count()),
            format!("Latest mode: {}", latest.mode),
            format!("Latest task: {}", truncate_text(&latest.input, 120)),
        ];
        let mut lines = lines;
        if let Some(final_response) = &latest.final_response {
            lines.push(format!(
                "Latest response: {}",
                truncate_text(final_response, 120)
            ));
        }
        if latest.tool_invocation_count > 0 {
            lines.push(format!(
                "Latest tool calls: {}",
                latest.tool_invocation_count
            ));
        }
        if !latest.events.is_empty() {
            lines.push(format!("Latest events: {}", latest.events.len()));
        }

        #[derive(serde::Serialize)]
        struct SessionShowJson {
            id: String,
            workspace: String,
            turn_count: usize,
            latest_mode: String,
            latest_task: String,
            latest_response: Option<String>,
            latest_tool_invocation_count: usize,
            latest_total_tokens: usize,
            latest_api_calls: usize,
            latest_event_count: usize,
            latest_events: Vec<AgentEvent>,
        }
        let json = SessionShowJson {
            id: record.id.clone(),
            workspace: display_path(&record.workspace_root),
            turn_count: record.turn_count(),
            latest_mode: latest.mode.clone(),
            latest_task: latest.input.clone(),
            latest_response: latest.final_response.clone(),
            latest_tool_invocation_count: latest.tool_invocation_count,
            latest_total_tokens: latest.total_tokens,
            latest_api_calls: latest.api_calls,
            latest_event_count: latest.events.len(),
            latest_events: latest.events.clone(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn sessions_fork(&self, id: String) -> AppResult<AppOutput> {
        let forked = fork_session(&self.paths.sessions_dir(), &id)?;

        let lines = vec![
            "Session forked".to_string(),
            format!("Source: {}", id),
            format!("New session: {}", forked.id),
            format!("Turns copied: {}", forked.turn_count()),
        ];

        #[derive(serde::Serialize)]
        struct ForkJson {
            source_id: String,
            new_id: String,
            turn_count: usize,
        }
        let json = ForkJson {
            source_id: id,
            new_id: forked.id.clone(),
            turn_count: forked.turn_count(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn sessions_replay(&self, id: String, turn: Option<usize>) -> AppResult<AppOutput> {
        let record = load_session(&self.paths.sessions_dir(), &id)?;
        let selected_turns = record
            .turns
            .iter()
            .filter(|entry| turn.map(|value| entry.index == value).unwrap_or(true))
            .collect::<Vec<_>>();

        if selected_turns.is_empty() {
            return Err(AppError::Message(match turn {
                Some(turn) => format!("session {} does not have turn {}", id, turn),
                None => format!("session {} has no replayable turns", id),
            }));
        }

        let mut lines = vec![
            "Session replay".to_string(),
            format!("ID: {}", record.id),
            format!("Workspace: {}", display_path(&record.workspace_root)),
            format!("Turns selected: {}", selected_turns.len()),
        ];
        let mut jsonl_values = Vec::new();

        #[derive(serde::Serialize)]
        struct ReplayTurnJson {
            index: usize,
            mode: String,
            input: String,
            event_count: usize,
            events: Vec<AgentEvent>,
        }

        #[derive(serde::Serialize)]
        struct ReplayJson {
            id: String,
            workspace: String,
            selected_turn_count: usize,
            turns: Vec<ReplayTurnJson>,
        }

        let turns = selected_turns
            .iter()
            .map(|entry| {
                lines.push(String::new());
                lines.push(format!(
                    "Turn {} :: {} :: {}",
                    entry.index,
                    entry.mode,
                    truncate_text(&entry.input, 96)
                ));
                if entry.events.is_empty() {
                    lines.push("(no events)".to_string());
                } else {
                    for event in &entry.events {
                        lines.push(format!("- {}", describe_event(event)));
                        jsonl_values.push(serde_json::json!({
                            "session_id": record.id,
                            "turn_index": entry.index,
                            "turn_mode": entry.mode,
                            "input": entry.input,
                            "payload": event
                        }));
                    }
                }

                ReplayTurnJson {
                    index: entry.index,
                    mode: entry.mode.clone(),
                    input: entry.input.clone(),
                    event_count: entry.events.len(),
                    events: entry.events.clone(),
                }
            })
            .collect::<Vec<_>>();

        let json = ReplayJson {
            id: record.id.clone(),
            workspace: display_path(&record.workspace_root),
            selected_turn_count: turns.len(),
            turns,
        };

        Ok(AppOutput::new_with_jsonl(lines, &json, jsonl_values))
    }

    async fn exec(&self, options: ExecOptions) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        let providers = ProviderRegistry::load(&self.paths, &workspace, &trust)?;
        let commands = CommandCatalog::load(&self.paths, &workspace, &trust)?;
        let skills = SkillCatalog::load(&self.paths, &workspace, &trust)?;
        let instructions = InstructionBundle::load(&workspace, &trust)?;
        let plugins = PluginCatalog::load(&self.paths, &workspace, &trust)?;
        let provider = providers.resolve_profile(
            options.provider.as_deref(),
            self.preferences.default_provider_id.as_deref(),
        )?;
        let sandbox = options
            .sandbox
            .unwrap_or_else(|| self.preferences.default_sandbox.clone());

        let session_source = match (
            options.resume_session.as_deref(),
            options.fork_session.as_deref(),
        ) {
            (Some(id), None) => Some((
                SessionReuseMode::Resume,
                self.load_session_for_workspace(&workspace.root, id)?,
            )),
            (None, Some(id)) => Some((
                SessionReuseMode::Fork,
                self.load_session_for_workspace(&workspace.root, id)?,
            )),
            (None, None) => None,
            (Some(_), Some(_)) => {
                return Err(AppError::Message(
                    "resume and fork modes are mutually exclusive".to_string(),
                ))
            }
        };

        // Dual memory: try new tool-based memory (MEMORY.md + USER.md via §-delimited
        // entries) first, fall back to legacy section-based MEMORY.md.
        let memory_content = build_memory_prompt_block(&workspace.root)
            .or_else(|| read_memory(&workspace.root));

        let mut assembly = assemble_prompt(PromptRequest {
            workspace: &workspace,
            trust: &trust,
            sandbox,
            provider,
            instructions: &instructions,
            commands: &commands,
            skills: &skills,
            selected_skill: options.skill.as_deref(),
            user_input: &options.input,
            memory_content: memory_content.as_deref(),
            ide_context: options.ide_context.as_ref(),
        })?;

        if let Some((mode, record)) = &session_source {
            assembly.final_prompt = inject_section_before_task(
                &assembly.final_prompt,
                &render_session_context(record, *mode),
            );
        }

        let checkpoint_path = if options.checkpoint {
            Some(save_checkpoint(&self.paths.checkpoints_dir(), &assembly)?)
        } else {
            None
        };

        let source_session_meta = session_source
            .as_ref()
            .map(|(mode, record)| (mode.as_str().to_string(), record.id.clone()));

        if options.plan_only {
            let planned_events = build_execution_events(
                "plan",
                &assembly,
                checkpoint_path.as_deref(),
                source_session_meta.as_ref(),
                &[],
                None,
            );
            let execution = SessionExecutionData {
                events: planned_events.clone(),
                ..Default::default()
            };
            let plan_saved_session = if options.persist_session {
                match &session_source {
                    Some((SessionReuseMode::Resume, record)) => Some((
                        "updated",
                        append_session_turn(
                            &self.paths.sessions_dir(),
                            &record.id,
                            &options.input,
                            &assembly,
                            "plan",
                            Some(&execution),
                        )?,
                    )),
                    Some((SessionReuseMode::Fork, record)) => {
                        let forked = fork_session(&self.paths.sessions_dir(), &record.id)?;
                        Some((
                            "forked",
                            append_session_turn(
                                &self.paths.sessions_dir(),
                                &forked.id,
                                &options.input,
                                &assembly,
                                "plan",
                                Some(&execution),
                            )?,
                        ))
                    }
                    None => Some((
                        "created",
                        save_new_session(
                            &self.paths.sessions_dir(),
                            &workspace.root,
                            None,
                            &options.input,
                            &assembly,
                            "plan",
                            Some(&execution),
                        )?,
                    )),
                }
            } else {
                None
            };
            let persisted_session_meta = plan_saved_session
                .as_ref()
                .map(|(action, record)| (action.to_string(), record.clone()));
            let output_events = build_execution_events(
                "plan",
                &assembly,
                checkpoint_path.as_deref(),
                source_session_meta.as_ref(),
                &[],
                plan_saved_session
                    .as_ref()
                    .map(|(action, record)| (*action, record)),
            );
            return Ok(render_exec_output(
                &assembly,
                checkpoint_path,
                source_session_meta,
                persisted_session_meta,
                &output_events,
                options.print_prompt,
                None,
            ));
        }

        let api_key = self.read_provider_api_key(&assembly.provider)?;
        let mut agent_options = AgentRunOptions::with_defaults(
            assembly.provider.clone(),
            workspace.root.clone(),
            assembly.final_prompt.clone(),
            api_key.clone(),
        );
        agent_options.sandbox = assembly.sandbox.clone();
        agent_options.permission = options.permission.clone();
        agent_options.streaming = options.stream;
        agent_options.auto_git = options.auto_git;
        agent_options.planning = false;
        agent_options.ide_context = options.ide_context.clone();

        // Build fallback provider list from same-family providers
        let mut fallback_providers = Vec::new();
        for profile in providers.profiles() {
            if profile.id != assembly.provider.id {
                if let Ok(key) = self.read_provider_api_key(profile) {
                    fallback_providers.push((profile.clone(), key));
                }
            }
        }
        agent_options.fallback_providers = fallback_providers;
        agent_options.plugin_definitions = plugins.plugins.clone();

        let result = run_agent(agent_options, plugins.into_tools()).await?;
        let stored_events = build_execution_events(
            "live",
            &assembly,
            checkpoint_path.as_deref(),
            source_session_meta.as_ref(),
            &result.events,
            None,
        );
        let execution = SessionExecutionData {
            final_response: if result.final_response.is_empty() {
                None
            } else {
                Some(result.final_response.clone())
            },
            turns_used: result.turns_used,
            tool_invocation_count: result.tool_invocations.len(),
            prompt_tokens: result.token_usage.prompt_tokens,
            completion_tokens: result.token_usage.completion_tokens,
            total_tokens: result.token_usage.total_tokens,
            api_calls: result.token_usage.api_calls,
            events: stored_events,
        };

        let saved_session = if options.persist_session {
            match &session_source {
                Some((SessionReuseMode::Resume, record)) => Some((
                    "updated",
                    append_session_turn(
                        &self.paths.sessions_dir(),
                        &record.id,
                        &options.input,
                        &assembly,
                        "live",
                        Some(&execution),
                    )?,
                )),
                Some((SessionReuseMode::Fork, record)) => {
                    let forked = fork_session(&self.paths.sessions_dir(), &record.id)?;
                    Some((
                        "forked",
                        append_session_turn(
                            &self.paths.sessions_dir(),
                            &forked.id,
                            &options.input,
                            &assembly,
                            "live",
                            Some(&execution),
                        )?,
                    ))
                }
                None => Some((
                    "created",
                    save_new_session(
                        &self.paths.sessions_dir(),
                        &workspace.root,
                        None,
                        &options.input,
                        &assembly,
                        "live",
                        Some(&execution),
                    )?,
                )),
            }
        } else {
            None
        };
        let persisted_session_meta = saved_session
            .as_ref()
            .map(|(action, record)| (action.to_string(), record.clone()));
        let output_events = build_execution_events(
            "live",
            &assembly,
            checkpoint_path.as_deref(),
            source_session_meta.as_ref(),
            &result.events,
            saved_session
                .as_ref()
                .map(|(action, record)| (*action, record)),
        );

        Ok(render_exec_output(
            &assembly,
            checkpoint_path,
            source_session_meta,
            persisted_session_meta,
            &output_events,
            options.print_prompt,
            Some(&result),
        ))
    }

    fn current_trust_state(&self, path: &Path) -> AppResult<crate::trust::TrustState> {
        let store = TrustStore::load(&self.paths.trust_store_path())?;
        Ok(store.evaluate(path, self.preferences.trust_enabled))
    }

    fn load_session_for_workspace(
        &self,
        workspace_root: &Path,
        session_id: &str,
    ) -> AppResult<SessionRecord> {
        let record = load_session(&self.paths.sessions_dir(), session_id)?;
        if record.workspace_root != workspace_root {
            return Err(AppError::Message(format!(
                "session {} belongs to {} instead of {}",
                session_id,
                display_path(&record.workspace_root),
                display_path(workspace_root)
            )));
        }
        Ok(record)
    }

    fn read_provider_api_key(
        &self,
        provider: &crate::providers::ProviderProfile,
    ) -> AppResult<String> {
        env::var(&provider.api_key_env)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::Message(format!(
                    "provider {} requires {} to be set before live execution",
                    provider.id, provider.api_key_env
                ))
            })
    }
}

#[derive(Clone, Debug)]
pub struct ExecOptions {
    pub input: String,
    pub skill: Option<String>,
    pub provider: Option<String>,
    pub sandbox: Option<SandboxPolicy>,
    pub checkpoint: bool,
    pub persist_session: bool,
    pub resume_session: Option<String>,
    pub fork_session: Option<String>,
    pub print_prompt: bool,
    pub permission: PermissionLevel,
    pub stream: bool,
    pub auto_git: bool,
    pub plan_only: bool,
    pub ide_context: Option<crate::agent::IdeContext>,
}

#[derive(Clone, Debug)]
pub enum AppCommand {
    Overview,
    ProvidersList,
    ProvidersCurrent,
    ProvidersShow { id: String },
    ProvidersUse { id: String, scope: ProviderScope },
    ProvidersDoctor { id: Option<String> },
    CommandsReload,
    TrustStatus { path: Option<PathBuf> },
    TrustSet { path: PathBuf, kind: TrustRuleKind },
    CheckpointsList,
    SessionsList,
    SessionsShow { id: String },
    SessionsFork { id: String },
    SessionsReplay { id: String, turn: Option<usize> },
    Exec(ExecOptions),
}

fn build_execution_events(
    mode: &str,
    assembly: &PromptAssembly,
    checkpoint_path: Option<&Path>,
    source_session: Option<&(String, String)>,
    runtime_events: &[AgentEvent],
    persisted_session: Option<(&str, &SessionRecord)>,
) -> Vec<AgentEvent> {
    let mut events = vec![AgentEvent::ExecutionPrepared {
        mode: mode.to_string(),
        workspace: display_path(&assembly.workspace_root),
        provider_id: assembly.provider.id.clone(),
        provider_label: assembly.provider.label.clone(),
        protocol: assembly.provider.protocol.as_str().to_string(),
        sandbox: assembly.sandbox.as_str().to_string(),
        trust: assembly.trust_label.clone(),
        active_command: assembly.active_command.clone(),
        active_skill: assembly.active_skill.clone(),
        prompt: assembly.final_prompt.clone(),
        attachment_count: assembly.attachments.len(),
        pending_shell_command_count: assembly.pending_shell_commands.len(),
        source_session_id: source_session.map(|(_, id)| id.clone()),
        source_mode: source_session.map(|(source_mode, _)| source_mode.clone()),
    }];

    if let Some(path) = checkpoint_path {
        events.push(AgentEvent::CheckpointSaved {
            path: display_path(path),
        });
    }

    events.extend(runtime_events.iter().cloned());

    if let Some((action, record)) = persisted_session {
        events.push(AgentEvent::SessionPersisted {
            action: action.to_string(),
            id: record.id.clone(),
            turn_count: record.turn_count(),
        });
    }

    events
}

fn describe_event(event: &AgentEvent) -> String {
    match event {
        AgentEvent::ExecutionPrepared {
            mode,
            provider_id,
            attachment_count,
            ..
        } => format!(
            "prepared {} execution with provider {} ({} attachments)",
            mode, provider_id, attachment_count
        ),
        AgentEvent::CheckpointSaved { path } => format!("checkpoint saved to {}", path),
        AgentEvent::RunStarted {
            model,
            protocol,
            max_turns,
            ..
        } => format!(
            "run started with {} via {} (max {} turns)",
            model, protocol, max_turns
        ),
        AgentEvent::TurnStarted {
            turn,
            message_count,
            ..
        } => format!("turn {} started with {} messages", turn, message_count),
        AgentEvent::ContextCompacted {
            turn,
            estimated_tokens,
            ..
        } => format!(
            "turn {} compacted context around {} estimated tokens",
            turn, estimated_tokens
        ),
        AgentEvent::ProviderCalled { turn, protocol, .. } => {
            format!("turn {} called provider via {}", turn, protocol)
        }
        AgentEvent::AssistantMessage {
            turn,
            tool_call_count,
            content,
        } => format!(
            "turn {} assistant replied ({} tool calls, {})",
            turn,
            tool_call_count,
            truncate_text(content, 72)
        ),
        AgentEvent::ToolCallRequested {
            turn, tool_name, ..
        } => format!("turn {} requested tool {}", turn, tool_name),
        AgentEvent::ToolCallDenied {
            turn, tool_name, ..
        } => format!("turn {} denied tool {}", turn, tool_name),
        AgentEvent::ToolCallCompleted {
            turn, tool_name, ..
        } => format!("turn {} completed tool {}", turn, tool_name),
        AgentEvent::CoordinatorStarted {
            depth,
            execution_mode,
            task_count,
            max_concurrency,
        } => format!(
            "coordinator depth {} started {} task(s) via {} (max concurrency {})",
            depth, task_count, execution_mode, max_concurrency
        ),
        AgentEvent::CoordinatorBatchStarted {
            depth,
            batch,
            total_batches,
            tasks,
        } => format!(
            "coordinator depth {} batch {}/{} started [{}]",
            depth,
            batch,
            total_batches,
            tasks.join(", ")
        ),
        AgentEvent::CoordinatorTaskStarted {
            depth,
            batch,
            task_name,
            depends_on,
        } => {
            if depends_on.is_empty() {
                format!(
                    "coordinator depth {} batch {} started delegated task {}",
                    depth, batch, task_name
                )
            } else {
                format!(
                    "coordinator depth {} batch {} started delegated task {} after [{}]",
                    depth,
                    batch,
                    task_name,
                    depends_on.join(", ")
                )
            }
        }
        AgentEvent::CoordinatorTaskBlocked {
            depth,
            batch,
            task_name,
            blocked_by,
        } => format!(
            "coordinator depth {} batch {} blocked task {} by [{}]",
            depth,
            batch,
            task_name,
            blocked_by.join(", ")
        ),
        AgentEvent::CoordinatorTaskCompleted {
            depth,
            batch,
            task_name,
            status,
            total_tokens,
            summary,
            ..
        } => format!(
            "coordinator depth {} batch {} finished task {} as {} ({} tokens, {})",
            depth,
            batch,
            task_name,
            status,
            total_tokens,
            truncate_text(summary, 60)
        ),
        AgentEvent::CoordinatorCompleted {
            depth,
            execution_mode,
            completed_count,
            failed_count,
            blocked_count,
            total_tokens,
            ..
        } => format!(
            "coordinator depth {} completed via {} (ok={}, failed={}, blocked={}, tokens={})",
            depth, execution_mode, completed_count, failed_count, blocked_count, total_tokens
        ),
        AgentEvent::RunCompleted {
            turns_used,
            tool_invocation_count,
            total_tokens,
            ..
        } => format!(
            "run completed after {} turns, {} tool calls, {} total tokens",
            turns_used, tool_invocation_count, total_tokens
        ),
        AgentEvent::SessionPersisted {
            action,
            id,
            turn_count,
        } => format!("session {} as {} ({} turns)", action, id, turn_count),
        AgentEvent::ProviderFallback {
            turn,
            from_provider,
            to_provider,
            reason,
        } => format!(
            "turn {} provider fallback: {} → {} ({})",
            turn,
            from_provider,
            to_provider,
            truncate_text(reason, 48)
        ),
        AgentEvent::ArtifactUpdated {
            path,
            artifact_type,
            summary,
        } => format!(
            "artifact updated: {} ({}) - {}",
            path,
            artifact_type,
            truncate_text(summary, 60)
        ),
    }
}
