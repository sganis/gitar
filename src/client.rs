// src/client.rs
use anyhow::Result;
use reqwest::{Client, Proxy};

use crate::config::ResolvedConfig;
use crate::{claude, gemini, openai};

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
        let mut builder = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(120));

        if let Ok(proxy_url) = std::env::var("GITAR_PROXY") {
            let proxy_url = proxy_url.trim();
            if !proxy_url.is_empty() {
                builder = builder.proxy(Proxy::all(proxy_url)?);
            }
        }

        let http = builder.build()?;

        Ok(Self {
            http,
            provider: config.provider.clone(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    fn is_claude_api(&self) -> bool {
        self.provider == "claude" || self.base_url.contains("anthropic.com")
    }

    fn is_gemini_api(&self) -> bool {
        self.provider == "gemini" || self.base_url.contains("generativelanguage.googleapis.com")
    }

    pub async fn chat(&self, system: &str, user: &str, stream: bool) -> Result<String> {
        if self.is_claude_api() {
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

        if self.is_gemini_api() {
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
        if self.is_gemini_api() {
            return gemini::list_models(&self.http, &self.base_url, self.api_key.as_deref()).await;
        }

        if self.is_claude_api() {
            return claude::list_models(&self.http, &self.base_url, self.api_key.as_deref()).await;
        }

        openai::list_models(&self.http, &self.base_url, self.api_key.as_deref()).await
    }
}

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;

    // Stable, explicit URLs (avoid depending on config constants that might be
    // provider names rather than URLs).
    const URL_OPENAI: &str = "https://api.openai.com/v1";
    const URL_CLAUDE: &str = "https://api.anthropic.com/v1";
    const URL_GEMINI: &str = "https://generativelanguage.googleapis.com/v1beta";
    const URL_GROQ: &str = "https://api.groq.com/openai/v1";
    const URL_OLLAMA: &str = "http://localhost:11434/v1";

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

    fn make_config(provider: &str, base_url: &str) -> ResolvedConfig {
        ResolvedConfig {
            provider: provider.into(),
            api_key: None,
            model: "test-model".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: base_url.into(),
            base_branch: "main".into(),
            stream: false,
        }
    }

    #[test]
    fn is_claude_api_detects_provider() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let config = make_config("claude", URL_OPENAI);
        let client = LlmClient::new(&config).unwrap();
        assert!(client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn is_claude_api_detects_url() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let config = make_config("openai", URL_CLAUDE);
        let client = LlmClient::new(&config).unwrap();
        assert!(client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn is_gemini_api_detects_provider() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let config = make_config("gemini", URL_OPENAI);
        let client = LlmClient::new(&config).unwrap();
        assert!(client.is_gemini_api());
        assert!(!client.is_claude_api());
    }

    #[test]
    fn is_gemini_api_detects_url() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let config = make_config("openai", URL_GEMINI);
        let client = LlmClient::new(&config).unwrap();
        assert!(client.is_gemini_api());
        assert!(!client.is_claude_api());
    }

    #[test]
    fn openai_provider_uses_openai_path() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let config = make_config("openai", URL_OPENAI);
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn groq_uses_openai_path() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let config = make_config("groq", URL_GROQ);
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn ollama_uses_openai_path() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let config = make_config("ollama", URL_OLLAMA);
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn provider_detection_mutually_exclusive() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let cases = [
            ("openai", URL_OPENAI, false, false),
            ("claude", URL_CLAUDE, true, false),
            ("gemini", URL_GEMINI, false, true),
            ("groq", URL_GROQ, false, false),
            ("ollama", URL_OLLAMA, false, false),
        ];

        for (provider, url, expected_claude, expected_gemini) in cases {
            let config = make_config(provider, url);
            let client = LlmClient::new(&config).unwrap();
            assert_eq!(
                client.is_claude_api(),
                expected_claude,
                "Claude detection failed for {} ({})",
                provider,
                url
            );
            assert_eq!(
                client.is_gemini_api(),
                expected_gemini,
                "Gemini detection failed for {} ({})",
                provider,
                url
            );
        }
    }

    #[test]
    fn base_url_strips_trailing_slash() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let config = ResolvedConfig {
            provider: "openai".into(),
            api_key: None,
            model: "test".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: "https://api.openai.com/v1/".into(),
            base_branch: "main".into(),
            stream: false,
        };
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.base_url.ends_with('/'));
        assert_eq!(client.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn model_getter_works() {
        let _env = EnvGuard::remove("GITAR_PROXY");

        let config = make_config("openai", URL_OPENAI);
        let client = LlmClient::new(&config).unwrap();
        assert_eq!(client.model(), "test-model");
    }
}