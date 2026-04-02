use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{display_path, AppPaths};
use crate::trust::TrustState;
use crate::workspace::Workspace;

const DEFAULT_PROVIDER_ID: &str = "openai-codex";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderFamily {
    Gemini,
    OpenAiCompatible,
    Anthropic,
}

impl ProviderFamily {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gemini" | "google-gemini" => Some(Self::Gemini),
            "openai" | "codex" | "openai-compatible" | "openai_compatible" => {
                Some(Self::OpenAiCompatible)
            }
            "anthropic" | "claude" => Some(Self::Anthropic),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gemini => "gemini",
            Self::OpenAiCompatible => "openai-compatible",
            Self::Anthropic => "anthropic",
        }
    }

    fn default_protocol(&self) -> ProviderProtocol {
        match self {
            Self::Gemini => ProviderProtocol::GeminiGenerateContent,
            Self::OpenAiCompatible => ProviderProtocol::OpenAiResponses,
            Self::Anthropic => ProviderProtocol::AnthropicMessages,
        }
    }

    fn default_key_env(&self) -> &'static str {
        match self {
            Self::Gemini => "GEMINI_API_KEY",
            Self::OpenAiCompatible => "OPENAI_API_KEY",
            Self::Anthropic => "ANTHROPIC_API_KEY",
        }
    }

    fn default_multimodal(&self) -> bool {
        match self {
            Self::Gemini => true,
            Self::OpenAiCompatible => false,
            Self::Anthropic => true,
        }
    }

    fn default_grounding(&self) -> bool {
        matches!(self, Self::Gemini)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderProtocol {
    GeminiGenerateContent,
    OpenAiResponses,
    OpenAiChatCompletions,
    AnthropicMessages,
}

impl ProviderProtocol {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gemini" | "gemini-generate-content" | "generate-content" => {
                Some(Self::GeminiGenerateContent)
            }
            "openai-responses" | "responses" => Some(Self::OpenAiResponses),
            "openai-chat" | "chat-completions" | "openai-chat-completions" => {
                Some(Self::OpenAiChatCompletions)
            }
            "anthropic" | "anthropic-messages" | "messages" => Some(Self::AnthropicMessages),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GeminiGenerateContent => "gemini-generate-content",
            Self::OpenAiResponses => "openai-responses",
            Self::OpenAiChatCompletions => "openai-chat-completions",
            Self::AnthropicMessages => "anthropic-messages",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderScope {
    Global,
    Workspace,
}

impl ProviderScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ProviderSource {
    BuiltIn,
    Environment,
    GlobalConfig(PathBuf),
    WorkspaceConfig(PathBuf),
}

impl ProviderSource {
    pub fn label(&self) -> String {
        match self {
            Self::BuiltIn => "built-in".to_string(),
            Self::Environment => "environment overrides".to_string(),
            Self::GlobalConfig(path) => format!("global config ({})", display_path(path)),
            Self::WorkspaceConfig(path) => format!("workspace config ({})", display_path(path)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProviderProfile {
    pub id: String,
    pub family: ProviderFamily,
    pub protocol: ProviderProtocol,
    pub label: String,
    pub api_base: String,
    pub api_key_env: String,
    pub model: String,
    pub best_for: String,
    pub strengths: Vec<String>,
    pub supports_multimodal: bool,
    pub supports_grounding: bool,
    pub source: ProviderSource,
}

#[derive(Clone, Debug)]
pub struct ProviderDoctor {
    pub profile: ProviderProfile,
    pub active_scope: &'static str,
    pub api_key_present: bool,
    pub masked_api_key: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProviderRegistry {
    profiles: Vec<ProviderProfile>,
    active_global: Option<String>,
    active_workspace: Option<String>,
    global_active_path: PathBuf,
    workspace_active_path: PathBuf,
}

impl ProviderRegistry {
    pub fn load(paths: &AppPaths, workspace: &Workspace, trust: &TrustState) -> io::Result<Self> {
        let global_registry_path = paths.global_provider_registry_path();
        let workspace_registry_path = workspace.root.join(".gemi-clawdex").join("providers.conf");
        let global_active_path = paths.global_active_provider_path();
        let workspace_active_path = workspace
            .root
            .join(".gemi-clawdex")
            .join("active-provider.txt");

        let mut providers = BTreeMap::new();
        let builtin_profiles = built_in_profiles();
        for profile in builtin_profiles {
            providers.insert(profile.id.clone(), profile);
        }

        for profile in env_profiles() {
            providers.insert(profile.id.clone(), profile);
        }

        let global_profiles = parse_provider_config(
            &global_registry_path,
            ProviderSource::GlobalConfig(global_registry_path.clone()),
        )?;
        for profile in global_profiles {
            providers.insert(profile.id.clone(), profile);
        }

        if !trust.restricts_project_config() {
            let workspace_profiles = parse_provider_config(
                &workspace_registry_path,
                ProviderSource::WorkspaceConfig(workspace_registry_path.clone()),
            )?;
            for profile in workspace_profiles {
                providers.insert(profile.id.clone(), profile);
            }
        }

        let profiles = providers
            .into_iter()
            .map(|(_, profile)| profile)
            .collect::<Vec<_>>();
        let active_global = read_optional_trimmed(&global_active_path)?;
        let active_workspace = if trust.restricts_project_config() {
            None
        } else {
            read_optional_trimmed(&workspace_active_path)?
        };

        Ok(Self {
            profiles,
            active_global,
            active_workspace,
            global_active_path,
            workspace_active_path,
        })
    }

    pub fn profiles(&self) -> &[ProviderProfile] {
        &self.profiles
    }

    pub fn current_profile(&self, preferred_id: Option<&str>) -> io::Result<ProviderProfile> {
        self.resolve_profile(None, preferred_id)
    }

    pub fn resolve_profile(
        &self,
        explicit_id: Option<&str>,
        preferred_id: Option<&str>,
    ) -> io::Result<ProviderProfile> {
        let candidates = [
            explicit_id,
            preferred_id,
            self.active_workspace.as_deref(),
            self.active_global.as_deref(),
            Some(DEFAULT_PROVIDER_ID),
        ];

        for value in candidates.iter().flatten() {
            if let Some(profile) = self.find(value) {
                return Ok(profile.clone());
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no provider profiles are available",
        ))
    }

    pub fn find(&self, id: &str) -> Option<&ProviderProfile> {
        let normalized = normalize_provider_id(id);
        self.profiles
            .iter()
            .find(|profile| profile.id == normalized)
    }

    pub fn set_active(&mut self, id: &str, scope: ProviderScope) -> io::Result<ProviderProfile> {
        let profile = self.find(id).cloned().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("unknown provider: {}", id))
        })?;
        let normalized = profile.id.clone();
        let target = match scope {
            ProviderScope::Global => &self.global_active_path,
            ProviderScope::Workspace => &self.workspace_active_path,
        };

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, format!("{}\n", normalized))?;

        match scope {
            ProviderScope::Global => self.active_global = Some(normalized),
            ProviderScope::Workspace => self.active_workspace = Some(normalized),
        }

        Ok(profile)
    }

    pub fn doctor(
        &self,
        id: Option<&str>,
        preferred_id: Option<&str>,
    ) -> io::Result<ProviderDoctor> {
        let profile = match id {
            Some(id) => self.find(id).cloned().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, format!("unknown provider: {}", id))
            })?,
            None => self.current_profile(preferred_id)?,
        };
        let key = env::var(&profile.api_key_env)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Ok(ProviderDoctor {
            profile,
            active_scope: if self.active_workspace.is_some() {
                "workspace"
            } else if self.active_global.is_some() {
                "global"
            } else {
                "default"
            },
            api_key_present: key.is_some(),
            masked_api_key: key.as_ref().map(|value| mask_secret(value)),
        })
    }

    pub fn active_global(&self) -> Option<&str> {
        self.active_global.as_deref()
    }

    pub fn active_workspace(&self) -> Option<&str> {
        self.active_workspace.as_deref()
    }
}

