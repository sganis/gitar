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

// =============================================================================
// CONFIG FILE
// =============================================================================
pub const CONFIG_FILENAME: &str = ".gitar.toml";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub base_url: Option<String>,
    pub base_branch: Option<String>,
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
}

// =============================================================================
// RESOLVED CONFIG
// =============================================================================
pub struct ResolvedConfig {
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
        let provider_url = cli_provider
            .and_then(|p| provider_to_url(p).map(String::from));

        let base_url = provider_url
            .or_else(|| cli_base_url.cloned())
            .or_else(|| file.base_url.clone())
            .unwrap_or_else(|| PROVIDER_OPENAI.to_string());

        let is_claude = base_url.contains("anthropic.com");
        let is_gemini = base_url.contains("generativelanguage.googleapis.com");
        let is_groq = base_url.contains("api.groq.com");

        let default_model = if is_claude {
            "claude-sonnet-4-5-20250929"
        } else if is_gemini {
            "gemini-2.5-flash"
        } else {
            "gpt-5-chat-latest"
        };

        let env_api_key = if is_claude {
            std::env::var("ANTHROPIC_API_KEY").ok()
        } else if is_groq {
            std::env::var("GROQ_API_KEY").ok()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        } else if is_gemini {
            std::env::var("GEMINI_API_KEY").ok()
        } else {
            std::env::var("OPENAI_API_KEY").ok()
        };

        let api_key = cli_api_key.cloned()
            .or(env_api_key)
            .or_else(|| file.api_key.clone());

        Self {
            api_key,
            model: cli_model.cloned()
                .or_else(|| file.model.clone())
                .unwrap_or_else(|| default_model.to_string()),
            max_tokens: cli_max_tokens.or(file.max_tokens).unwrap_or(500),
            temperature: cli_temperature.or(file.temperature).unwrap_or(0.5),
            base_url,
            base_branch: cli_base_branch.cloned()
                .or_else(|| file.base_branch.clone())
                .unwrap_or_else(default_branch_fn),
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
        assert!(config.api_key.is_none());
        assert!(config.model.is_none());
        assert!(config.max_tokens.is_none());
        assert!(config.temperature.is_none());
        assert!(config.base_url.is_none());
        assert!(config.base_branch.is_none());
    }

    #[test]
    fn config_serializes_to_toml() {
        let config = Config {
            api_key: Some("sk-test123".into()),
            model: Some("gpt-4o".into()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            base_url: None,
            base_branch: Some("main".into()),
        };
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("api_key = \"sk-test123\""));
        assert!(toml_str.contains("model = \"gpt-4o\""));
        assert!(toml_str.contains("max_tokens = 4096"));
        assert!(toml_str.contains("temperature = 0.7"));
        assert!(toml_str.contains("base_branch = \"main\""));
    }

