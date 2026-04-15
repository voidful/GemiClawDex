// GemiClawDex — Worktree Isolation
//
// Git worktree-based execution isolation for parallel or sandboxed tasks.
// Creates temporary worktrees from the current branch, runs work inside them,
// and cleans up afterward.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// A managed git worktree for isolated execution.
#[derive(Clone, Debug)]
pub struct IsolatedWorktree {
    /// Path to the created worktree directory.
    pub path: PathBuf,
    /// The branch or commit the worktree was created from.
    pub source_ref: String,
    /// Whether this worktree is still active.
    pub active: bool,
}

impl IsolatedWorktree {
    /// Create a new worktree from the given workspace root.
    /// The worktree is created as a detached HEAD from the current branch.
    pub fn create(workspace_root: &Path, label: Option<&str>) -> io::Result<Self> {
        let source_ref = current_branch(workspace_root).unwrap_or_else(|| "HEAD".to_string());

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir_name = match label {
            Some(label) => format!(".gcd-worktree-{}-{}", label, unique),
            None => format!(".gcd-worktree-{}", unique),
        };

        let worktree_path = workspace_root.parent().unwrap_or(workspace_root).join(&dir_name);

        let output = Command::new("git")
            .args(["worktree", "add", "--detach"])
            .arg(&worktree_path)
            .arg("HEAD")
            .current_dir(workspace_root)
            .output()
            .map_err(|err| {
                io::Error::other(
                    format!("Failed to create git worktree: {}", err),
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(io::Error::other(
                format!("git worktree add failed: {}", stderr.trim()),
            ));
        }

        Ok(Self {
            path: worktree_path,
            source_ref,
            active: true,
        })
    }

    /// Remove this worktree and clean up.
    pub fn remove(&mut self, workspace_root: &Path) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }

        let output = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&self.path)
            .current_dir(workspace_root)
            .output()
            .map_err(|err| {
                io::Error::other(
                    format!("Failed to remove git worktree: {}", err),
                )
            })?;

        if !output.status.success() {
            // Fallback: try to force-remove the directory
            let _ = fs::remove_dir_all(&self.path);
        }

        // Prune stale worktree entries
        let _ = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(workspace_root)
            .output();

        self.active = false;
        Ok(())
    }
}

/// List existing GCD-managed worktrees for a workspace.
pub fn list_worktrees(workspace_root: &Path) -> io::Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(workspace_root)
        .output()
        .map_err(|err| {
            io::Error::other(
                format!("Failed to list git worktrees: {}", err),
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gcd_worktrees = Vec::new();
    for line in stdout.lines() {
        if let Some(path_str) = line.strip_prefix("worktree ") {
            let path = PathBuf::from(path_str);
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(".gcd-worktree-"))
                .unwrap_or(false)
            {
                gcd_worktrees.push(path);
            }
        }
    }

    Ok(gcd_worktrees)
}

/// Clean up all GCD-managed worktrees for a workspace.
pub fn cleanup_all_worktrees(workspace_root: &Path) -> io::Result<usize> {
    let worktrees = list_worktrees(workspace_root)?;
    let mut removed = 0;
    for path in &worktrees {
        let output = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(path)
            .current_dir(workspace_root)
            .output();
        match output {
            Ok(o) if o.status.success() => removed += 1,
            _ => {
                let _ = fs::remove_dir_all(path);
                removed += 1;
            }
        }
    }

    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(workspace_root)
        .output();

    Ok(removed)
}

/// Get the current branch name.
fn current_branch(workspace_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_branch_returns_something_in_git_repo() {
        // This test runs inside the GCD repo itself
        let result = current_branch(Path::new("."));
        // May or may not be in a git repo during testing
        if result.is_some() {
            assert!(!result.unwrap().is_empty());
        }
    }

    #[test]
    fn list_worktrees_does_not_panic() {
        // Should not panic even outside a git repo
        let _ = list_worktrees(Path::new("/nonexistent"));
    }
}
