use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::display_path;
use crate::prompt::PromptAssembly;

#[derive(Clone, Debug)]
pub struct CheckpointRecord {
    pub file_name: String,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub id: String,
    pub parent_id: Option<String>,
    pub workspace_root: PathBuf,
    pub created_at_secs: u64,
    pub updated_at_secs: u64,
    pub turns: Vec<SessionTurnRecord>,
}

impl SessionRecord {
    pub fn turn_count(&self) -> usize {
        self.turns.len()
    }

    pub fn latest_turn(&self) -> Option<&SessionTurnRecord> {
        self.turns.last()
    }
}

#[derive(Clone, Debug)]
pub struct SessionTurnRecord {
    pub index: usize,
    pub timestamp_secs: u64,
    pub provider_id: String,
    pub provider_label: String,
    pub sandbox: String,
    pub trust_label: String,
    pub active_command: Option<String>,
    pub active_skill: Option<String>,
    pub input: String,
    pub prompt: String,
}

#[derive(Clone, Debug)]
pub struct SessionListEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub workspace_root: PathBuf,
    pub created_at_secs: u64,
    pub updated_at_secs: u64,
    pub turn_count: usize,
    pub latest_provider_id: String,
    pub latest_summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionReuseMode {
    Resume,
    Fork,
}

impl SessionReuseMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Resume => "resume",
            Self::Fork => "fork",
        }
    }
}

pub fn save_checkpoint(base_dir: &Path, assembly: &PromptAssembly) -> io::Result<PathBuf> {
    fs::create_dir_all(base_dir)?;
    let stamp = current_timestamp_secs();
    let slug = slugify(
        assembly
            .active_command
            .as_deref()
            .or_else(|| assembly.active_skill.as_deref())
            .unwrap_or("session"),
    );
    let file_name = format!("{}-{}.md", stamp, slug);
    let path = base_dir.join(file_name);
    let body = format!(
        "# GemiClawdex Checkpoint\n\n- Workspace: {}\n- Provider: {}\n- Trust: {}\n- Sandbox: {}\n\n## Prompt\n\n{}",
        display_path(&assembly.workspace_root),
        assembly.provider.label,
        assembly.trust_label,
        assembly.sandbox.as_str(),
        assembly.final_prompt
    );
    fs::write(&path, body)?;
    Ok(path)
}

pub fn list_checkpoints(base_dir: &Path) -> io::Result<Vec<CheckpointRecord>> {
    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut checkpoints = Vec::new();
    for entry in fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let content = fs::read_to_string(path)?;
        let summary = content
            .lines()
            .find(|line| {
                !line.trim().is_empty() && !line.starts_with('#') && !line.starts_with("- ")
            })
            .unwrap_or("checkpoint")
            .trim()
            .to_string();
        checkpoints.push(CheckpointRecord { file_name, summary });
    }
    checkpoints.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(checkpoints)
}

pub fn save_new_session(
    base_dir: &Path,
    workspace_root: &Path,
    parent_id: Option<&str>,
    user_input: &str,
    assembly: &PromptAssembly,
) -> io::Result<SessionRecord> {
    fs::create_dir_all(base_dir)?;
    let id = generate_session_id(
        assembly
            .active_command
            .as_deref()
            .or_else(|| assembly.active_skill.as_deref())
            .unwrap_or(user_input),
    );
    let now = current_timestamp_secs();
    let turn = build_turn(1, user_input, assembly, now);
    let record = SessionRecord {
        id,
        parent_id: parent_id.map(|value| value.to_string()),
        workspace_root: workspace_root.to_path_buf(),
        created_at_secs: now,
        updated_at_secs: now,
        turns: vec![turn],
    };
    write_session(base_dir, &record)?;
    Ok(record)
}

pub fn append_session_turn(
    base_dir: &Path,
    session_id: &str,
    user_input: &str,
    assembly: &PromptAssembly,
) -> io::Result<SessionRecord> {
    let mut record = load_session(base_dir, session_id)?;
    let now = current_timestamp_secs();
    let next_index = record.turns.len() + 1;
    record
        .turns
        .push(build_turn(next_index, user_input, assembly, now));
    record.updated_at_secs = now;
    write_session(base_dir, &record)?;
    Ok(record)
}

