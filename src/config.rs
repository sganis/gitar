// src/config.rs
use crate::context::{Preset, SecretAction};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_MAX_DIFF_CHARS: usize = 50_000;

// =============================================================================
// PROVIDER CONSTANTS
// =============================================================================

pub const PROVIDER_OPENAI: &str = "https://api.openai.com/v1";
pub const PROVIDER_CLAUDE: &str = "https://api.anthropic.com/v1";
pub const PROVIDER_GEMINI: &str = "https://generativelanguage.googleapis.com";
pub const PROVIDER_GROQ: &str = "https://api.groq.com/openai/v1";
pub const PROVIDER_OLLAMA: &str = "http://localhost:11434/v1";

// =============================================================================
// DEFAULT MODELS - Edit these to change defaults for each provider
// =============================================================================

pub const DEFAULT_MODEL_OPENAI: &str = "gpt-4.1-2025-04-14";
pub const DEFAULT_MODEL_CLAUDE: &str = "claude-sonnet-4-5-20250514";
pub const DEFAULT_MODEL_GEMINI: &str = "gemini-2.5-flash";
pub const DEFAULT_MODEL_GROQ: &str = "llama-3.3-70b-versatile";
pub const DEFAULT_MODEL_OLLAMA: &str = "llama3.2:latest";

pub fn provider_to_url(provider: &str) -> Option<&'static str> {
    match provider.to_lowercase().as_str() {
        "openai" => Some(PROVIDER_OPENAI),
        "claude" | "anthropic" => Some(PROVIDER_CLAUDE),
        "gemini" | "google" => Some(PROVIDER_GEMINI),
        "groq" => Some(PROVIDER_GROQ),
        "ollama" | "local" => Some(PROVIDER_OLLAMA),
        _ => None,
    }
}

pub fn normalize_provider(provider: &str) -> &'static str {
    match provider.to_lowercase().as_str() {
        "anthropic" => "claude",
        "google" => "gemini",
        "local" => "ollama",
        "openai" => "openai",
        "claude" => "claude",
        "gemini" => "gemini",
        "groq" => "groq",
        "ollama" => "ollama",
        _ => "openai",
    }
}

fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "openai" => DEFAULT_MODEL_OPENAI,
        "claude" => DEFAULT_MODEL_CLAUDE,
        "gemini" => DEFAULT_MODEL_GEMINI,
        "groq" => DEFAULT_MODEL_GROQ,
        "ollama" => DEFAULT_MODEL_OLLAMA,
        _ => "",
    }
}

fn env_var_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("OPENAI_API_KEY"),
        "claude" => Some("ANTHROPIC_API_KEY"),
        "gemini" => Some("GEMINI_API_KEY"),
        "groq" => Some("GROQ_API_KEY"),
        "ollama" => None,
        _ => Some("OPENAI_API_KEY"),
    }
}

// =============================================================================
// CONFIG FILE
// =============================================================================

pub const CONFIG_FILENAME: &str = ".gitar.toml";
pub const SYSTEM_CONFIG_ENV: &str = "GITAR_SYSTEM_CONFIG";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub base_url: Option<String>,
    pub stream: Option<bool>,
}

impl ProviderConfig {
    /// Merge self with another config. Self takes precedence (non-None values win).
    pub fn merge_with(&mut self, base: &ProviderConfig) {
        if self.api_key.is_none() {
            self.api_key = base.api_key.clone();
        }
        if self.model.is_none() {
            self.model = base.model.clone();
        }
        if self.max_tokens.is_none() {
            self.max_tokens = base.max_tokens;
        }
        if self.temperature.is_none() {
            self.temperature = base.temperature;
        }
        if self.base_url.is_none() {
            self.base_url = base.base_url.clone();
        }
        if self.stream.is_none() {
            self.stream = base.stream;
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_provider: Option<String>,
    pub base_branch: Option<String>,
    pub max_diff_chars: Option<usize>,
    pub insecure: Option<bool>,
    pub preset: Option<String>,
    pub secret_action: Option<String>,
    /// When true, commands default to --apply mode (execute instead of dry-run)
    pub auto_apply: Option<bool>,
    pub openai: Option<ProviderConfig>,
    pub claude: Option<ProviderConfig>,
    pub gemini: Option<ProviderConfig>,
    pub groq: Option<ProviderConfig>,
    pub ollama: Option<ProviderConfig>,
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(CONFIG_FILENAME))
    }

    /// Returns the system config path from GITAR_SYSTEM_CONFIG env var, if set.
    pub fn system_path() -> Option<PathBuf> {
        std::env::var(SYSTEM_CONFIG_ENV)
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    }

