// src/openai.rs
use anyhow::{bail, Context, Result};
use reqwest::Client;
use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

use crate::types::*;

pub static REASONING_MODELS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

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
    let url = format!("{}/chat/completions", base_url);

    let is_reasoning_model = REASONING_MODELS.lock().unwrap().contains(model);

    let messages = vec![
        ChatMessage { role: "system".to_string(), content: system.to_string() },
        ChatMessage { role: "user".to_string(), content: user.to_string() },
    ];

    let request = ChatCompletionRequest {
        model: model.to_string(),
        messages: messages.clone(),
        max_tokens: if is_reasoning_model { None } else { Some(max_tokens) },
        max_completion_tokens: if is_reasoning_model { Some(max_tokens) } else { None },
        temperature: if is_reasoning_model { None } else { Some(temperature) },
    };

    let response = send_chat_request(http, &url, api_key, &request).await;

    if let Err(e) = &response {
        let err_str = e.to_string();
        if (err_str.contains("max_completion_tokens") || err_str.contains("temperature"))
            && !is_reasoning_model
        {
            REASONING_MODELS.lock().unwrap().insert(model.to_string());

            let retry_request = ChatCompletionRequest {
                model: model.to_string(),
                messages,
                max_tokens: None,
                max_completion_tokens: Some(max_tokens),
                temperature: None,
            };

            return send_chat_request(http, &url, api_key, &retry_request).await;
        }
    }

    response
}

async fn send_chat_request(
    http: &Client,
    url: &str,
    api_key: Option<&str>,
    request: &ChatCompletionRequest,
) -> Result<String> {
    let mut req_builder = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

    if let Some(key) = api_key {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
    }

    let response = req_builder
        .json(request)
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

    let resp: ChatCompletionResponse =
        serde_json::from_str(&body).context("Failed to parse response")?;

    resp.choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .map(|s| s.trim().to_string())
        .context("No response content from API")
}

pub async fn list_models(
    http: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>> {
    let url = format!("{}/models", base_url);

    let mut req_builder = http.get(&url).header("Accept", "application/json");

    if let Some(key) = api_key {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
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
    fn reasoning_models_starts_empty() {
        // Clear any existing state
        REASONING_MODELS.lock().unwrap().clear();
        assert!(REASONING_MODELS.lock().unwrap().is_empty());
    }

    #[test]
    fn reasoning_models_can_insert_and_check() {
        REASONING_MODELS.lock().unwrap().clear();
        REASONING_MODELS.lock().unwrap().insert("o1-preview".to_string());
        assert!(REASONING_MODELS.lock().unwrap().contains("o1-preview"));
        assert!(!REASONING_MODELS.lock().unwrap().contains("gpt-4o"));
        REASONING_MODELS.lock().unwrap().clear();
    }

    #[test]
    fn chat_completion_request_for_normal_model() {
        let request = ChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![
                ChatMessage { role: "system".to_string(), content: "test".to_string() },
                ChatMessage { role: "user".to_string(), content: "hello".to_string() },
            ],
            max_tokens: Some(500),
            max_completion_tokens: None,
            temperature: Some(0.5),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"max_tokens\":500"));
        assert!(json.contains("\"temperature\":0.5"));
        assert!(!json.contains("max_completion_tokens"));
    }

    #[test]
    fn chat_completion_request_for_reasoning_model() {
        let request = ChatCompletionRequest {
            model: "o1-preview".to_string(),
            messages: vec![
                ChatMessage { role: "system".to_string(), content: "test".to_string() },
                ChatMessage { role: "user".to_string(), content: "hello".to_string() },
            ],
            max_tokens: None,
            max_completion_tokens: Some(500),
            temperature: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"max_completion_tokens\":500"));
        assert!(!json.contains("\"max_tokens\""));
        assert!(!json.contains("\"temperature\""));
    }
}