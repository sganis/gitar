// src/context/prompt.rs
//
// Handles loading and merging custom prompts from TOML files.
// Users can override default prompts via ~/.gitar/prompts.toml or .gitar/prompts.toml

use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use crate::config::Config;
use crate::git;

static CACHED_OVERRIDES: OnceLock<PromptOverrides> = OnceLock::new();

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PromptPair {
    pub system: Option<String>,
    pub user: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PromptOverrides {
    pub commit: Option<PromptPair>,
    pub history: Option<PromptPair>,
    pub pr: Option<PromptPair>,
    pub changelog: Option<PromptPair>,
    pub explain: Option<PromptPair>,
    pub version: Option<PromptPair>,
    pub weekly: Option<PromptPair>,
}

impl PromptOverrides {
    /// Merge another PromptOverrides into self (other takes precedence)
    pub fn merge_with(&mut self, other: &PromptOverrides) {
        merge_pair(&mut self.commit, &other.commit);
        merge_pair(&mut self.history, &other.history);
        merge_pair(&mut self.pr, &other.pr);
        merge_pair(&mut self.changelog, &other.changelog);
        merge_pair(&mut self.explain, &other.explain);
        merge_pair(&mut self.version, &other.version);
        merge_pair(&mut self.weekly, &other.weekly);
    }
}

fn merge_pair(target: &mut Option<PromptPair>, source: &Option<PromptPair>) {
    match (target.as_mut(), source) {
        (Some(t), Some(s)) => {
            if s.system.is_some() {
                t.system = s.system.clone();
            }
            if s.user.is_some() {
                t.user = s.user.clone();
            }
        }
        (None, Some(s)) => {
            *target = Some(s.clone());
        }
        _ => {}
    }
}

/// Load prompt overrides from a TOML file
fn load_from_path(path: &Path) -> Option<PromptOverrides> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
}

/// Load user prompts from ~/.gitar/prompts.toml
fn load_user_prompts() -> Option<PromptOverrides> {
    Config::prompts_path().and_then(|p| load_from_path(&p))
}

/// Load project prompts from .gitar/prompts.toml
fn load_project_prompts() -> Option<PromptOverrides> {
    git::get_repo_root()
        .ok()
        .map(|root| std::path::PathBuf::from(root).join(".gitar").join("prompts.toml"))
        .and_then(|p| load_from_path(&p))
}

/// Load all prompt overrides (cached)
/// Merge order: user prompts first, then project prompts (project wins)
pub fn load_prompt_overrides() -> &'static PromptOverrides {
    CACHED_OVERRIDES.get_or_init(|| {
        let mut overrides = PromptOverrides::default();

        // Load user prompts first
        if let Some(user) = load_user_prompts() {
            overrides.merge_with(&user);
        }

        // Project prompts override user prompts
        if let Some(project) = load_project_prompts() {
            overrides.merge_with(&project);
        }

        overrides
    })
}

/// Get a custom system prompt if defined, otherwise return None
pub fn get_custom_system(key: &str) -> Option<&'static str> {
    let overrides = load_prompt_overrides();
    let pair = match key {
        "commit" => overrides.commit.as_ref(),
        "history" => overrides.history.as_ref(),
        "pr" => overrides.pr.as_ref(),
        "changelog" => overrides.changelog.as_ref(),
        "explain" => overrides.explain.as_ref(),
        "version" => overrides.version.as_ref(),
        "weekly" => overrides.weekly.as_ref(),
        _ => None,
    };
    pair.and_then(|p| p.system.as_deref())
}

/// Get a custom user prompt if defined, otherwise return None
pub fn get_custom_user(key: &str) -> Option<&'static str> {
    let overrides = load_prompt_overrides();
    let pair = match key {
        "commit" => overrides.commit.as_ref(),
        "history" => overrides.history.as_ref(),
        "pr" => overrides.pr.as_ref(),
        "changelog" => overrides.changelog.as_ref(),
        "explain" => overrides.explain.as_ref(),
        "version" => overrides.version.as_ref(),
        "weekly" => overrides.weekly.as_ref(),
        _ => None,
    };
    pair.and_then(|p| p.user.as_deref())
}

/// Validate that a custom prompt contains required placeholders
/// Returns a list of missing placeholders (empty if all present)
pub fn validate_placeholders(key: &str, prompt: &str) -> Vec<&'static str> {
    let required = match key {
        "commit" => vec!["{diff}"],
        "history" => vec!["{diff}", "{original_message}"],
        "pr" => vec!["{branch}", "{commits}", "{stats}", "{diff}"],
        "changelog" => vec!["{range}", "{count}", "{commits}"],
        "explain" => vec!["{stats}", "{diff}"],
        "version" => vec!["{version}", "{diff}"],
        "weekly" => vec!["{stats}", "{diff}"],
        _ => vec![],
    };

    required
        .into_iter()
        .filter(|p| !prompt.contains(p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_prompt_pair() {
        let toml = r#"
[commit]
system = "Custom system"
user = "Custom user {diff}"
"#;
        let overrides: PromptOverrides = toml::from_str(toml).unwrap();
        assert!(overrides.commit.is_some());
        let commit = overrides.commit.unwrap();
        assert_eq!(commit.system, Some("Custom system".to_string()));
        assert_eq!(commit.user, Some("Custom user {diff}".to_string()));
    }

    #[test]
    fn parse_partial_override() {
        let toml = r#"
[commit]
system = "Only system"
"#;
        let overrides: PromptOverrides = toml::from_str(toml).unwrap();
        assert!(overrides.commit.is_some());
        let commit = overrides.commit.unwrap();
        assert_eq!(commit.system, Some("Only system".to_string()));
        assert!(commit.user.is_none());
    }

    #[test]
    fn merge_overrides() {
        let mut base = PromptOverrides {
            commit: Some(PromptPair {
                system: Some("base system".to_string()),
                user: Some("base user".to_string()),
            }),
            ..Default::default()
        };

        let override_prompts = PromptOverrides {
            commit: Some(PromptPair {
                system: Some("override system".to_string()),
                user: None,
            }),
            ..Default::default()
        };

        base.merge_with(&override_prompts);

        let commit = base.commit.unwrap();
        assert_eq!(commit.system, Some("override system".to_string()));
        assert_eq!(commit.user, Some("base user".to_string()));
    }

    #[test]
    fn validate_commit_placeholders() {
        let missing = validate_placeholders("commit", "No placeholders here");
        assert_eq!(missing, vec!["{diff}"]);

        let missing = validate_placeholders("commit", "Has {diff}");
        assert!(missing.is_empty());
    }

    #[test]
    fn validate_pr_placeholders() {
        let missing = validate_placeholders("pr", "Only {diff}");
        assert!(missing.contains(&"{branch}"));
        assert!(missing.contains(&"{commits}"));
        assert!(missing.contains(&"{stats}"));
        assert!(!missing.contains(&"{diff}"));
    }

    #[test]
    fn load_from_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("prompts.toml");
        std::fs::write(
            &path,
            r#"
[commit]
system = "Test system"
"#,
        )
        .unwrap();

        let overrides = load_from_path(&path).unwrap();
        assert!(overrides.commit.is_some());
        assert_eq!(
            overrides.commit.unwrap().system,
            Some("Test system".to_string())
        );
    }
}