pub fn builtin_provider(id: &str) -> Option<ProviderProfile> {
    let normalized = normalize_provider_id(id);
    built_in_profiles()
        .into_iter()
        .find(|profile| profile.id == normalized)
}

fn built_in_profiles() -> Vec<ProviderProfile> {
    vec![
        ProviderProfile {
            id: "openai-codex".to_string(),
            family: ProviderFamily::OpenAiCompatible,
            protocol: ProviderProtocol::OpenAiResponses,
            label: "OpenAI Codex".to_string(),
            api_base: "https://api.openai.com/v1".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            model: "codex-latest".to_string(),
            best_for: "agentic coding, repository instructions, and patch planning".to_string(),
            strengths: vec![
                "Great default for coding-agent workflows".to_string(),
                "Natural fit for AGENTS.md-style repository instructions".to_string(),
                "Strong foundation for edit-and-verify loops".to_string(),
            ],
            supports_multimodal: false,
            supports_grounding: false,
            source: ProviderSource::BuiltIn,
        },
        ProviderProfile {
            id: "gemini-official".to_string(),
            family: ProviderFamily::Gemini,
            protocol: ProviderProtocol::GeminiGenerateContent,
            label: "Google Gemini".to_string(),
            api_base: "https://generativelanguage.googleapis.com".to_string(),
            api_key_env: "GEMINI_API_KEY".to_string(),
            model: "gemini-2.5-pro".to_string(),
            best_for: "multimodal coding workflows, huge context, and search-grounded planning"
                .to_string(),
            strengths: vec![
                "Large context for repo-wide prompt assembly".to_string(),
                "Strong multimodal support".to_string(),
                "Grounding-friendly design".to_string(),
            ],
            supports_multimodal: true,
            supports_grounding: true,
            source: ProviderSource::BuiltIn,
        },
        ProviderProfile {
            id: "claude-official".to_string(),
            family: ProviderFamily::Anthropic,
            protocol: ProviderProtocol::AnthropicMessages,
            label: "Anthropic Claude".to_string(),
            api_base: "https://api.anthropic.com".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            model: "claude-sonnet-4.5".to_string(),
            best_for: "deep code review, long edits, and skill-driven orchestration".to_string(),
            strengths: vec![
                "Very strong code review quality".to_string(),
                "Pairs well with reusable skill packs".to_string(),
                "Good at long-form reasoning across files".to_string(),
            ],
            supports_multimodal: true,
            supports_grounding: false,
            source: ProviderSource::BuiltIn,
        },
    ]
}

