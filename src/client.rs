// src/client.rs
use anyhow::Result;
use reqwest::{Client, Proxy};

use crate::config::ResolvedConfig;
use crate::{claude, gemini, openai};

pub struct LlmClient {
    http: Client,
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
        self.base_url.contains("anthropic.com")
    }

    fn is_gemini_api(&self) -> bool {
        self.base_url.contains("generativelanguage.googleapis.com")
    }

    pub async fn chat(&self, system: &str, user: &str) -> Result<String> {
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
    use crate::config::PROVIDER_CLAUDE;
    use crate::config::PROVIDER_GEMINI;
    use crate::config::PROVIDER_GROQ;
    use crate::config::PROVIDER_OLLAMA;
    use crate::config::PROVIDER_OPENAI;

    fn make_config(base_url: &str) -> ResolvedConfig {
        ResolvedConfig {
            api_key: None,
            model: "test-model".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: base_url.into(),
            base_branch: "main".into(),
        }
    }

    #[test]
    fn is_claude_api_detects_anthropic_url() {
        let config = make_config(PROVIDER_CLAUDE);
        let client = LlmClient::new(&config).unwrap();
        assert!(client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn is_claude_api_false_for_openai() {
        let config = make_config(PROVIDER_OPENAI);
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn is_gemini_api_detects_google_url() {
        let config = make_config(PROVIDER_GEMINI);
        let client = LlmClient::new(&config).unwrap();
        assert!(client.is_gemini_api());
        assert!(!client.is_claude_api());
    }

    #[test]
    fn is_gemini_api_false_for_openai() {
        let config = make_config(PROVIDER_OPENAI);
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn is_gemini_api_false_for_claude() {
        let config = make_config(PROVIDER_CLAUDE);
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_gemini_api());
        assert!(client.is_claude_api());
    }

    #[test]
    fn groq_uses_openai_path() {
        let config = make_config(PROVIDER_GROQ);
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn ollama_uses_openai_path() {
        let config = make_config(PROVIDER_OLLAMA);
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn provider_detection_mutually_exclusive() {
        let urls = [
            (PROVIDER_OPENAI, false, false),
            (PROVIDER_CLAUDE, true, false),
            (PROVIDER_GEMINI, false, true),
            (PROVIDER_GROQ, false, false),
            (PROVIDER_OLLAMA, false, false),
            ("http://localhost:8080", false, false),
        ];

        for (url, expected_claude, expected_gemini) in urls {
            let config = make_config(url);
            let client = LlmClient::new(&config).unwrap();
            assert_eq!(client.is_claude_api(), expected_claude, "Claude detection failed for {}", url);
            assert_eq!(client.is_gemini_api(), expected_gemini, "Gemini detection failed for {}", url);
        }
    }

    #[test]
    fn base_url_strips_trailing_slash() {
        let config = ResolvedConfig {
            api_key: None,
            model: "test".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: "https://api.openai.com/v1/".into(),
            base_branch: "main".into(),
        };
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.base_url.ends_with('/'));
    }

    #[test]
    fn model_getter_works() {
        let config = ResolvedConfig {
            api_key: None,
            model: "gpt-4o".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: PROVIDER_OPENAI.into(),
            base_branch: "main".into(),
        };
        let client = LlmClient::new(&config).unwrap();
        assert_eq!(client.model(), "gpt-4o");
    }
}