// src/claude.rs
use anyhow::{bail, Context, Result};
use reqwest::Client;

use crate::types::*;

pub async fn chat(
    http: &Client,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    max_tokens: u32,
    temperature: f32,
    system: &str,
    user: &str,
) -> Result<String> {
    let url = format!("{}/messages", base_url);

    let request = ClaudeRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: user.to_string(),
        }],
        system: system.to_string(),
        max_tokens,
        temperature: Some(temperature),
    };

    let mut req_builder = http
        .post(&url)
        .header("Content-Type", "application/json")
        .header("anthropic-version", "2023-06-01");

    if let Some(key) = api_key {
        req_builder = req_builder.header("x-api-key", key);
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

    let resp: ClaudeResponse =
        serde_json::from_str(&body).context("Failed to parse Claude response")?;

    resp.content
        .first()
        .and_then(|c| c.text.as_ref())
        .map(|s| s.trim().to_string())
        .context("No response content from Claude API")
}

pub async fn list_models(
    http: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>> {
    let url = format!("{}/models", base_url);

    let mut req_builder = http
        .get(&url)
        .header("Accept", "application/json")
        .header("anthropic-version", "2023-06-01");

    if let Some(key) = api_key {
        req_builder = req_builder.header("x-api-key", key);
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

    let resp: ModelsResponse =
        serde_json::from_str(&body).context("Failed to parse models response")?;

    Ok(resp.data.into_iter().map(|m| m.id).collect())
}

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_request_builds_correctly() {
        let request = ClaudeRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            system: "You are helpful.".to_string(),
            max_tokens: 1024,
            temperature: Some(0.7),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"claude-sonnet-4-5-20250929\""));
        assert!(json.contains("\"system\":\"You are helpful.\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"temperature\":0.7"));
    }

    #[test]
    fn claude_request_user_message_only() {
        let request = ClaudeRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Test message".to_string(),
            }],
            system: "System prompt".to_string(),
            max_tokens: 500,
            temperature: Some(0.5),
        };

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "user");
    }

    #[test]
    fn claude_model_ids_valid_format() {
        let valid_models = [
            "claude-opus-4-5-20251101",
            "claude-sonnet-4-5-20250929",
            "claude-haiku-4-5-20251001",
            "claude-opus-4-1-20250805",
            "claude-sonnet-4-20250514",
            "claude-opus-4-20250514",
        ];
        for model in valid_models {
            assert!(model.starts_with("claude-"), "Model should start with 'claude-': {}", model);
            assert!(model.contains("-202"), "Model should contain date suffix: {}", model);
        }
    }
}