fn env_profiles() -> Vec<ProviderProfile> {
    let mut profiles = Vec::new();

    if let Some(profile) = openai_env_profile() {
        profiles.push(profile);
    }
    if let Some(profile) = gemini_env_profile() {
        profiles.push(profile);
    }
    if let Some(profile) = anthropic_env_profile() {
        profiles.push(profile);
    }

    profiles
}

fn openai_env_profile() -> Option<ProviderProfile> {
    if !env_flag("CLAUDE_CODE_USE_OPENAI")
        && !any_env_present(&[
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            "OPENAI_API_BASE",
            "OPENAI_MODEL",
            "CODEANY_API_KEY",
            "CODEANY_BASE_URL",
            "CODEANY_MODEL",
        ])
    {
        return None;
    }

    let api_base = first_env_value(&["OPENAI_BASE_URL", "OPENAI_API_BASE", "CODEANY_BASE_URL"])
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let api_key_env = if env_has_value("CODEANY_API_KEY") {
        "CODEANY_API_KEY"
    } else if api_base.contains("openrouter.ai") && env_has_value("OPENROUTER_API_KEY") {
        "OPENROUTER_API_KEY"
    } else {
        "OPENAI_API_KEY"
    };
    let model = first_env_value(&["OPENAI_MODEL", "CODEANY_MODEL"])
        .unwrap_or_else(|| "codex-latest".to_string());

    Some(ProviderProfile {
        id: "openai-env".to_string(),
        family: ProviderFamily::OpenAiCompatible,
        protocol: ProviderProtocol::OpenAiResponses,
        label: "OpenAI Compatible (Env)".to_string(),
        api_base,
        api_key_env: api_key_env.to_string(),
        model,
        best_for: "environment-driven OpenAI-compatible routing and Codex-style agent workflows"
            .to_string(),
        strengths: vec![
            "Compatible with OPENAI_* and CODEANY_* environment variables".to_string(),
            "Helpful for headless scripting and provider swaps".to_string(),
        ],
        supports_multimodal: false,
        supports_grounding: false,
        source: ProviderSource::Environment,
    })
}

