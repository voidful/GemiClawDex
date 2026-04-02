use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrustRuleKind {
    Trusted,
    ParentTrusted,
    Untrusted,
}

impl TrustRuleKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "trusted" | "trust" => Some(Self::Trusted),
            "parent" | "parent-trusted" => Some(Self::ParentTrusted),
            "untrusted" | "deny" => Some(Self::Untrusted),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::ParentTrusted => "parent",
            Self::Untrusted => "untrusted",
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrustRule {
    pub kind: TrustRuleKind,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct TrustState {
    pub kind: Option<TrustRuleKind>,
    pub matched_path: Option<PathBuf>,
    pub trust_enabled: bool,
}

impl TrustState {
    pub fn status_label(&self) -> &'static str {
        if !self.trust_enabled {
            return "disabled";
        }
        match self.kind {
            Some(TrustRuleKind::Trusted) => "trusted",
            Some(TrustRuleKind::ParentTrusted) => "trusted-via-parent",
            Some(TrustRuleKind::Untrusted) => "untrusted",
            None => "unknown",
        }
    }

    pub fn restricts_project_config(&self) -> bool {
        self.trust_enabled
            && self.kind != Some(TrustRuleKind::Trusted)
            && self.kind != Some(TrustRuleKind::ParentTrusted)
    }
}

#[derive(Clone, Debug, Default)]
pub struct TrustStore {
    pub rules: Vec<TrustRule>,
}

impl TrustStore {
    pub fn load(path: &Path) -> io::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        let mut rules = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let mut parts = trimmed.splitn(2, '\t');
            let kind = parts.next().and_then(TrustRuleKind::parse);
            let path_part = parts.next();
            if let (Some(kind), Some(path_part)) = (kind, path_part) {
                rules.push(TrustRule {
                    kind,
                    path: PathBuf::from(path_part),
                });
            }
        }
        Ok(Self { rules })
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut lines = Vec::new();
        for rule in &self.rules {
            lines.push(format!(
                "{}\t{}",
                rule.kind.as_str(),
                rule.path.to_string_lossy()
            ));
        }
        fs::write(path, lines.join("\n"))
    }

    pub fn upsert(&mut self, rule: TrustRule) {
        self.rules.retain(|existing| existing.path != rule.path);
        self.rules.push(rule);
        self.rules.sort_by(|left, right| left.path.cmp(&right.path));
    }

    pub fn evaluate(&self, path: &Path, trust_enabled: bool) -> TrustState {
        if !trust_enabled {
            return TrustState {
                kind: None,
                matched_path: None,
                trust_enabled,
            };
        }

        let mut best_match: Option<&TrustRule> = None;
        for rule in &self.rules {
            if path.starts_with(&rule.path) {
                match best_match {
                    Some(existing) => {
                        if rule.path.components().count() > existing.path.components().count() {
                            best_match = Some(rule);
                        }
                    }
                    None => best_match = Some(rule),
                }
            }
        }

        TrustState {
            kind: best_match.map(|rule| rule.kind.clone()),
            matched_path: best_match.map(|rule| rule.path.clone()),
            trust_enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TrustRule, TrustRuleKind, TrustStore};
    use std::path::PathBuf;

    #[test]
    fn longest_matching_rule_wins() {
        let mut store = TrustStore::default();
        store.upsert(TrustRule {
            kind: TrustRuleKind::ParentTrusted,
            path: PathBuf::from("/tmp/projects"),
        });
        store.upsert(TrustRule {
            kind: TrustRuleKind::Untrusted,
            path: PathBuf::from("/tmp/projects/danger"),
        });

        let state = store.evaluate(PathBuf::from("/tmp/projects/danger/app").as_path(), true);
        assert_eq!(state.kind, Some(TrustRuleKind::Untrusted));
    }
}
