use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::commands::{CommandCatalog, CustomCommand};
use crate::config::{display_path, SandboxPolicy};
use crate::instructions::InstructionBundle;
use crate::providers::ProviderProfile;
use crate::skills::{SkillCatalog, SkillDefinition};
use crate::trust::TrustState;
use crate::workspace::Workspace;

const TEXT_INJECTION_LINE_LIMIT: usize = 400;
const TEXT_INJECTION_BYTE_LIMIT: usize = 32 * 1024;
const DIRECTORY_LIST_LIMIT: usize = 200;

#[derive(Clone, Debug)]
pub enum PromptAttachment {
    FileText {
        path: PathBuf,
        preview: String,
    },
    DirectoryListing {
        path: PathBuf,
        entries: Vec<String>,
    },
    BinaryReference {
        path: PathBuf,
        media_type: &'static str,
    },
}

#[derive(Clone, Debug)]
pub struct PromptAssembly {
    pub provider: ProviderProfile,
    pub workspace_root: PathBuf,
    pub trust_label: String,
    pub sandbox: SandboxPolicy,
    pub active_command: Option<String>,
    pub active_skill: Option<String>,
    pub attachments: Vec<PromptAttachment>,
    pub pending_shell_commands: Vec<String>,
    pub final_prompt: String,
}

pub struct PromptRequest<'a> {
    pub workspace: &'a Workspace,
    pub trust: &'a TrustState,
    pub sandbox: SandboxPolicy,
    pub provider: ProviderProfile,
    pub instructions: &'a InstructionBundle,
    pub commands: &'a CommandCatalog,
    pub skills: &'a SkillCatalog,
    pub selected_skill: Option<&'a str>,
    pub user_input: &'a str,
}

pub fn assemble_prompt(request: PromptRequest<'_>) -> io::Result<PromptAssembly> {
    let invocation = parse_command_invocation(request.user_input);
    let (active_command, command_text) = match invocation {
        Some((name, args, full_invocation)) => {
            let command = request.commands.find(&name).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("unknown command: {}", name),
                )
            })?;
            let rendered =
                render_command_prompt(command, &args, &full_invocation, request.workspace)?;
            (Some(command.name.clone()), rendered)
        }
        None => (None, request.user_input.to_string()),
    };

    let active_skill = match request.selected_skill {
        Some(name) => Some(
            request
                .skills
                .find(name)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, format!("unknown skill: {}", name))
                })?
                .clone(),
        ),
        None => None,
    };

    let mut attachments = Vec::new();
    let mut prompt_body = command_text;
    let injection = expand_file_injections(&prompt_body, request.workspace)?;
    attachments.extend(injection.attachments);

    let shell = sanitize_shell_blocks(&injection.segments);
    prompt_body = shell
        .expanded
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join("");

    let mut sections = Vec::new();
    sections.push(format!(
        "# Runtime\nProvider: {} ({})\nProvider ID: {}\nProtocol: {}\nAPI Base: {}\nSandbox: {}\nTrust: {}\nWorkspace: {}",
        request.provider.label,
        request.provider.model,
        request.provider.id,
        request.provider.protocol.as_str(),
        request.provider.api_base,
        request.sandbox.as_str(),
        request.trust.status_label(),
        display_path(&request.workspace.root),
    ));

    if !request.instructions.sources.is_empty() {
        let mut instruction_text = String::from("# Repository Instructions");
        for source in &request.instructions.sources {
            instruction_text.push_str("\n\n## ");
            instruction_text.push_str(&source.label);
            instruction_text.push('\n');
            instruction_text.push_str(source.content.trim());
        }
        sections.push(instruction_text);
    }

    if let Some(skill) = &active_skill {
        sections.push(render_skill_section(skill));
    }

    sections.push(format!("# Task\n{}", prompt_body.trim()));

    if !shell.pending.is_empty() {
        let mut shell_section = String::from("# Pending Shell Commands\n");
        for pending in &shell.pending {
            shell_section.push_str("- ");
            shell_section.push_str(pending);
            shell_section.push('\n');
        }
        sections.push(shell_section.trim_end().to_string());
    }

    Ok(PromptAssembly {
        provider: request.provider,
        workspace_root: request.workspace.root.clone(),
        trust_label: request.trust.status_label().to_string(),
        sandbox: request.sandbox,
        active_command,
        active_skill: active_skill.map(|skill| skill.name),
        attachments,
        pending_shell_commands: shell.pending,
        final_prompt: sections.join("\n\n"),
    })
}

fn render_skill_section(skill: &SkillDefinition) -> String {
    format!(
        "# Loaded Skill\nName: {}\nSummary: {}\n\n{}",
        skill.name,
        skill.summary,
        skill.body.trim()
    )
}

fn render_command_prompt(
    command: &CustomCommand,
    args: &str,
    full_invocation: &str,
    workspace: &Workspace,
) -> io::Result<String> {
    let prompt = substitute_command_args(&command.prompt, args, full_invocation);
    let header = format!(
        "# Custom Command\nName: {}\nDescription: {}\nSource: {}\nWorkspace: {}\n",
        command.name,
        command.description,
        display_path(&command.source_path),
        display_path(&workspace.root),
    );
    Ok(format!("{}\n{}", header, prompt))
}

