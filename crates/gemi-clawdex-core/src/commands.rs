use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::AppPaths;
use crate::trust::TrustState;
use crate::workspace::Workspace;

#[derive(Clone, Debug)]
pub struct CustomCommand {
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub source_path: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct CommandCatalog {
    pub commands: Vec<CustomCommand>,
}

impl CommandCatalog {
    pub fn load(paths: &AppPaths, workspace: &Workspace, trust: &TrustState) -> io::Result<Self> {
        if trust.restricts_project_config() {
            return Ok(Self::default());
        }

        let mut commands = Vec::new();
        load_commands_from_dir(&paths.global_commands_dir(), &mut commands)?;
        load_commands_from_dir(&workspace.project_commands_dir(), &mut commands)?;

        commands.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(Self { commands })
    }

    pub fn find(&self, name: &str) -> Option<&CustomCommand> {
        self.commands.iter().find(|command| command.name == name)
    }
}

fn load_commands_from_dir(root: &Path, commands: &mut Vec<CustomCommand>) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }

    let mut files = walk_files(root)?;
    files.sort();
    for file in files {
        if file.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }

        let relative = file.strip_prefix(root).unwrap_or_else(|_| file.as_path());
        let name = relative_path_to_command_name(relative);
        let content = fs::read_to_string(&file)?;
        let prompt = extract_toml_string(&content, "prompt").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("command {} is missing prompt", file.to_string_lossy()),
            )
        })?;
        let description = extract_toml_string(&content, "description")
            .unwrap_or_else(|| fallback_description(&name));

        commands.push(CustomCommand {
            name,
            description,
            prompt,
            source_path: file,
        });
    }

    Ok(())
}

fn walk_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn relative_path_to_command_name(relative: &Path) -> String {
    let without_extension = relative.with_extension("");
    let mut parts = Vec::new();
    for component in without_extension.components() {
        parts.push(component.as_os_str().to_string_lossy().into_owned());
    }
    parts.join(":")
}

fn fallback_description(name: &str) -> String {
    format!("Run {} command", name.replace(':', " "))
}

fn extract_toml_string(content: &str, key: &str) -> Option<String> {
    let key_pos = find_assignment_start(content, key)?;
    let rest = &content[key_pos + key.len()..];
    let rest = rest.trim_start();
    if !rest.starts_with('=') {
        return None;
    }
    let value = rest[1..].trim_start();

    if let Some(body) = value.strip_prefix("\"\"\"") {
        let end = body.find("\"\"\"")?;
        return Some(body[..end].to_string());
    }

    if !value.starts_with('"') {
        return None;
    }

    let mut escaped = false;
    let mut out = String::new();
    for ch in value[1..].chars() {
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

        match ch {
            '\\' => escaped = true,
            '"' => return Some(out),
            other => out.push(other),
        }
    }

    None
}

fn find_assignment_start(content: &str, key: &str) -> Option<usize> {
    let mut offset = 0usize;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(remainder) = trimmed.strip_prefix(key) {
            if remainder.trim_start().starts_with('=') {
                let indent = line.len() - trimmed.len();
                return Some(offset + indent);
            }
        }
        offset += line.len();
    }

    let trimmed = content.trim_start();
    if let Some(remainder) = trimmed.strip_prefix(key) {
        let indent = content.len() - trimmed.len();
        if remainder.trim_start().starts_with('=') {
            return Some(indent);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::extract_toml_string;

    #[test]
    fn parses_multiline_prompt() {
        let input = "description = \"Review code\"\nprompt = \"\"\"hello\nworld\"\"\"\n";
        let prompt = extract_toml_string(input, "prompt");
        assert_eq!(prompt.as_deref(), Some("hello\nworld"));
    }
}
