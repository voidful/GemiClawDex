use std::fs;
use std::io;

use crate::trust::TrustState;
use crate::workspace::Workspace;

#[derive(Clone, Debug)]
pub struct InstructionSource {
    pub label: String,
    pub content: String,
}

#[derive(Clone, Debug, Default)]
pub struct InstructionBundle {
    pub sources: Vec<InstructionSource>,
}

impl InstructionBundle {
    pub fn load(workspace: &Workspace, trust: &TrustState) -> io::Result<Self> {
        if trust.restricts_project_config() {
            return Ok(Self::default());
        }

        let mut sources = Vec::new();
        for name in &["AGENTS.md", "GEMINI.md", "CLAUDE.md", "GEMICLAWDEX.md"] {
            let path = workspace.root.join(name);
            if path.exists() {
                let content = fs::read_to_string(&path)?;
                sources.push(InstructionSource {
                    label: (*name).to_string(),
                    content,
                });
            }
        }
        Ok(Self { sources })
    }
}
