use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SandboxPolicy {
    Off,
    ReadOnly,
    WorkspaceWrite,
    Container,
}

impl SandboxPolicy {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "read-only" | "readonly" => Some(Self::ReadOnly),
            "workspace-write" | "write" => Some(Self::WorkspaceWrite),
            "container" => Some(Self::Container),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::Container => "container",
        }
    }

    pub fn summary(&self) -> &'static str {
        match self {
            Self::Off => "No sandboxing. Fastest, least isolated.",
            Self::ReadOnly => "Read-only workspace inspection. Safest default for review work.",
            Self::WorkspaceWrite => "Workspace writes allowed, host-wide changes discouraged.",
            Self::Container => "Strong isolation for untrusted repos or risky automation.",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub current_dir: PathBuf,
    pub config_home: PathBuf,
}

impl AppPaths {
    pub fn detect(current_dir: PathBuf) -> Self {
        Self {
            current_dir,
            config_home: default_config_home(),
        }
    }

    pub fn trust_store_path(&self) -> PathBuf {
        self.config_home.join("trusted-folders.txt")
    }

    pub fn checkpoints_dir(&self) -> PathBuf {
        self.config_home.join("checkpoints")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.config_home.join("sessions")
    }

    pub fn global_commands_dir(&self) -> PathBuf {
        self.config_home.join("commands")
    }

    pub fn global_skills_dir(&self) -> PathBuf {
        self.config_home.join("skills")
    }

    pub fn global_provider_registry_path(&self) -> PathBuf {
        self.config_home.join("providers.conf")
    }

    pub fn global_active_provider_path(&self) -> PathBuf {
        self.config_home.join("active-provider.txt")
    }
}

#[derive(Clone, Debug)]
pub struct RuntimePreferences {
    pub default_provider_id: Option<String>,
    pub default_sandbox: SandboxPolicy,
    pub trust_enabled: bool,
}

impl Default for RuntimePreferences {
    fn default() -> Self {
        Self {
            default_provider_id: default_provider_from_env(),
            default_sandbox: env::var("GEMICLAWDEX_SANDBOX")
                .ok()
                .and_then(|value| SandboxPolicy::parse(value.trim()))
                .unwrap_or(SandboxPolicy::WorkspaceWrite),
            trust_enabled: env::var("GEMICLAWDEX_TRUST")
                .ok()
                .map(|value| parse_env_bool(&value))
                .unwrap_or(true),
        }
    }
}

fn default_provider_from_env() -> Option<String> {
    if let Some(value) = env::var("GEMICLAWDEX_PROVIDER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Some(value);
    }

    if env_flag("CLAUDE_CODE_USE_OPENAI") {
        return Some("openai-env".to_string());
    }

    if any_env_present(&[
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "OPENAI_API_BASE",
        "OPENAI_MODEL",
        "CODEANY_API_KEY",
        "CODEANY_BASE_URL",
        "CODEANY_MODEL",
    ]) {
        return Some("openai-env".to_string());
    }

    if any_env_present(&[
        "GEMINI_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_BASE_URL",
        "GEMINI_MODEL",
    ]) {
        return Some("gemini-env".to_string());
    }

    if any_env_present(&["ANTHROPIC_API_KEY", "ANTHROPIC_BASE_URL", "ANTHROPIC_MODEL"]) {
        return Some("anthropic-env".to_string());
    }

    None
}

fn default_config_home() -> PathBuf {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".gemi-clawdex")
}

fn parse_env_bool(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "off" | "no"
    )
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| parse_env_bool(&value))
        .unwrap_or(false)
}

fn any_env_present(names: &[&str]) -> bool {
    names.iter().any(|name| {
        env::var(name)
            .ok()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    })
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