pub fn load_session(base_dir: &Path, session_id: &str) -> io::Result<SessionRecord> {
    let dir = session_dir(base_dir, session_id);
    let metadata_path = dir.join("session.txt");
    let metadata = parse_key_value_block(&fs::read_to_string(&metadata_path)?)?;
    let id = metadata
        .get("id")
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_data("session metadata is missing id"))?;
    let workspace_root = metadata
        .get("workspace_root")
        .map(PathBuf::from)
        .ok_or_else(|| invalid_data("session metadata is missing workspace_root"))?;
    let created_at_secs = parse_u64_field(&metadata, "created_at_secs")?;
    let updated_at_secs = parse_u64_field(&metadata, "updated_at_secs")?;
    let parent_id = metadata
        .get("parent_id")
        .cloned()
        .filter(|value| !value.is_empty());

    let turns_dir = dir.join("turns");
    let mut turn_paths = Vec::new();
    if turns_dir.exists() {
        for entry in fs::read_dir(&turns_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                turn_paths.push(path);
            }
        }
    }
    turn_paths.sort();

    let mut turns = Vec::new();
    for path in turn_paths {
        turns.push(read_turn_file(&path)?);
    }

    Ok(SessionRecord {
        id,
        parent_id,
        workspace_root,
        created_at_secs,
        updated_at_secs,
        turns,
    })
}

