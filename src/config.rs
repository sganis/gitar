// src/config.rs
use crate::prompt::{Preset, SecretAction};
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
        "openai" => "gpt-5-chat-latest",
        "claude" => "claude-sonnet-4-5-20250929",
        "gemini" => "gemini-2.5-flash",
        "groq" => "llama-3.3-70b-versatile",
        "ollama" => "llama3.2:latest",
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub base_url: Option<String>,
    pub stream: Option<bool>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_provider: Option<String>,
    pub base_branch: Option<String>,
    pub max_diff_chars: Option<usize>,
    pub insecure_tls: Option<bool>,
    pub preset: Option<String>,
    pub secret_action: Option<String>,
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

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(&p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("Could not determine home directory")?;
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(&path, content).context("Failed to write config file")?;
        println!("Config saved to: {}", path.display());
        Ok(())
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
    pub insecure_tls: bool,
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
        cli_insecure_tls: Option<bool>,
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

        let insecure_tls = cli_insecure_tls.or(file.insecure_tls).unwrap_or(false);

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
            insecure_tls,
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
}