fn substitute_command_args(template: &str, args: &str, full_invocation: &str) -> String {
    if template.contains("{{args}}") {
        return template.replace("{{args}}", args);
    }

    if args.trim().is_empty() {
        return template.to_string();
    }

    format!(
        "{}\n\nRaw command: {}",
        template.trim_end(),
        full_invocation
    )
}

fn parse_command_invocation(input: &str) -> Option<(String, String, String)> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed[1..].splitn(2, char::is_whitespace);
    let name = parts.next()?.trim();
    let args = parts.next().unwrap_or("").trim().to_string();
    Some((name.to_string(), args, trimmed.to_string()))
}

struct FileInjectionResult {
    segments: Vec<PromptSegment>,
    attachments: Vec<PromptAttachment>,
}

fn expand_file_injections(text: &str, workspace: &Workspace) -> io::Result<FileInjectionResult> {
    let spans = find_balanced_spans(text, "@{");
    if spans.is_empty() {
        return Ok(FileInjectionResult {
            segments: vec![PromptSegment {
                text: text.to_string(),
                injected: false,
            }],
            attachments: Vec::new(),
        });
    }

    let mut segments = Vec::new();
    let mut attachments = Vec::new();
    let mut cursor = 0usize;

    for span in spans {
        push_segment(&mut segments, &text[cursor..span.start], false);
        let raw_path = span.body.trim();
        let resolved = resolve_injected_path(raw_path, workspace)?;
        let rendered = render_injected_path(&resolved)?;
        push_segment(&mut segments, &rendered.rendered, true);
        attachments.extend(rendered.attachments);
        cursor = span.end;
    }

    push_segment(&mut segments, &text[cursor..], false);

    Ok(FileInjectionResult {
        segments,
        attachments,
    })
}

struct InjectedRender {
    rendered: String,
    attachments: Vec<PromptAttachment>,
}

fn render_injected_path(path: &Path) -> io::Result<InjectedRender> {
    let metadata = fs::metadata(path)?;
    if metadata.is_dir() {
        let entries = collect_directory_entries(path)?;
        let rendered = format!(
            "<injected-directory path=\"{}\">\n{}\n</injected-directory>",
            display_path(path),
            entries.join("\n")
        );
        return Ok(InjectedRender {
            rendered,
            attachments: vec![PromptAttachment::DirectoryListing {
                path: path.to_path_buf(),
                entries,
            }],
        });
    }

    if let Some(media_type) = detect_binary_media_type(path) {
        let rendered = format!(
            "<attached-file path=\"{}\" media_type=\"{}\">binary reference reserved for provider adapter</attached-file>",
            display_path(path),
            media_type,
        );
        return Ok(InjectedRender {
            rendered,
            attachments: vec![PromptAttachment::BinaryReference {
                path: path.to_path_buf(),
                media_type,
            }],
        });
    }

    let preview = read_text_preview(path)?;
    let rendered = format!(
        "<injected-file path=\"{}\">\n{}\n</injected-file>",
        display_path(path),
        preview
    );
    Ok(InjectedRender {
        rendered,
        attachments: vec![PromptAttachment::FileText {
            path: path.to_path_buf(),
            preview,
        }],
    })
}

fn resolve_injected_path(raw: &str, workspace: &Workspace) -> io::Result<PathBuf> {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        if candidate.starts_with(&workspace.root) || candidate.starts_with(&workspace.current_dir) {
            return Ok(candidate);
        }
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("injected path is outside workspace: {}", raw),
        ));
    }

    let current_candidate = workspace.current_dir.join(raw);
    if current_candidate.exists() {
        return Ok(current_candidate);
    }

    let workspace_candidate = workspace.root.join(raw);
    if workspace_candidate.exists() {
        return Ok(workspace_candidate);
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("unable to resolve injected path: {}", raw),
    ))
}

fn read_text_preview(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    if looks_binary(&bytes) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "binary file is not supported as text injection: {}",
                display_path(path)
            ),
        ));
    }

    let text = String::from_utf8_lossy(&bytes);
    let mut out = String::new();
    let mut total_bytes = 0usize;
    for (index, line) in text.lines().take(TEXT_INJECTION_LINE_LIMIT).enumerate() {
        total_bytes += line.len();
        if total_bytes > TEXT_INJECTION_BYTE_LIMIT {
            break;
        }
        out.push_str(&format!("{}\t{}\n", index + 1, line));
    }
    Ok(out.trim_end().to_string())
}

fn looks_binary(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    let sample_len = bytes.len().min(1024);
    let sample = &bytes[..sample_len];
    let mut suspicious = 0usize;
    for byte in sample {
        if *byte == 0 {
            return true;
        }
        if (*byte < 7) || (*byte > 14 && *byte < 32) {
            suspicious += 1;
        }
    }
    suspicious * 10 > sample_len
}

fn detect_binary_media_type(path: &Path) -> Option<&'static str> {
    let extension = path.extension().and_then(|ext| ext.to_str())?;
    match extension.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "pdf" => Some("application/pdf"),
        _ => None,
    }
}

