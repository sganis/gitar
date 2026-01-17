// src/config.rs
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
        "claude" => "claude-sonnet-4-5-20250929",
        "gemini" => "gemini-2.5-flash",
        "groq" => "llama-3.3-70b-versatile",
        "ollama" => "llama3.2:latest",
        _ => "gpt-4o",
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
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_provider: Option<String>,
    pub base_branch: Option<String>,
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
        file: &Config,
        default_branch_fn: impl Fn() -> String,
    ) -> Self {
        // Determine provider: CLI > config default > "openai"
        let provider = cli_provider
            .map(|p| normalize_provider(p))
            .or_else(|| file.default_provider.as_ref().map(|p| normalize_provider(p)))
            .unwrap_or("openai")
            .to_string();

        let provider_config = file.get_provider(&provider);

        // Base URL: CLI > provider config > provider default
        let base_url = cli_base_url
            .cloned()
            .or_else(|| provider_config.and_then(|p| p.base_url.clone()))
            .unwrap_or_else(|| provider_to_url(&provider).unwrap_or(PROVIDER_OPENAI).to_string());

        // API key: CLI > provider config > env var
        let env_api_key = env_var_for_provider(&provider)
            .and_then(|var| std::env::var(var).ok());

        let api_key = cli_api_key
            .cloned()
            .or_else(|| provider_config.and_then(|p| p.api_key.clone()))
            .or(env_api_key);

        // Model: CLI > provider config > provider default
        let model = cli_model
            .cloned()
            .or_else(|| provider_config.and_then(|p| p.model.clone()))
            .unwrap_or_else(|| default_model_for_provider(&provider).to_string());

        // Max tokens: CLI > provider config > default
        let max_tokens = cli_max_tokens
            .or_else(|| provider_config.and_then(|p| p.max_tokens))
            .unwrap_or(500);

        // Temperature: CLI > provider config > default
        let temperature = cli_temperature
            .or_else(|| provider_config.and_then(|p| p.temperature))
            .unwrap_or(0.5);

        // Base branch: CLI > config > git default
        let base_branch = cli_base_branch
            .cloned()
            .or_else(|| file.base_branch.clone())
            .unwrap_or_else(default_branch_fn);

        Self {
            provider,
            api_key,
            model,
            max_tokens,
            temperature,
            base_url,
            base_branch,
        }
    }
}

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_is_empty() {
        let config = Config::default();
        assert!(config.default_provider.is_none());
        assert!(config.base_branch.is_none());
        assert!(config.openai.is_none());
        assert!(config.claude.is_none());
    }

    #[test]
    fn config_serializes_to_toml() {
        let config = Config {
            default_provider: Some("claude".into()),
            base_branch: Some("main".into()),
            openai: Some(ProviderConfig {
                api_key: Some("sk-test123".into()),
                model: Some("gpt-4o".into()),
                max_tokens: Some(1000),
                temperature: Some(0.7),
                base_url: None,
            }),
            claude: None,
            gemini: None,
            groq: None,
            ollama: None,
        };
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("default_provider = \"claude\""));
        assert!(toml_str.contains("[openai]"));
    }

    #[test]
    fn config_deserializes_from_toml() {
        let toml_str = r#"
            default_provider = "gemini"
            base_branch = "develop"

            [openai]
            api_key = "sk-test"
            model = "gpt-4o"

            [claude]
            api_key = "sk-ant-test"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_provider, Some("gemini".into()));
        assert!(config.openai.is_some());
        assert!(config.claude.is_some());
    }

    #[test]
    fn config_get_provider() {
        let config = Config {
            openai: Some(ProviderConfig {
                model: Some("gpt-4o".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(config.get_provider("openai").is_some());
        assert!(config.get_provider("claude").is_none());
    }

    #[test]
    fn provider_to_url_all() {
        assert_eq!(provider_to_url("openai"), Some(PROVIDER_OPENAI));
        assert_eq!(provider_to_url("claude"), Some(PROVIDER_CLAUDE));
        assert_eq!(provider_to_url("anthropic"), Some(PROVIDER_CLAUDE));
        assert_eq!(provider_to_url("gemini"), Some(PROVIDER_GEMINI));
        assert_eq!(provider_to_url("groq"), Some(PROVIDER_GROQ));
        assert_eq!(provider_to_url("ollama"), Some(PROVIDER_OLLAMA));
        assert_eq!(provider_to_url("invalid"), None);
    }

    #[test]
    fn normalize_provider_aliases() {
        assert_eq!(normalize_provider("anthropic"), "claude");
        assert_eq!(normalize_provider("google"), "gemini");
        assert_eq!(normalize_provider("local"), "ollama");
        assert_eq!(normalize_provider("CLAUDE"), "claude");
    }

    #[test]
    fn default_model_for_providers() {
        assert_eq!(default_model_for_provider("openai"), "gpt-4o");
        assert_eq!(default_model_for_provider("claude"), "claude-sonnet-4-5-20250929");
        assert_eq!(default_model_for_provider("gemini"), "gemini-2.5-flash");
    }

    #[test]
    fn resolved_config_uses_provider_defaults() {
        std::env::remove_var("OPENAI_API_KEY");
        let file = Config::default();
        let provider = "openai".to_string();
        let resolved = ResolvedConfig::new(
            None, None, None, None, None, Some(&provider), None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.provider, "openai");
        assert_eq!(resolved.model, "gpt-4o");
        assert_eq!(resolved.base_url, PROVIDER_OPENAI);
    }

    #[test]
    fn resolved_config_uses_provider_config() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        let file = Config {
            claude: Some(ProviderConfig {
                api_key: Some("sk-ant-test".into()),
                model: Some("claude-opus-4-5-20251101".into()),
                max_tokens: Some(2000),
                temperature: Some(0.8),
                base_url: None,
            }),
            ..Default::default()
        };
        let provider = "claude".to_string();
        let resolved = ResolvedConfig::new(
            None, None, None, None, None, Some(&provider), None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.provider, "claude");
        assert_eq!(resolved.api_key, Some("sk-ant-test".into()));
        assert_eq!(resolved.model, "claude-opus-4-5-20251101");
        assert_eq!(resolved.max_tokens, 2000);
    }

    #[test]
    fn resolved_config_cli_overrides_provider_config() {
        let file = Config {
            openai: Some(ProviderConfig {
                api_key: Some("file-key".into()),
                model: Some("gpt-4o".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let provider = "openai".to_string();
        let cli_key = "cli-key".to_string();
        let cli_model = "gpt-4o-mini".to_string();
        let resolved = ResolvedConfig::new(
            Some(&cli_key), Some(&cli_model), Some(500), Some(0.9),
            None, Some(&provider), None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.api_key, Some("cli-key".into()));
        assert_eq!(resolved.model, "gpt-4o-mini");
    }

    #[test]
    fn resolved_config_uses_default_provider_from_config() {
        std::env::remove_var("GEMINI_API_KEY");
        let file = Config {
            default_provider: Some("gemini".into()),
            gemini: Some(ProviderConfig {
                api_key: Some("gemini-key".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let resolved = ResolvedConfig::new(
            None, None, None, None, None, None, None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.provider, "gemini");
        assert_eq!(resolved.api_key, Some("gemini-key".into()));
    }
}