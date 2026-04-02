use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::AppPaths;
use crate::trust::TrustState;
use crate::workspace::Workspace;

#[derive(Clone, Debug)]
pub struct SkillDefinition {
    pub name: String,
    pub summary: String,
    pub body: String,
    pub source_path: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct SkillCatalog {
    pub skills: Vec<SkillDefinition>,
}

impl SkillCatalog {
    pub fn load(paths: &AppPaths, workspace: &Workspace, trust: &TrustState) -> io::Result<Self> {
        if trust.restricts_project_config() {
            return Ok(Self::default());
        }

        let mut skills = Vec::new();
        load_skills_from_dir(&paths.global_skills_dir(), &mut skills)?;
        load_skills_from_dir(&workspace.project_skills_dir(), &mut skills)?;
        skills.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(Self { skills })
    }

    pub fn find(&self, name: &str) -> Option<&SkillDefinition> {
        self.skills.iter().find(|skill| skill.name == name)
    }
}

fn load_skills_from_dir(root: &Path, skills: &mut Vec<SkillDefinition>) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }

    let mut files = walk_markdown_files(root)?;
    files.sort();
    for file in files {
        let relative = file.strip_prefix(root).unwrap_or_else(|_| file.as_path());
        let file_name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let name = if file_name.eq_ignore_ascii_case("SKILL.md") {
            relative
                .parent()
                .map(path_to_skill_name)
                .unwrap_or_else(|| "skill".to_string())
        } else {
            path_to_skill_name(relative.with_extension("").as_path())
        };
        let body = fs::read_to_string(&file)?;
        let summary = first_non_empty_line(&body);
        skills.push(SkillDefinition {
            name,
            summary,
            body,
            source_path: file,
        });
    }

    Ok(())
}

fn walk_markdown_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file()
                && path.extension().and_then(|ext| ext.to_str()) == Some("md")
            {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn path_to_skill_name(path: &Path) -> String {
    let mut parts = Vec::new();
    for component in path.components() {
        parts.push(component.as_os_str().to_string_lossy().into_owned());
    }
    parts.join(":")
}

fn first_non_empty_line(body: &str) -> String {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let without_prefix = trimmed.trim_start_matches('#').trim();
        return without_prefix.to_string();
    }
    "Reusable skill".to_string()
}
