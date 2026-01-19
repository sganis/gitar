// src/client.rs
use anyhow::Result;
use reqwest::{Client, Proxy};

use crate::config::ResolvedConfig;
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
            eprintln!("\x1b[33m⚠️  TLS certificate verification disabled (insecure)\x1b[0m");
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

    pub fn model(&self) -> &str { &self.model }
    pub fn provider(&self) -> &str { &self.provider }

    fn is_claude(&self) -> bool {
        self.provider == "claude" || self.base_url.contains("anthropic.com")
    }

    fn is_gemini(&self) -> bool {
        self.provider == "gemini" || self.base_url.contains("generativelanguage.googleapis.com")
    }

    pub async fn chat(&self, system: &str, user: &str, stream: bool) -> Result<String> {
        if self.is_claude() {
            return claude::chat(&self.http, &self.base_url, self.api_key.as_deref(),
                &self.model, self.max_tokens, self.temperature, system, user, stream).await;
        }
        if self.is_gemini() {
            return gemini::chat(&self.http, &self.base_url, self.api_key.as_deref(),
                &self.model, self.max_tokens, self.temperature, system, user, stream).await;
        }
        openai::chat(&self.http, &self.base_url, self.api_key.as_deref(),
            &self.model, self.max_tokens, self.temperature, system, user, stream).await
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
    use crate::prompt::{Preset, SecretAction};

    fn make_config(provider: &str, url: &str) -> ResolvedConfig {
        ResolvedConfig {
            provider: provider.into(), api_key: None, model: "test".into(),
            max_tokens: 500, temperature: 0.5, base_url: url.into(),
            base_branch: "main".into(), stream: false, max_diff_chars: 10_000,
            insecure_tls: false, preset: Preset::Default, secret_action: SecretAction::Redact,
        }
    }

    struct EnvGuard { key: &'static str, prev: Option<String> }
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