fn gemini_env_profile() -> Option<ProviderProfile> {
    if !any_env_present(&[
        "GEMINI_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_BASE_URL",
        "GEMINI_MODEL",
    ]) {
        return None;
    }

    Some(ProviderProfile {
        id: "gemini-env".to_string(),
        family: ProviderFamily::Gemini,
        protocol: ProviderProtocol::GeminiGenerateContent,
        label: "Google Gemini (Env)".to_string(),
        api_base: first_env_value(&["GEMINI_BASE_URL"])
            .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string()),
        api_key_env: if env_has_value("GOOGLE_API_KEY") {
            "GOOGLE_API_KEY".to_string()
        } else {
            "GEMINI_API_KEY".to_string()
        },
        model: first_env_value(&["GEMINI_MODEL"]).unwrap_or_else(|| "gemini-2.5-pro".to_string()),
        best_for: "environment-driven Gemini workflows with multimodal and long-context prompts"
            .to_string(),
        strengths: vec![
            "Picks up common Gemini environment variables automatically".to_string(),
            "Good fit for large prompt assemblies and multimodal review".to_string(),
        ],
        supports_multimodal: true,
        supports_grounding: true,
        source: ProviderSource::Environment,
    })
}

fn anthropic_env_profile() -> Option<ProviderProfile> {
    if !any_env_present(&["ANTHROPIC_API_KEY", "ANTHROPIC_BASE_URL", "ANTHROPIC_MODEL"]) {
        return None;
    }

    Some(ProviderProfile {
        id: "anthropic-env".to_string(),
        family: ProviderFamily::Anthropic,
        protocol: ProviderProtocol::AnthropicMessages,
        label: "Anthropic Claude (Env)".to_string(),
        api_base: first_env_value(&["ANTHROPIC_BASE_URL"])
            .unwrap_or_else(|| "https://api.anthropic.com".to_string()),
        api_key_env: "ANTHROPIC_API_KEY".to_string(),
        model: first_env_value(&["ANTHROPIC_MODEL"])
            .unwrap_or_else(|| "claude-sonnet-4.5".to_string()),
        best_for: "environment-driven Claude review and long-edit workflows".to_string(),
        strengths: vec![
            "Picks up common Anthropic environment variables automatically".to_string(),
            "Useful for review-heavy and skill-driven sessions".to_string(),
        ],
        supports_multimodal: true,
        supports_grounding: false,
        source: ProviderSource::Environment,
    })
}

fn normalize_provider_id(id: &str) -> String {
    match id.trim().to_ascii_lowercase().as_str() {
        "openai" | "codex" | "openai-codex" => "openai-codex".to_string(),
        "openai-env" => "openai-env".to_string(),
        "gemini" | "gemini-official" => "gemini-official".to_string(),
        "gemini-env" => "gemini-env".to_string(),
        "claude" | "anthropic" | "claude-official" => "claude-official".to_string(),
        "anthropic-env" => "anthropic-env".to_string(),
        other => other.to_string(),
    }
}

