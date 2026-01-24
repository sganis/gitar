// src/client.rs
use anyhow::Result;
use reqwest::{Client, Proxy};

use crate::color::{styled, warning_style};
use crate::config::{provider_to_url, ProviderConfig, ResolvedConfig};
use crate::provider::{claude, gemini, openai};

pub struct LlmClient {
    http: Client,
    provider: String,
    base_url: String,
    api_key: Option<String>,
    model: String,
    max_tokens: u32,
    temperature: f32,
}

impl LlmClient {
    pub fn new(config: &ResolvedConfig) -> Result<Self> {
        let mut builder = Client::builder().timeout(std::time::Duration::from_secs(120));

        // TLS verification: secure by default
        if config.insecure_tls {
            eprintln!(
                "{} TLS certificate verification disabled (insecure)",
                styled("[WARN]", warning_style())
            );
            builder = builder.danger_accept_invalid_certs(true);
        }

        if let Ok(proxy_url) = std::env::var("ALL_PROXY") {
            let proxy_url = proxy_url.trim();
            if !proxy_url.is_empty() {
                builder = builder.proxy(Proxy::all(proxy_url)?);
            }
        }

        Ok(Self {
            http: builder.build()?,
            provider: config.provider.clone(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        })
    }

    /// Create a client with just a provider and provider config (for init command)
    pub fn new_with_provider(provider: &str, provider_config: &ProviderConfig) -> Result<Self> {
        let mut builder = Client::builder().timeout(std::time::Duration::from_secs(120));

        if let Ok(proxy_url) = std::env::var("ALL_PROXY") {
            let proxy_url = proxy_url.trim();
            if !proxy_url.is_empty() {
                builder = builder.proxy(Proxy::all(proxy_url)?);
            }
        }

        let base_url = provider_config
            .base_url
            .clone()
            .or_else(|| provider_to_url(provider).map(String::from))
            .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", provider))?;

        let model = provider_config
            .model
            .clone()
            .unwrap_or_else(|| match provider {
                "openai" => "gpt-5-chat-latest".to_string(),
                "claude" => "claude-sonnet-4-5-20250929".to_string(),
                "gemini" => "gemini-2.5-flash".to_string(),
                "groq" => "llama-3.3-70b-versatile".to_string(),
                "ollama" => "llama3.2:latest".to_string(),
                _ => "gpt-5-chat-latest".to_string(),
            });

        Ok(Self {
            http: builder.build()?,
            provider: provider.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: provider_config.api_key.clone(),
            model,
            max_tokens: provider_config.max_tokens.unwrap_or(4096),
            temperature: provider_config.temperature.unwrap_or(0.7),
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn provider(&self) -> &str {
        &self.provider
    }

    fn is_claude(&self) -> bool {
        self.provider == "claude" || self.base_url.contains("anthropic.com")
    }

    fn is_gemini(&self) -> bool {
        self.provider == "gemini" || self.base_url.contains("generativelanguage.googleapis.com")
    }

    pub async fn chat(&self, system: &str, user: &str, stream: bool) -> Result<String> {
        if self.is_claude() {
            return claude::chat(
                &self.http,
                &self.base_url,
                self.api_key.as_deref(),
                &self.model,
                self.max_tokens,
                self.temperature,
                system,
                user,
                stream,
            )
            .await;
        }
        if self.is_gemini() {
            return gemini::chat(
                &self.http,
                &self.base_url,
                self.api_key.as_deref(),
                &self.model,
                self.max_tokens,
                self.temperature,
                system,
                user,
                stream,
            )
            .await;
        }
        openai::chat(
            &self.http,
            &self.base_url,
            self.api_key.as_deref(),
            &self.model,
            self.max_tokens,
            self.temperature,
            system,
            user,
            stream,
        )
        .await
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        if self.is_gemini() {
            return gemini::list_models(&self.http, &self.base_url, self.api_key.as_deref()).await;
        }
        if self.is_claude() {
            return claude::list_models(&self.http, &self.base_url, self.api_key.as_deref()).await;
        }
        openai::list_models(&self.http, &self.base_url, self.api_key.as_deref()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;
    use crate::context::{Preset, SecretAction};

    fn make_config(provider: &str, url: &str) -> ResolvedConfig {
        ResolvedConfig {
            provider: provider.into(),
            api_key: None,
            model: "test".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: url.into(),
            base_branch: "main".into(),
            stream: false,
            max_diff_chars: 10_000,
            insecure_tls: false,
            preset: Preset::Default,
            secret_action: SecretAction::Redact,
        }
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }
    impl EnvGuard {
        fn remove(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, prev }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn detect_claude() {
        let _e = EnvGuard::remove("ALL_PROXY");
        let c = LlmClient::new(&make_config("claude", "https://api.openai.com")).unwrap();
        assert!(c.is_claude());
    }

    #[test]
    fn detect_gemini() {
        let _e = EnvGuard::remove("ALL_PROXY");
        let c = LlmClient::new(&make_config("gemini", "https://api.openai.com")).unwrap();
        assert!(c.is_gemini());
    }

    #[test]
    fn detect_by_url() {
        let _e = EnvGuard::remove("ALL_PROXY");
        let c = LlmClient::new(&make_config("openai", "https://api.anthropic.com/v1")).unwrap();
        assert!(c.is_claude());
    }
}
