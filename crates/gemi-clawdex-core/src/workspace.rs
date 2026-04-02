use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Workspace {
    pub root: PathBuf,
    pub current_dir: PathBuf,
    pub detected_by: String,
}

impl Workspace {
    pub fn discover(start: &Path) -> io::Result<Self> {
        let start = if start.exists() {
            fs::canonicalize(start)?
        } else {
            start.to_path_buf()
        };

        let mut current = start.as_path();
        loop {
            if let Some(marker) = detect_marker(current) {
                return Ok(Self {
                    root: current.to_path_buf(),
                    current_dir: start.clone(),
                    detected_by: marker.to_string(),
                });
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => {
                    return Ok(Self {
                        root: start.clone(),
                        current_dir: start,
                        detected_by: "cwd-fallback".to_string(),
                    })
                }
            }
        }
    }

    pub fn project_commands_dir(&self) -> PathBuf {
        self.root.join(".gemi-clawdex").join("commands")
    }

    pub fn project_skills_dir(&self) -> PathBuf {
        self.root.join(".gemi-clawdex").join("skills")
    }
}

fn detect_marker(path: &Path) -> Option<&'static str> {
    let markers = [
        (".gemi-clawdex", ".gemi-clawdex"),
        (".git", ".git"),
        ("Cargo.toml", "Cargo.toml"),
        ("package.json", "package.json"),
    ];
    for (entry, label) in markers.iter() {
        if path.join(entry).exists() {
            return Some(*label);
        }
    }
    None
}