    /// Load config from a specific path.
    pub fn load_from_path(path: &std::path::Path) -> Option<Self> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
    }

    /// Load config with cascading: system config -> user config.
    /// User config values override system config values.
    pub fn load() -> Self {
        // Start with system config if available
        let mut config = Self::system_path()
            .and_then(|p| Self::load_from_path(&p))
            .unwrap_or_default();

        // Merge user config on top (user values take precedence)
        if let Some(user_config) = Self::path().and_then(|p| Self::load_from_path(&p)) {
            config.merge_with(&user_config);
        }

        config
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("Could not determine home directory")?;
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(&path, content).context("Failed to write config file")?;
        println!("Config saved to: {}", path.display());
        Ok(())
    }

    /// Merge another config into self. The other config's values take precedence.
    pub fn merge_with(&mut self, other: &Config) {
        // Top-level fields: other takes precedence if set
        if other.default_provider.is_some() {
            self.default_provider = other.default_provider.clone();
        }
        if other.base_branch.is_some() {
            self.base_branch = other.base_branch.clone();
        }
        if other.max_diff_chars.is_some() {
            self.max_diff_chars = other.max_diff_chars;
        }
        if other.insecure.is_some() {
            self.insecure = other.insecure;
        }
        if other.preset.is_some() {
            self.preset = other.preset.clone();
        }
        if other.secret_action.is_some() {
            self.secret_action = other.secret_action.clone();
        }
        if other.auto_apply.is_some() {
            self.auto_apply = other.auto_apply;
        }

        // Provider configs: merge each provider section
        Self::merge_provider(&mut self.openai, &other.openai);
        Self::merge_provider(&mut self.claude, &other.claude);
        Self::merge_provider(&mut self.gemini, &other.gemini);
        Self::merge_provider(&mut self.groq, &other.groq);
        Self::merge_provider(&mut self.ollama, &other.ollama);
    }

    fn merge_provider(target: &mut Option<ProviderConfig>, source: &Option<ProviderConfig>) {
        match (target.as_mut(), source) {
            (Some(t), Some(s)) => {
                // Both exist: merge source into target (source takes precedence)
                if s.api_key.is_some() {
                    t.api_key = s.api_key.clone();
                }
                if s.model.is_some() {
                    t.model = s.model.clone();
                }
                if s.max_tokens.is_some() {
                    t.max_tokens = s.max_tokens;
                }
                if s.temperature.is_some() {
                    t.temperature = s.temperature;
                }
                if s.base_url.is_some() {
                    t.base_url = s.base_url.clone();
                }
                if s.stream.is_some() {
                    t.stream = s.stream;
                }
            }
            (None, Some(s)) => {
                // Only source exists: clone it
                *target = Some(s.clone());
            }
            _ => {
                // Target exists or both are None: nothing to do
            }
        }
    }

    pub fn get_provider(&self, name: &str) -> Option<&ProviderConfig> {
        match name {
            "openai" => self.openai.as_ref(),
            "claude" => self.claude.as_ref(),
            "gemini" => self.gemini.as_ref(),
            "groq" => self.groq.as_ref(),
            "ollama" => self.ollama.as_ref(),
            _ => None,
        }
    }

    pub fn get_provider_mut(&mut self, name: &str) -> &mut ProviderConfig {
        match name {
            "openai" => self.openai.get_or_insert_with(ProviderConfig::default),
            "claude" => self.claude.get_or_insert_with(ProviderConfig::default),
            "gemini" => self.gemini.get_or_insert_with(ProviderConfig::default),
            "groq" => self.groq.get_or_insert_with(ProviderConfig::default),
            "ollama" => self.ollama.get_or_insert_with(ProviderConfig::default),
            _ => self.openai.get_or_insert_with(ProviderConfig::default),
        }
    }
}

// =============================================================================
// RESOLVED CONFIG
// =============================================================================

pub struct ResolvedConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub base_url: String,
    pub base_branch: String,
    pub stream: bool,
    pub max_diff_chars: usize,
    pub insecure: bool,
    pub preset: Preset,
    pub secret_action: SecretAction,
}