fn read_optional_trimmed(path: &Path) -> io::Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn parse_provider_config(path: &Path, source: ProviderSource) -> io::Result<Vec<ProviderProfile>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    let mut providers = Vec::new();
    let mut current_id: Option<String> = None;
    let mut current_values = BTreeMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(id) = parse_provider_section(trimmed) {
            if let Some(previous_id) = current_id.take() {
                providers.push(build_profile(previous_id, &current_values, source.clone())?);
                current_values.clear();
            }
            current_id = Some(id);
            continue;
        }

        if let Some((key, value)) = parse_key_value(trimmed) {
            current_values.insert(key.to_string(), value);
        }
    }

    if let Some(id) = current_id {
        providers.push(build_profile(id, &current_values, source)?);
    }

    Ok(providers)
}

fn parse_provider_section(line: &str) -> Option<String> {
    if !line.starts_with("[provider ") || !line.ends_with(']') {
        return None;
    }
    let inner = line
        .trim_start_matches("[provider ")
        .trim_end_matches(']')
        .trim();
    parse_string_literal(inner)
}

fn parse_key_value(line: &str) -> Option<(&str, String)> {
    let mut parts = line.splitn(2, '=');
    let key = parts.next()?.trim();
    let raw_value = parts.next()?.trim();
    Some((key, parse_value(raw_value)))
}

fn parse_value(raw: &str) -> String {
    parse_string_literal(raw).unwrap_or_else(|| raw.to_string())
}

fn parse_string_literal(raw: &str) -> Option<String> {
    if !raw.starts_with('"') || !raw.ends_with('"') || raw.len() < 2 {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for ch in raw[1..raw.len() - 1].chars() {
        if escaped {
            match ch {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                other => out.push(other),
            }
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

fn build_profile(
    id: String,
    values: &BTreeMap<String, String>,
    source: ProviderSource,
) -> io::Result<ProviderProfile> {
    let family = values
        .get("family")
        .and_then(|value| ProviderFamily::parse(value))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("provider {} is missing a valid family", id),
            )
        })?;
    let protocol = values
        .get("protocol")
        .and_then(|value| ProviderProtocol::parse(value))
        .unwrap_or_else(|| family.default_protocol());
    let label = values
        .get("label")
        .cloned()
        .unwrap_or_else(|| id.replace('-', " "));
    let api_base = values.get("api_base").cloned().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("provider {} is missing api_base", id),
        )
    })?;
    let api_key_env = values
        .get("api_key_env")
        .cloned()
        .unwrap_or_else(|| family.default_key_env().to_string());
    let model = values
        .get("model")
        .cloned()
        .unwrap_or_else(|| fallback_model(&family).to_string());
    let best_for = values
        .get("best_for")
        .cloned()
        .unwrap_or_else(|| "custom provider profile".to_string());
    let supports_multimodal = values
        .get("supports_multimodal")
        .and_then(|value| parse_bool(value))
        .unwrap_or_else(|| family.default_multimodal());
    let supports_grounding = values
        .get("supports_grounding")
        .and_then(|value| parse_bool(value))
        .unwrap_or_else(|| family.default_grounding());
    let strengths = values
        .get("strengths")
        .map(|value| {
            value
                .split('|')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(Vec::new);

    Ok(ProviderProfile {
        id: normalize_provider_id(&id),
        family,
        protocol,
        label,
        api_base,
        api_key_env,
        model,
        best_for,
        strengths,
        supports_multimodal,
        supports_grounding,
        source,
    })
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn fallback_model(family: &ProviderFamily) -> &'static str {
    match family {
        ProviderFamily::Gemini => "gemini-2.5-pro",
        ProviderFamily::OpenAiCompatible => "codex-latest",
        ProviderFamily::Anthropic => "claude-sonnet-4.5",
    }
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| parse_bool(&value).unwrap_or(false))
        .unwrap_or(false)
}