pub fn list_sessions(base_dir: &Path) -> io::Result<Vec<SessionListEntry>> {
    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !path.join("session.txt").exists() {
            continue;
        }
        let session_id = entry.file_name().to_string_lossy().into_owned();
        let record = load_session(base_dir, &session_id)?;
        let latest = record
            .latest_turn()
            .cloned()
            .ok_or_else(|| invalid_data("session has no turns"))?;
        entries.push(SessionListEntry {
            id: record.id.clone(),
            parent_id: record.parent_id.clone(),
            workspace_root: record.workspace_root.clone(),
            created_at_secs: record.created_at_secs,
            updated_at_secs: record.updated_at_secs,
            turn_count: record.turn_count(),
            latest_provider_id: latest.provider_id,
            latest_summary: summarize_task(&latest.input),
        });
    }

    entries.sort_by(|left, right| {
        right
            .updated_at_secs
            .cmp(&left.updated_at_secs)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(entries)
}

pub fn fork_session(base_dir: &Path, source_id: &str) -> io::Result<SessionRecord> {
    let source = load_session(base_dir, source_id)?;
    let now = current_timestamp_secs();
    let slug_source = source
        .latest_turn()
        .map(|turn| turn.input.as_str())
        .unwrap_or_else(|| source.id.as_str());
    let forked = SessionRecord {
        id: generate_session_id(slug_source),
        parent_id: Some(source.id.clone()),
        workspace_root: source.workspace_root.clone(),
        created_at_secs: now,
        updated_at_secs: now,
        turns: source.turns,
    };
    write_session(base_dir, &forked)?;
    Ok(forked)
}

pub fn render_session_context(record: &SessionRecord, mode: SessionReuseMode) -> String {
    let mut lines = vec![
        "# Session Continuation".to_string(),
        format!("Mode: {}", mode.as_str()),
        format!("Session ID: {}", record.id),
        format!("Workspace: {}", display_path(&record.workspace_root)),
        format!("Existing turns: {}", record.turn_count()),
    ];

    if let Some(parent_id) = &record.parent_id {
        lines.push(format!("Parent session: {}", parent_id));
    }

    lines.push("Recent turn summaries".to_string());
    for turn in record
        .turns
        .iter()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .iter()
        .rev()
    {
        let mut summary = format!(
            "- [{}] {} | provider={} | sandbox={}",
            turn.index,
            summarize_task(&turn.input),
            turn.provider_id,
            turn.sandbox
        );
        if let Some(command) = &turn.active_command {
            summary.push_str(&format!(" | command={}", command));
        }
        if let Some(skill) = &turn.active_skill {
            summary.push_str(&format!(" | skill={}", skill));
        }
        lines.push(summary);
    }

    lines.join("\n")
}

fn write_session(base_dir: &Path, record: &SessionRecord) -> io::Result<()> {
    let dir = session_dir(base_dir, &record.id);
    let turns_dir = dir.join("turns");
    fs::create_dir_all(&turns_dir)?;

    let mut metadata = Vec::new();
    metadata.push(format!("id={}", record.id));
    metadata.push(format!(
        "parent_id={}",
        record.parent_id.clone().unwrap_or_default()
    ));
    metadata.push(format!(
        "workspace_root={}",
        display_path(&record.workspace_root)
    ));
    metadata.push(format!("created_at_secs={}", record.created_at_secs));
    metadata.push(format!("updated_at_secs={}", record.updated_at_secs));
    fs::write(dir.join("session.txt"), metadata.join("\n"))?;

    for turn in &record.turns {
        write_turn_file(&turns_dir.join(format!("{:04}.txt", turn.index)), turn)?;
    }

    Ok(())
}

fn write_turn_file(path: &Path, turn: &SessionTurnRecord) -> io::Result<()> {
    let mut header = Vec::new();
    header.push(format!("index={}", turn.index));
    header.push(format!("timestamp_secs={}", turn.timestamp_secs));
    header.push(format!("provider_id={}", turn.provider_id));
    header.push(format!("provider_label={}", turn.provider_label));
    header.push(format!("sandbox={}", turn.sandbox));
    header.push(format!("trust_label={}", turn.trust_label));
    header.push(format!(
        "active_command={}",
        turn.active_command.clone().unwrap_or_default()
    ));
    header.push(format!(
        "active_skill={}",
        turn.active_skill.clone().unwrap_or_default()
    ));
    header.push(format!("input_bytes={}", turn.input.as_bytes().len()));
    header.push(format!("prompt_bytes={}", turn.prompt.as_bytes().len()));

    let mut body = header.join("\n");
    body.push_str("\n\n");
    body.push_str(&turn.input);
    body.push_str(&turn.prompt);
    fs::write(path, body)
}

fn read_turn_file(path: &Path) -> io::Result<SessionTurnRecord> {
    let content = fs::read_to_string(path)?;
    let (header, payload) = content
        .split_once("\n\n")
        .ok_or_else(|| invalid_data("session turn is missing payload separator"))?;
    let fields = parse_key_value_block(header)?;
    let input_bytes = parse_usize_field(&fields, "input_bytes")?;
    let prompt_bytes = parse_usize_field(&fields, "prompt_bytes")?;
    let payload_bytes = payload.as_bytes();
    if payload_bytes.len() < input_bytes + prompt_bytes {
        return Err(invalid_data(
            "session turn payload is shorter than declared sizes",
        ));
    }

    let input = String::from_utf8(payload_bytes[..input_bytes].to_vec())
        .map_err(|_| invalid_data("session input is not valid utf-8"))?;
    let prompt = String::from_utf8(payload_bytes[input_bytes..input_bytes + prompt_bytes].to_vec())
        .map_err(|_| invalid_data("session prompt is not valid utf-8"))?;

    Ok(SessionTurnRecord {
        index: parse_usize_field(&fields, "index")?,
        timestamp_secs: parse_u64_field(&fields, "timestamp_secs")?,
        provider_id: required_field(&fields, "provider_id")?,
        provider_label: required_field(&fields, "provider_label")?,
        sandbox: required_field(&fields, "sandbox")?,
        trust_label: required_field(&fields, "trust_label")?,
        active_command: optional_field(&fields, "active_command"),
        active_skill: optional_field(&fields, "active_skill"),
        input,
        prompt,
    })
}

fn build_turn(
    index: usize,
    user_input: &str,
    assembly: &PromptAssembly,
    timestamp_secs: u64,
) -> SessionTurnRecord {
    SessionTurnRecord {
        index,
        timestamp_secs,
        provider_id: assembly.provider.id.clone(),
        provider_label: assembly.provider.label.clone(),
        sandbox: assembly.sandbox.as_str().to_string(),
        trust_label: assembly.trust_label.clone(),
        active_command: assembly.active_command.clone(),
        active_skill: assembly.active_skill.clone(),
        input: user_input.to_string(),
        prompt: assembly.final_prompt.clone(),
    }
}

fn parse_key_value_block(content: &str) -> io::Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| invalid_data("expected key=value line"))?;
        values.insert(key.trim().to_string(), value.to_string());
    }
    Ok(values)
}

fn required_field(values: &BTreeMap<String, String>, key: &str) -> io::Result<String> {
    values
        .get(key)
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_data(format!("missing required field: {}", key)))
}

fn optional_field(values: &BTreeMap<String, String>, key: &str) -> Option<String> {
    values.get(key).cloned().filter(|value| !value.is_empty())
}

