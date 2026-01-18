// src/providers/claude.rs
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use std::io::{self, Write};
use tokio::time::sleep;

use crate::providers::retry::{check_response_for_retry, RetryConfig, RetryDecision};
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
    stream: bool,
) -> Result<String> {
    let url = format!("{}/messages", base_url);
    let config = RetryConfig::default();

    let request = ClaudeRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: user.to_string(),
        }],
        system: system.to_string(),
        max_tokens,
        temperature: Some(temperature),
        stream: Some(stream),
    };

    for attempt in 0..=config.max_retries {
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

        // For streaming, we need to handle retry before consuming the body
        if stream {
            if !status.is_success() {
                let body = response.text().await.context("Failed to read error body")?;
                match check_response_for_retry(status, &body, attempt, &config)? {
                    RetryDecision::Success => unreachable!(),
                    RetryDecision::Retry { delay } => {
                        sleep(delay).await;
                        continue;
                    }
                }
            }

            let mut full_text = String::new();
            let mut s = response.bytes_stream();

            while let Some(item) = s.next().await {
                let chunk = item.context("Error while reading stream")?;
                let text = String::from_utf8_lossy(&chunk);

                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        let data = data.trim();
                        if data == "[DONE]" {
                            break;
                        }

                        if let Ok(delta) = serde_json::from_str::<ClaudeStreamDelta>(data) {
                            if let Some(t) = delta.delta.and_then(|d| d.text) {
                                print!("{}", t);
                                io::stdout().flush()?;
                                full_text.push_str(&t);
                            }
                        }
                    }
                }
            }

            println!();
            return Ok(full_text);
        }

        // Non-streaming path
        let body = response
            .text()
            .await
            .context("Failed to read response body")?;

        match check_response_for_retry(status, &body, attempt, &config)? {
            RetryDecision::Success => {}
            RetryDecision::Retry { delay } => {
                sleep(delay).await;
                continue;
            }
        }

        let resp: ClaudeResponse =
            serde_json::from_str(&body).context("Failed to parse Claude response")?;

        return resp
            .content
            .first()
            .and_then(|c| c.text.as_ref())
            .map(|s| s.trim().to_string())
            .context("No response content from Claude API");
    }

    bail!("All retry attempts exhausted for Claude API")
}

pub async fn list_models(
    http: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>> {
    let url = format!("{}/models", base_url);
    let config = RetryConfig::default();

    for attempt in 0..=config.max_retries {
        let mut req_builder = http
            .get(&url)
            .header("Accept", "application/json")
            .header("anthropic-version", "2023-06-01");

        if let Some(key) = api_key {
            req_builder = req_builder.header("x-api-key", key);
        }

        let response = req_builder.send().await.context("Failed to send request")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read response body")?;

        match check_response_for_retry(status, &body, attempt, &config)? {
            RetryDecision::Success => {}
            RetryDecision::Retry { delay } => {
                sleep(delay).await;
                continue;
            }
        }

        let resp: ModelsResponse =
            serde_json::from_str(&body).context("Failed to parse models response")?;

        return Ok(resp.data.into_iter().map(|m| m.id).collect());
    }

    bail!("All retry attempts exhausted for Claude API")
}

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn assert_f64_approx(actual: f64, expected: f64, eps: f64) {
        let diff = (actual - expected).abs();
        assert!(
            diff <= eps,
            "expected ~{}, got {} (diff {}, eps {})",
            expected,
            actual,
            diff,
            eps
        );
    }

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
            stream: Some(false),
        };

        let v: Value = serde_json::to_value(&request).unwrap();

        assert_eq!(v["model"], "claude-sonnet-4-5-20250929");
        assert_eq!(v["system"], "You are helpful.");
        assert_eq!(v["max_tokens"], 1024);
        assert_eq!(v["stream"], false);

        let temp = v["temperature"]
            .as_f64()
            .expect("temperature should be a number");
        assert_f64_approx(temp, 0.7, 1e-6);

        assert_eq!(v["messages"].as_array().unwrap().len(), 1);
        assert_eq!(v["messages"][0]["role"], "user");
        assert_eq!(v["messages"][0]["content"], "Hello");
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
            stream: Some(true),
        };

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "user");
        assert_eq!(request.messages[0].content, "Test message");
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
            assert!(
                model.starts_with("claude-"),
                "Model should start with 'claude-': {}",
                model
            );

            let last_dash = model.rfind('-').unwrap();
            let suffix = &model[last_dash + 1..];

            assert_eq!(
                suffix.len(),
                8,
                "Model date suffix should be 8 digits (YYYYMMDD): {}",
                model
            );
            assert!(
                suffix.chars().all(|c| c.is_ascii_digit()),
                "Model date suffix should be digits only: {}",
                model
            );
            assert!(
                suffix.starts_with("202"),
                "Model date suffix should look like 202x...: {}",
                model
            );
        }
    }
}