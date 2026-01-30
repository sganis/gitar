// src/client.rs
use anyhow::{Context, Result};
use reqwest::{Certificate, Client, ClientBuilder, Proxy};

use crate::color::{styled, warning_style};
use crate::config::{provider_to_url, ProviderConfig, ResolvedConfig};
use crate::provider::{anthropic, claudecode, gemini, openai};

/// Environment variable for custom CA certificate file path.
pub const CA_FILE_ENV: &str = "GITAR_CA_FILE";

/// Load custom CA certificate from GITAR_CA_FILE if set.
/// Supports both PEM and DER formats (tries PEM first, then DER).
fn load_ca_certificate() -> Result<Option<Certificate>> {
    let ca_path = match std::env::var(CA_FILE_ENV) {
        Ok(p) if !p.trim().is_empty() => p,
        _ => return Ok(None),
    };

    let cert_data = std::fs::read(&ca_path)
        .with_context(|| format!("Failed to read CA certificate from {}", ca_path))?;

    // Try PEM format first, then DER
    let cert = Certificate::from_pem(&cert_data)
        .or_else(|_| Certificate::from_der(&cert_data))
        .with_context(|| {
            format!(
                "Failed to parse CA certificate from {} (tried PEM and DER formats)",
                ca_path
            )
        })?;

    Ok(Some(cert))
}

/// Apply common client configuration: proxy and CA certificate.
fn apply_common_config(mut builder: ClientBuilder) -> Result<ClientBuilder> {
    // Custom CA certificate
    if let Some(cert) = load_ca_certificate()? {
        builder = builder.add_root_certificate(cert);
    }

    // Proxy
    if let Ok(proxy_url) = std::env::var("GITAR_PROXY") {
        let proxy_url = proxy_url.trim();
        if !proxy_url.is_empty() {
            builder = builder.proxy(Proxy::all(proxy_url)?);
        }
    }

    Ok(builder)
}

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
        if config.insecure {
            eprintln!(
                "{} TLS certificate verification disabled (insecure)",
                styled("[WARN]", warning_style())
            );
            builder = builder.danger_accept_invalid_certs(true);
        }

        builder = apply_common_config(builder)?;

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
        let builder = Client::builder().timeout(std::time::Duration::from_secs(120));
        let builder = apply_common_config(builder)?;

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
                "anthropic" => "claude-sonnet-4-5-20250929".to_string(),
                "gemini" => "gemini-2.5-flash".to_string(),
                "groq" => "llama-3.3-70b-versatile".to_string(),
                "ollama" => "llama3.2:latest".to_string(),
                "claudecode" => "sonnet".to_string(),
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

    fn is_anthropic(&self) -> bool {
        self.provider == "anthropic" || self.base_url.contains("anthropic.com")
    }

    fn is_gemini(&self) -> bool {
        self.provider == "gemini" || self.base_url.contains("generativelanguage.googleapis.com")
    }

    fn is_claudecode(&self) -> bool {
        self.provider == "claudecode"
    }

    pub async fn chat(&self, system: &str, user: &str, stream: bool) -> Result<String> {
        // Claude Code uses subprocess, not HTTP - check first
        if self.is_claudecode() {
            return claudecode::chat(&self.model, system, user, stream).await;
        }
        if self.is_anthropic() {
            return anthropic::chat(
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
        // Claude Code uses subprocess, not HTTP - check first
        if self.is_claudecode() {
            return claudecode::list_models().await;
        }
        if self.is_gemini() {
            return gemini::list_models(&self.http, &self.base_url, self.api_key.as_deref()).await;
        }
        if self.is_anthropic() {
            return anthropic::list_models(&self.http, &self.base_url, self.api_key.as_deref()).await;
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
            insecure: false,
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
    fn detect_anthropic() {
        let _e1 = EnvGuard::remove("GITAR_PROXY");
        let _e2 = EnvGuard::remove(CA_FILE_ENV);
        let c = LlmClient::new(&make_config("anthropic", "https://api.openai.com")).unwrap();
        assert!(c.is_anthropic());
    }

    #[test]
    fn detect_gemini() {
        let _e1 = EnvGuard::remove("GITAR_PROXY");
        let _e2 = EnvGuard::remove(CA_FILE_ENV);
        let c = LlmClient::new(&make_config("gemini", "https://api.openai.com")).unwrap();
        assert!(c.is_gemini());
    }

    #[test]
    fn detect_by_url() {
        let _e1 = EnvGuard::remove("GITAR_PROXY");
        let _e2 = EnvGuard::remove(CA_FILE_ENV);
        let c = LlmClient::new(&make_config("openai", "https://api.anthropic.com/v1")).unwrap();
        assert!(c.is_anthropic());
    }
}