fn env_has_value(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn any_env_present(names: &[&str]) -> bool {
    names.iter().any(|name| env_has_value(name))
}

fn first_env_value(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn mask_secret(value: &str) -> String {
    if value.len() <= 8 {
        return "********".to_string();
    }
    format!("{}...{}", &value[..4], &value[value.len() - 4..])
}

#[cfg(test)]
mod tests {
    use super::{
        parse_provider_config, ProviderFamily, ProviderProtocol, ProviderRegistry, ProviderScope,
        ProviderSource,
    };
    use crate::config::AppPaths;
    use crate::trust::TrustState;
    use crate::workspace::Workspace;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_custom_provider_config() {
        let root = unique_dir("providers");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("providers.conf");
        fs::write(
            &path,
            "[provider \"openrouter-codex\"]\nlabel = \"OpenRouter Codex\"\nfamily = \"openai-compatible\"\nprotocol = \"openai-responses\"\napi_base = \"https://openrouter.ai/api/v1\"\napi_key_env = \"OPENROUTER_API_KEY\"\nmodel = \"openai/codex-mini-latest\"\nstrengths = \"relay routing | one key for multiple vendors\"\n",
        )
        .unwrap();

        let profiles =
            parse_provider_config(&path, ProviderSource::GlobalConfig(path.clone())).unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, "openrouter-codex");
        assert_eq!(profiles[0].family, ProviderFamily::OpenAiCompatible);
        assert_eq!(profiles[0].protocol, ProviderProtocol::OpenAiResponses);
        assert_eq!(profiles[0].strengths.len(), 2);
    }

    #[test]
    fn workspace_active_provider_overrides_global() {
        let root = unique_dir("active");
        fs::create_dir_all(root.join(".gemi-clawdex")).unwrap();
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();
        let workspace_root = root.clone();

        fs::write(home.join("active-provider.txt"), "claude-official\n").unwrap();
        fs::write(
            root.join(".gemi-clawdex").join("active-provider.txt"),
            "gemini-official\n",
        )
        .unwrap();

        let registry = ProviderRegistry::load(
            &AppPaths {
                current_dir: root,
                config_home: home,
            },
            &Workspace {
                root: workspace_root.clone(),
                current_dir: workspace_root,
                detected_by: ".gemi-clawdex".to_string(),
            },
            &TrustState {
                kind: None,
                matched_path: None,
                trust_enabled: false,
            },
        )
        .unwrap();

        let current = registry.current_profile(None).unwrap();
        assert_eq!(current.id, "gemini-official");
    }

    #[test]
    fn set_active_persists_workspace_provider() {
        let root = unique_dir("persist");
        fs::create_dir_all(root.join(".gemi-clawdex")).unwrap();
        fs::write(
            root.join(".gemi-clawdex").join("providers.conf"),
            "[provider \"openrouter-codex\"]\nlabel = \"OpenRouter Codex\"\nfamily = \"openai-compatible\"\napi_base = \"https://openrouter.ai/api/v1\"\napi_key_env = \"OPENROUTER_API_KEY\"\nmodel = \"openai/codex-mini-latest\"\n",
        )
        .unwrap();
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();
        let workspace_root = root.clone();
        let paths = AppPaths {
            current_dir: root,
            config_home: home,
        };
        let workspace = Workspace {
            root: workspace_root.clone(),
            current_dir: workspace_root,
            detected_by: ".gemi-clawdex".to_string(),
        };
        let trust = TrustState {
            kind: None,
            matched_path: None,
            trust_enabled: false,
        };

        let mut registry = ProviderRegistry::load(&paths, &workspace, &trust).unwrap();
        registry
            .set_active("openrouter-codex", ProviderScope::Workspace)
            .unwrap();

        let reloaded = ProviderRegistry::load(&paths, &workspace, &trust).unwrap();
        let current = reloaded.current_profile(None).unwrap();
        assert_eq!(current.id, "openrouter-codex");
    }

    fn unique_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("gemi-clawdex-{}-{}", label, stamp))
    }
}