fn parse_u64_field(values: &BTreeMap<String, String>, key: &str) -> io::Result<u64> {
    values
        .get(key)
        .ok_or_else(|| invalid_data(format!("missing numeric field: {}", key)))?
        .parse::<u64>()
        .map_err(|_| invalid_data(format!("invalid numeric field: {}", key)))
}

fn parse_usize_field(values: &BTreeMap<String, String>, key: &str) -> io::Result<usize> {
    values
        .get(key)
        .ok_or_else(|| invalid_data(format!("missing numeric field: {}", key)))?
        .parse::<usize>()
        .map_err(|_| invalid_data(format!("invalid numeric field: {}", key)))
}

fn summarize_task(input: &str) -> String {
    let line = input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("session");
    truncate(line.trim(), 96)
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }

    let mut out = String::new();
    for ch in value.chars().take(limit.saturating_sub(3)) {
        out.push(ch);
    }
    out.push_str("...");
    out
}

fn session_dir(base_dir: &Path, session_id: &str) -> PathBuf {
    base_dir.join(session_id)
}

fn generate_session_id(seed: &str) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{}-{}", stamp, slugify(seed))
}

fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '-' || ch == '_' || ch == ':' || ch == ' ') && !out.ends_with('-') {
            out.push('-');
        }
    }
    if out.is_empty() {
        "session".to_string()
    } else {
        out.trim_matches('-').to_string()
    }
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::{
        append_session_turn, fork_session, list_sessions, load_session, render_session_context,
        save_new_session, SessionReuseMode,
    };
    use crate::config::SandboxPolicy;
    use crate::prompt::PromptAssembly;
    use crate::providers::builtin_provider;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn saves_and_loads_session_round_trip() {
        let root = unique_dir("session-round-trip");
        fs::create_dir_all(&root).unwrap();
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).unwrap();

        let record = save_new_session(
            &root,
            &workspace,
            None,
            "/review src/app.rs",
            &sample_assembly(&workspace, "first prompt"),
        )
        .unwrap();

        let loaded = load_session(&root, &record.id).unwrap();
        assert_eq!(loaded.id, record.id);
        assert_eq!(loaded.turn_count(), 1);
        assert_eq!(loaded.latest_turn().unwrap().input, "/review src/app.rs");
        assert_eq!(loaded.latest_turn().unwrap().prompt, "first prompt");
    }

    #[test]
    fn appends_turns_and_lists_latest_first() {
        let root = unique_dir("session-list");
        fs::create_dir_all(&root).unwrap();
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).unwrap();

        let record = save_new_session(
            &root,
            &workspace,
            None,
            "first task",
            &sample_assembly(&workspace, "prompt one"),
        )
        .unwrap();
        append_session_turn(
            &root,
            &record.id,
            "second task",
            &sample_assembly(&workspace, "prompt two"),
        )
        .unwrap();

        let sessions = list_sessions(&root).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].turn_count, 2);
        assert_eq!(sessions[0].latest_summary, "second task");
    }

    #[test]
    fn forks_session_and_preserves_parent_link() {
        let root = unique_dir("session-fork");
        fs::create_dir_all(&root).unwrap();
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).unwrap();

        let original = save_new_session(
            &root,
            &workspace,
            None,
            "review providers",
            &sample_assembly(&workspace, "prompt one"),
        )
        .unwrap();
        let forked = fork_session(&root, &original.id).unwrap();
        let loaded = load_session(&root, &forked.id).unwrap();
        assert_eq!(loaded.parent_id.as_deref(), Some(original.id.as_str()));
        assert_eq!(loaded.turn_count(), 1);

        let context = render_session_context(&loaded, SessionReuseMode::Fork);
        assert!(context.contains("Mode: fork"));
        assert!(context.contains("review providers"));
    }

    fn sample_assembly(workspace_root: &Path, prompt: &str) -> PromptAssembly {
        PromptAssembly {
            provider: builtin_provider("openai-codex").unwrap(),
            workspace_root: workspace_root.to_path_buf(),
            trust_label: "trusted".to_string(),
            sandbox: SandboxPolicy::WorkspaceWrite,
            active_command: Some("review".to_string()),
            active_skill: Some("code-review".to_string()),
            attachments: Vec::new(),
            pending_shell_commands: Vec::new(),
            final_prompt: prompt.to_string(),
        }
    }

    fn unique_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("gemi-clawdex-{}-{}", label, stamp))
    }
}
