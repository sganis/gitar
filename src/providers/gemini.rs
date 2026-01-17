// src/gemini.rs
use anyhow::{bail, Context, Result};
use reqwest::Client;

use crate::types::*;

fn normalize_base_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1beta") {
        base.to_string()
    } else {
        format!("{}/v1beta", base)
    }
}

fn normalize_model_path(model: &str) -> String {
    if model.starts_with("models/") {
        model.to_string()
    } else {
        format!("models/{}", model)
    }
}

pub async fn chat(
    http: &Client,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    _max_tokens: u32,
    _temperature: f32,
    system: &str,
    user: &str,
) -> Result<String> {
    let base = normalize_base_url(base_url);
    let model_path = normalize_model_path(model);
    let url = format!("{}/{}:generateContent", base, model_path);

    let request = GeminiGenerateContentRequest {
        system_instruction: if system.trim().is_empty() {
            None
        } else {
            Some(GeminiContent {
                parts: vec![GeminiPart { text: system.to_string() }],
            })
        },
        contents: vec![GeminiContent {
            parts: vec![GeminiPart { text: user.to_string() }],
        }],
    };

    let mut req_builder = http
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

    if let Some(key) = api_key {
        req_builder = req_builder.header("X-goog-api-key", key);
    }

    let response = req_builder
        .json(&request)
        .send()
        .await
        .context("Failed to send request")?;

    let status = response.status();
    let body = response.text().await.context("Failed to read response body")?;

    if !status.is_success() {
        if let Ok(err) = serde_json::from_str::<ApiError>(&body) {
            if let Some(detail) = err.error {
                if let Some(msg) = detail.message {
                    bail!("API error ({}): {}", status, msg);
                }
            }
        }
        bail!("API error ({}): {}", status, &body[..body.len().min(500)]);
    }

    let resp: GeminiGenerateContentResponse =
        serde_json::from_str(&body).context("Failed to parse Gemini response")?;

    let text = resp
        .candidates
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.as_ref())
        .and_then(|c| c.parts.first())
        .map(|p| p.text.trim().to_string());

    text.context("No response content from Gemini API")
}

pub async fn list_models(
    http: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>> {
    let base = normalize_base_url(base_url);
    let url = format!("{}/models", base);

    let mut req_builder = http.get(&url).header("Accept", "application/json");

    if let Some(key) = api_key {
        req_builder = req_builder.header("X-goog-api-key", key);
    }

    let response = req_builder.send().await.context("Failed to send request")?;

    let status = response.status();
    let body = response.text().await.context("Failed to read response body")?;

    if !status.is_success() {
        if let Ok(err) = serde_json::from_str::<ApiError>(&body) {
            if let Some(detail) = err.error {
                if let Some(msg) = detail.message {
                    bail!("API error ({}): {}", status, msg);
                }
            }
        }
        bail!("API error ({}): {}", status, &body[..body.len().min(500)]);
    }

    let resp: GeminiModelsResponse =
        serde_json::from_str(&body).context("Failed to parse Gemini models response")?;

    Ok(resp
        .models
        .into_iter()
        .map(|m| m.name.strip_prefix("models/").unwrap_or(&m.name).to_string())
        .collect())
}

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_base_url_adds_v1beta() {
        assert_eq!(
            normalize_base_url("https://generativelanguage.googleapis.com"),
            "https://generativelanguage.googleapis.com/v1beta"
        );
    }

    #[test]
    fn normalize_base_url_preserves_existing_v1beta() {
        assert_eq!(
            normalize_base_url("https://generativelanguage.googleapis.com/v1beta"),
            "https://generativelanguage.googleapis.com/v1beta"
        );
    }

    #[test]
    fn normalize_base_url_strips_trailing_slash() {
        assert_eq!(
            normalize_base_url("https://generativelanguage.googleapis.com/"),
            "https://generativelanguage.googleapis.com/v1beta"
        );
    }

    #[test]
    fn normalize_model_path_adds_prefix() {
        assert_eq!(normalize_model_path("gemini-2.5-flash"), "models/gemini-2.5-flash");
    }

    #[test]
    fn normalize_model_path_preserves_existing_prefix() {
        assert_eq!(normalize_model_path("models/gemini-2.5-flash"), "models/gemini-2.5-flash");
    }

    #[test]
    fn gemini_request_with_system_instruction() {
        let request = GeminiGenerateContentRequest {
            system_instruction: Some(GeminiContent {
                parts: vec![GeminiPart { text: "You are helpful.".to_string() }],
            }),
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: "Hello".to_string() }],
            }],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("system_instruction"));
        assert!(json.contains("You are helpful."));
    }

    #[test]
    fn gemini_request_without_system_instruction() {
        let request = GeminiGenerateContentRequest {
            system_instruction: None,
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: "Hello".to_string() }],
            }],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("system_instruction"));
    }

    #[test]
    fn gemini_request_empty_system_handled() {
        let system = "   ";
        let system_instruction = if system.trim().is_empty() {
            None
        } else {
            Some(GeminiContent {
                parts: vec![GeminiPart { text: system.to_string() }],
            })
        };
        assert!(system_instruction.is_none());
    }

    #[test]
    fn strip_models_prefix() {
        let name = "models/gemini-2.5-flash";
        let result = name.strip_prefix("models/").unwrap_or(name);
        assert_eq!(result, "gemini-2.5-flash");
    }

    #[test]
    fn strip_models_prefix_not_present() {
        let name = "gemini-2.5-flash";
        let result = name.strip_prefix("models/").unwrap_or(name);
        assert_eq!(result, "gemini-2.5-flash");
    }
}