// GemiClawdex — Application facade
//
// Routes CLI commands to the appropriate subsystem.
// Rewritten to use serde-based output and thiserror for errors.

use std::io;
use std::path::{Path, PathBuf};

use crate::commands::CommandCatalog;
use crate::config::{display_path, AppPaths, RuntimePreferences, SandboxPolicy};
use crate::instructions::InstructionBundle;
use crate::output::{
    inject_section_before_task, render_exec_output, render_provider_output, truncate_text,
    AppOutput, CheckpointEntryJson, CheckpointListJson, CountsJson, OverviewJson,
    ProviderDoctorJson, ProviderJson, ProviderListJson, ReloadJson, SessionEntryJson,
    SessionListJson, TrustSetJson, TrustStatusJson,
};
use crate::prompt::{assemble_prompt, PromptRequest};
use crate::providers::{ProviderRegistry, ProviderScope};
use crate::session::{
    append_session_turn, fork_session, list_checkpoints, list_sessions, load_session,
    render_session_context, save_checkpoint, save_new_session, SessionRecord, SessionReuseMode,
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

    pub fn handle(&self, command: AppCommand) -> AppResult<AppOutput> {
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
            AppCommand::Exec(options) => self.exec(options),
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
            "GemiClawdex".to_string(),
            format!("Workspace: {}", display_path(&workspace.root)),
            format!("Detected by: {}", workspace.detected_by),
            format!("Trust: {}", trust.status_label()),
            format!("Current provider: {} ({})", current_provider.id, current_provider.label),
            format!("Sandbox default: {}", self.preferences.default_sandbox.as_str()),
            format!("Registered providers: {}", providers.profiles().len()),
            format!("Instructions loaded: {}", instructions.sources.len()),
            format!("Custom commands loaded: {}", commands.commands.len()),
            format!("Skills loaded: {}", skills.skills.len()),
            format!("Saved sessions: {}", sessions.len()),
            "Use `gemi-clawdex exec ...` to run a coding task.".to_string(),
        ];

        let json = OverviewJson {
            app: "GemiClawdex".to_string(),
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
                p.id, p.label, p.model
            ));
        }

        let json = ProviderListJson {
            current: ProviderJson::from(&current),
            active_global: providers.active_global().map(|s| s.to_string()),
            active_workspace: providers.active_workspace().map(|s| s.to_string()),
            providers: providers.profiles().iter().map(ProviderJson::from).collect(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn providers_current(&self) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        let providers = ProviderRegistry::load(&self.paths, &workspace, &trust)?;
        let provider = providers.current_profile(self.preferences.default_provider_id.as_deref())?;
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
                "workspace-scoped provider switching is blocked for untrusted workspaces".to_string(),
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
        struct ProviderUseJson { scope: String, provider: ProviderJson }
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
            format!("Latest task: {}", truncate_text(&latest.input, 120)),
        ];

        #[derive(serde::Serialize)]
        struct SessionShowJson { id: String, workspace: String, turn_count: usize }
        let json = SessionShowJson {
            id: record.id.clone(),
            workspace: display_path(&record.workspace_root),
            turn_count: record.turn_count(),
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
        struct ForkJson { source_id: String, new_id: String, turn_count: usize }
        let json = ForkJson {
            source_id: id,
            new_id: forked.id.clone(),
            turn_count: forked.turn_count(),
        };

        Ok(AppOutput::new(lines, &json))
    }

    fn exec(&self, options: ExecOptions) -> AppResult<AppOutput> {
        let workspace = Workspace::discover(&self.paths.current_dir)?;
        let trust = self.current_trust_state(&workspace.root)?;
        let providers = ProviderRegistry::load(&self.paths, &workspace, &trust)?;
        let commands = CommandCatalog::load(&self.paths, &workspace, &trust)?;
        let skills = SkillCatalog::load(&self.paths, &workspace, &trust)?;
        let instructions = InstructionBundle::load(&workspace, &trust)?;
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

        let saved_session = if options.persist_session {
            match &session_source {
                Some((SessionReuseMode::Resume, record)) => Some((
                    "updated",
                    append_session_turn(
                        &self.paths.sessions_dir(),
                        &record.id,
                        &options.input,
                        &assembly,
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
                    )?,
                )),
            }
        } else {
            None
        };

        let source_session_meta = session_source
            .as_ref()
            .map(|(mode, record)| (mode.as_str().to_string(), record.id.clone()));
        let persisted_session_meta = saved_session
            .as_ref()
            .map(|(action, record)| (action.to_string(), record.clone()));

        Ok(render_exec_output(
            &assembly,
            checkpoint_path,
            source_session_meta,
            persisted_session_meta,
            options.print_prompt,
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
    Exec(ExecOptions),
}