    #[test]
    fn config_deserializes_from_toml() {
        let toml_str = r#"
            api_key = "sk-test"
            model = "gpt-4"
            max_tokens = 2048
            temperature = 0.5
            base_branch = "develop"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.api_key, Some("sk-test".into()));
        assert_eq!(config.model, Some("gpt-4".into()));
        assert_eq!(config.max_tokens, Some(2048));
        assert_eq!(config.temperature, Some(0.5));
        assert_eq!(config.base_branch, Some("develop".into()));
    }

    #[test]
    fn config_deserializes_partial_toml() {
        let toml_str = r#"model = "gpt-4""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.api_key.is_none());
        assert_eq!(config.model, Some("gpt-4".into()));
        assert!(config.max_tokens.is_none());
    }

    #[test]
    fn config_handles_empty_toml() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.api_key.is_none());
        assert!(config.model.is_none());
    }

    #[test]
    fn config_roundtrip() {
        let original = Config {
            api_key: Some("test-key".into()),
            model: Some("gpt-5-chat-latest".into()),
            max_tokens: Some(8192),
            temperature: Some(0.5),
            base_url: Some("https://api.example.com".into()),
            base_branch: Some("develop".into()),
        };
        let toml_str = toml::to_string(&original).unwrap();
        let restored: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(original.api_key, restored.api_key);
        assert_eq!(original.model, restored.model);
        assert_eq!(original.max_tokens, restored.max_tokens);
        assert_eq!(original.temperature, restored.temperature);
        assert_eq!(original.base_url, restored.base_url);
        assert_eq!(original.base_branch, restored.base_branch);
    }

    #[test]
    fn config_path_in_home() {
        if let Some(path) = Config::path() {
            let path_str = path.to_string_lossy();
            assert!(path_str.ends_with(".gitar.toml"));
        }
    }

    #[test]
    fn config_filename_correct() {
        assert_eq!(CONFIG_FILENAME, ".gitar.toml");
    }

    #[test]
    fn provider_to_url_openai() {
        assert_eq!(provider_to_url("openai"), Some(PROVIDER_OPENAI));
        assert_eq!(provider_to_url("OPENAI"), Some(PROVIDER_OPENAI));
        assert_eq!(provider_to_url("OpenAI"), Some(PROVIDER_OPENAI));
    }

    #[test]
    fn provider_to_url_claude() {
        assert_eq!(provider_to_url("claude"), Some(PROVIDER_CLAUDE));
        assert_eq!(provider_to_url("CLAUDE"), Some(PROVIDER_CLAUDE));
        assert_eq!(provider_to_url("anthropic"), Some(PROVIDER_CLAUDE));
        assert_eq!(provider_to_url("Anthropic"), Some(PROVIDER_CLAUDE));
    }

    #[test]
    fn provider_to_url_gemini() {
        assert_eq!(provider_to_url("gemini"), Some(PROVIDER_GEMINI));
        assert_eq!(provider_to_url("GEMINI"), Some(PROVIDER_GEMINI));
    }

    #[test]
    fn provider_to_url_groq() {
        assert_eq!(provider_to_url("groq"), Some(PROVIDER_GROQ));
        assert_eq!(provider_to_url("GROQ"), Some(PROVIDER_GROQ));
    }

    #[test]
    fn provider_to_url_ollama() {
        assert_eq!(provider_to_url("ollama"), Some(PROVIDER_OLLAMA));
        assert_eq!(provider_to_url("OLLAMA"), Some(PROVIDER_OLLAMA));
        assert_eq!(provider_to_url("local"), Some(PROVIDER_OLLAMA));
        assert_eq!(provider_to_url("LOCAL"), Some(PROVIDER_OLLAMA));
    }

    #[test]
    fn provider_to_url_invalid() {
        assert_eq!(provider_to_url("invalid"), None);
        assert_eq!(provider_to_url("azure"), None);
        assert_eq!(provider_to_url(""), None);
    }

    #[test]
    fn provider_constants_valid_urls() {
        assert!(PROVIDER_OPENAI.starts_with("https://"));
        assert!(PROVIDER_CLAUDE.starts_with("https://"));
        assert!(PROVIDER_GEMINI.starts_with("https://"));
        assert!(PROVIDER_GROQ.starts_with("https://"));
        assert!(PROVIDER_OLLAMA.starts_with("http://"));
    }

    #[test]
    fn provider_ollama_url_is_localhost() {
        assert!(PROVIDER_OLLAMA.contains("localhost:11434"));
    }

    #[test]
    fn resolved_config_uses_defaults() {
        std::env::remove_var("OPENAI_API_KEY");
        let file = Config::default();
        let resolved = ResolvedConfig::new(
            None, None, None, None, None, None, None,
            &file, || "main".into(),
        );
        assert!(resolved.api_key.is_none());
        assert_eq!(resolved.model, "gpt-5-chat-latest");
        assert_eq!(resolved.max_tokens, 500);
        assert_eq!(resolved.temperature, 0.5);
        assert_eq!(resolved.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn resolved_config_file_overrides_defaults() {
        std::env::remove_var("OPENAI_API_KEY");
        let file = Config {
            api_key: Some("file-key".into()),
            model: Some("gpt-3.5-turbo".into()),
            max_tokens: Some(2048),
            temperature: Some(0.3),
            base_url: Some("https://custom.api".into()),
            base_branch: Some("develop".into()),
        };
        let resolved = ResolvedConfig::new(
            None, None, None, None, None, None, None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.api_key, Some("file-key".into()));
        assert_eq!(resolved.model, "gpt-3.5-turbo");
        assert_eq!(resolved.max_tokens, 2048);
        assert_eq!(resolved.temperature, 0.3);
        assert_eq!(resolved.base_url, "https://custom.api");
        assert_eq!(resolved.base_branch, "develop");
    }

    #[test]
    fn resolved_config_cli_overrides_file() {
        let file = Config {
            api_key: Some("file-key".into()),
            model: Some("gpt-3.5-turbo".into()),
            max_tokens: Some(2048),
            temperature: Some(0.3),
            base_url: Some("https://file.api".into()),
            base_branch: Some("develop".into()),
        };
        let cli_key = "cli-key".to_string();
        let cli_model = "claude-3".to_string();
        let cli_url = "https://cli.api".to_string();
        let cli_branch = "main".to_string();
        let resolved = ResolvedConfig::new(
            Some(&cli_key), Some(&cli_model), Some(1024), Some(0.9),
            Some(&cli_url), None, Some(&cli_branch),
            &file, || "main".into(),
        );
        assert_eq!(resolved.api_key, Some("cli-key".into()));
        assert_eq!(resolved.model, "claude-3");
        assert_eq!(resolved.max_tokens, 1024);
        assert_eq!(resolved.temperature, 0.9);
        assert_eq!(resolved.base_url, "https://cli.api");
        assert_eq!(resolved.base_branch, "main");
    }

    #[test]
    fn resolved_config_uses_claude_default_model() {
        let cli_url = "https://api.anthropic.com/v1".to_string();
        let file = Config::default();
        let resolved = ResolvedConfig::new(
            None, None, None, None, Some(&cli_url), None, None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.model, "claude-sonnet-4-5-20250929");
        assert_eq!(resolved.base_url, "https://api.anthropic.com/v1");
    }

    #[test]
    fn resolved_config_uses_gemini_default_model() {
        let cli_url = "https://generativelanguage.googleapis.com".to_string();
        let file = Config::default();
        let resolved = ResolvedConfig::new(
            None, None, None, None, Some(&cli_url), None, None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.model, "gemini-2.5-flash");
    }

    #[test]
    fn resolved_config_provider_sets_claude_url() {
        let provider = "claude".to_string();
        let file = Config::default();
        let resolved = ResolvedConfig::new(
            None, None, None, None, None, Some(&provider), None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.base_url, PROVIDER_CLAUDE);
        assert_eq!(resolved.model, "claude-sonnet-4-5-20250929");
    }

    #[test]
    fn resolved_config_provider_sets_gemini_url() {
        let provider = "gemini".to_string();
        let file = Config::default();
        let resolved = ResolvedConfig::new(
            None, None, None, None, None, Some(&provider), None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.base_url, PROVIDER_GEMINI);
        assert_eq!(resolved.model, "gemini-2.5-flash");
    }

    #[test]
    fn resolved_config_provider_overrides_base_url() {
        let cli_url = "https://custom.api".to_string();
        let provider = "claude".to_string();
        let file = Config::default();
        let resolved = ResolvedConfig::new(
            None, None, None, None, Some(&cli_url), Some(&provider), None,
            &file, || "main".into(),
        );
        assert_eq!(resolved.base_url, PROVIDER_CLAUDE);
    }
}