fn collect_directory_entries(root: &Path) -> io::Result<Vec<String>> {
    let mut entries = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                if entry.file_name().to_string_lossy() != ".git" {
                    stack.push(path);
                }
                continue;
            }

            if file_type.is_file() {
                let relative = path.strip_prefix(root).unwrap_or_else(|_| path.as_path());
                entries.push(relative.to_string_lossy().replace('\\', "/"));
                if entries.len() >= DIRECTORY_LIST_LIMIT {
                    return Ok(entries);
                }
            }
        }
    }

    entries.sort();
    Ok(entries)
}

#[derive(Clone, Debug)]
struct PromptSegment {
    text: String,
    injected: bool,
}

fn push_segment(segments: &mut Vec<PromptSegment>, text: &str, injected: bool) {
    if text.is_empty() {
        return;
    }

    if let Some(last) = segments.last_mut() {
        if last.injected == injected {
            last.text.push_str(text);
            return;
        }
    }

    segments.push(PromptSegment {
        text: text.to_string(),
        injected,
    });
}

struct ShellSanitization {
    expanded: Vec<PromptSegment>,
    pending: Vec<String>,
}

fn sanitize_shell_blocks(segments: &[PromptSegment]) -> ShellSanitization {
    if segments.is_empty() {
        return ShellSanitization {
            expanded: Vec::new(),
            pending: Vec::new(),
        };
    }

    let mut expanded = Vec::new();
    let mut pending = Vec::new();

    for segment in segments {
        if segment.injected {
            expanded.push(segment.clone());
            continue;
        }

        let spans = find_balanced_spans(&segment.text, "!{");
        if spans.is_empty() {
            expanded.push(segment.clone());
            continue;
        }

        let mut cursor = 0usize;
        let mut rendered = String::new();
        for span in spans {
            rendered.push_str(&segment.text[cursor..span.start]);
            let command = span.body.trim().to_string();
            pending.push(command.clone());
            rendered.push_str(&format!(
                "<shell-approval required=\"true\">{}</shell-approval>",
                command
            ));
            cursor = span.end;
        }
        rendered.push_str(&segment.text[cursor..]);
        expanded.push(PromptSegment {
            text: rendered,
            injected: false,
        });
    }

    ShellSanitization { expanded, pending }
}

#[derive(Clone, Debug)]
struct Span {
    start: usize,
    end: usize,
    body: String,
}

fn find_balanced_spans(text: &str, opener: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let opener_bytes = opener.as_bytes();
    let mut index = 0usize;

    while index + opener_bytes.len() <= bytes.len() {
        if &bytes[index..index + opener_bytes.len()] != opener_bytes {
            index += 1;
            continue;
        }

        let start = index;
        index += opener_bytes.len();
        let body_start = index;
        let mut depth = 1i32;

        while index < bytes.len() {
            match bytes[index] as char {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        spans.push(Span {
                            start,
                            end: index + 1,
                            body: text[body_start..index].to_string(),
                        });
                        break;
                    }
                }
                _ => {}
            }
            index += 1;
        }

        if depth != 0 {
            break;
        }

        index += 1;
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::{assemble_prompt, PromptRequest};
    use crate::commands::{CommandCatalog, CustomCommand};
    use crate::instructions::InstructionBundle;
    use crate::providers::builtin_provider;
    use crate::skills::SkillCatalog;
    use crate::trust::TrustState;
    use crate::workspace::Workspace;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn command_prompt_expands_files_and_shells() {
        let root = unique_test_dir("prompt");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("guide.txt"),
            "alpha\nliteral !{echo nope}\nbeta\n",
        )
        .unwrap();

        let workspace = Workspace {
            root: root.clone(),
            current_dir: root.clone(),
            detected_by: ".gemi-clawdex".to_string(),
        };
        let trust = TrustState {
            kind: None,
            matched_path: None,
            trust_enabled: false,
        };
        let commands = CommandCatalog {
            commands: vec![CustomCommand {
                name: "review".to_string(),
                description: "Review a file".to_string(),
                prompt: "Use @{guide.txt}\nTask: {{args}}\nRun !{git status}".to_string(),
                source_path: root.join("review.toml"),
            }],
        };

        let assembly = assemble_prompt(PromptRequest {
            workspace: &workspace,
            trust: &trust,
            sandbox: crate::config::SandboxPolicy::WorkspaceWrite,
            provider: builtin_provider("openai-codex").unwrap(),
            instructions: &InstructionBundle::default(),
            commands: &commands,
            skills: &SkillCatalog::default(),
            selected_skill: None,
            user_input: "/review src/main.rs",
        })
        .unwrap();

        assert!(assembly.final_prompt.contains("Task: src/main.rs"));
        assert!(assembly.final_prompt.contains("<injected-file"));
        assert!(assembly
            .final_prompt
            .contains("<shell-approval required=\"true\">git status</shell-approval>"));
        assert_eq!(
            assembly.pending_shell_commands,
            vec!["git status".to_string()]
        );
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("gemi-clawdex-{}-{}", label, unique))
    }
}