impl ResolvedConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cli_api_key: Option<&String>,
        cli_model: Option<&String>,
        cli_max_tokens: Option<u32>,
        cli_temperature: Option<f32>,
        cli_base_url: Option<&String>,
        cli_provider: Option<&String>,
        cli_base_branch: Option<&String>,
        cli_stream: Option<bool>,
        cli_insecure: Option<bool>,
        cli_preset: Option<&String>,
        file: &Config,
        default_branch_fn: impl Fn() -> String,
        repo_root: &std::path::Path,
    ) -> Self {
        let provider = cli_provider
            .map(|p| normalize_provider(p))
            .or_else(|| {
                file.default_provider
                    .as_ref()
                    .map(|p| normalize_provider(p))
            })
            .unwrap_or("openai")
            .to_string();

        let provider_config = file.get_provider(&provider);

        let base_url = cli_base_url
            .cloned()
            .or_else(|| provider_config.and_then(|p| p.base_url.clone()))
            .unwrap_or_else(|| {
                provider_to_url(&provider)
                    .unwrap_or(PROVIDER_OPENAI)
                    .to_string()
            });

        let env_api_key = env_var_for_provider(&provider).and_then(|var| std::env::var(var).ok());

        let api_key = cli_api_key
            .cloned()
            .or_else(|| provider_config.and_then(|p| p.api_key.clone()))
            .or(env_api_key);

        let model = cli_model
            .cloned()
            .or_else(|| provider_config.and_then(|p| p.model.clone()))
            .unwrap_or_else(|| default_model_for_provider(&provider).to_string());

        let max_tokens = cli_max_tokens
            .or_else(|| provider_config.and_then(|p| p.max_tokens))
            .unwrap_or(500);

        let temperature = cli_temperature
            .or_else(|| provider_config.and_then(|p| p.temperature))
            .unwrap_or(0.5);

        let base_branch = cli_base_branch
            .cloned()
            .or_else(|| file.base_branch.clone())
            .unwrap_or_else(default_branch_fn);

        let stream = cli_stream
            .or_else(|| provider_config.and_then(|p| p.stream))
            .unwrap_or(false);

        let max_diff_chars = file.max_diff_chars.unwrap_or(DEFAULT_MAX_DIFF_CHARS);

        let insecure = cli_insecure.or(file.insecure).unwrap_or(false);

        let preset = Preset::resolve(cli_preset, file.preset.as_ref(), repo_root);

        let secret_action = file
            .secret_action
            .as_ref()
            .and_then(|s| SecretAction::from_str(s))
            .unwrap_or_default();

        Self {
            provider,
            api_key,
            model,
            max_tokens,
            temperature,
            base_url,
            base_branch,
            stream,
            max_diff_chars,
            insecure,
            preset,
            secret_action,
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_repo() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn config_default_empty() {
        let c = Config::default();
        assert!(c.default_provider.is_none());
        assert!(c.secret_action.is_none());
        assert!(c.auto_apply.is_none());
    }

    #[test]
    fn config_auto_apply_parses() {
        let toml = r#"auto_apply = true"#;
        let c: Config = toml::from_str(toml).unwrap();
        assert_eq!(c.auto_apply, Some(true));
    }

    #[test]
    fn provider_to_url_all() {
        assert_eq!(provider_to_url("openai"), Some(PROVIDER_OPENAI));
        assert_eq!(provider_to_url("claude"), Some(PROVIDER_CLAUDE));
        assert_eq!(provider_to_url("gemini"), Some(PROVIDER_GEMINI));
        assert_eq!(provider_to_url("ollama"), Some(PROVIDER_OLLAMA));
    }

    #[test]
    fn resolved_defaults() {
        std::env::remove_var("OPENAI_API_KEY");
        let repo = temp_repo();
        let r = ResolvedConfig::new(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &Config::default(),
            || "main".into(),
            repo.path(),
        );
        assert_eq!(r.provider, "openai");
        assert_eq!(r.secret_action, SecretAction::Redact);
    }

    #[test]
    fn resolved_secret_action_from_config() {
        let repo = temp_repo();
        let file = Config {
            secret_action: Some("block".into()),
            ..Default::default()
        };
        let r = ResolvedConfig::new(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &file,
            || "main".into(),
            repo.path(),
        );
        assert_eq!(r.secret_action, SecretAction::Block);
    }

    #[test]
    fn resolved_preset_detects() {
        let repo = temp_repo();
        std::fs::write(repo.path().join("Cargo.toml"), "").unwrap();
        let r = ResolvedConfig::new(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &Config::default(),
            || "main".into(),
            repo.path(),
        );
        assert_eq!(r.preset, Preset::Rust);
    }

    #[test]
    fn config_merge_user_overrides_system() {
        let mut system = Config {
            default_provider: Some("openai".into()),
            base_branch: Some("main".into()),
            openai: Some(ProviderConfig {
                api_key: Some("system-key".into()),
                model: Some("gpt-4".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let user = Config {
            default_provider: Some("claude".into()),
            // base_branch not set - should keep system value
            openai: Some(ProviderConfig {
                model: Some("gpt-4o".into()), // override model
                // api_key not set - should keep system value
                ..Default::default()
            }),
            ..Default::default()
        };

        system.merge_with(&user);

        // User values should take precedence
        assert_eq!(system.default_provider, Some("claude".into()));
        // System values should be kept when user doesn't override
        assert_eq!(system.base_branch, Some("main".into()));
        // Provider merge: user model wins, system api_key kept
        let openai = system.openai.unwrap();
        assert_eq!(openai.model, Some("gpt-4o".into()));
        assert_eq!(openai.api_key, Some("system-key".into()));
    }

    #[test]
    fn config_merge_user_adds_provider() {
        let mut system = Config {
            default_provider: Some("openai".into()),
            openai: Some(ProviderConfig {
                api_key: Some("openai-key".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let user = Config {
            claude: Some(ProviderConfig {
                api_key: Some("claude-key".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        system.merge_with(&user);

        // System provider should still exist
        assert!(system.openai.is_some());
        // User provider should be added
        assert_eq!(
            system.claude.as_ref().unwrap().api_key,
            Some("claude-key".into())
        );
    }

    #[test]
    fn config_load_from_path_works() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.toml");
        std::fs::write(
            &path,
            r#"
default_provider = "gemini"
[gemini]
api_key = "test-key"
"#,
        )
        .unwrap();

        let config = Config::load_from_path(&path).unwrap();
        assert_eq!(config.default_provider, Some("gemini".into()));
        assert_eq!(
            config.gemini.as_ref().unwrap().api_key,
            Some("test-key".into())
        );
    }
}
