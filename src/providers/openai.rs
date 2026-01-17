// src/openai.rs
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use std::collections::HashSet;
use std::io::{self, Write};
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
    stream: bool,
) -> Result<String> {
    let url = format!("{}/chat/completions", base_url);

    let is_reasoning_model = REASONING_MODELS.lock().unwrap().contains(model);

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: user.to_string(),
        },
    ];

    if stream {
        let request_json =
            build_chat_request_json(model, &messages, is_reasoning_model, max_tokens, temperature, true);

        let resp = send_chat_request_stream(http, &url, api_key, request_json).await;

        if let Err(e) = &resp {
            let err_str = e.to_string();
            if (err_str.contains("max_completion_tokens") || err_str.contains("temperature"))
                && !is_reasoning_model
            {
                REASONING_MODELS.lock().unwrap().insert(model.to_string());

                let retry_json =
                    build_chat_request_json(model, &messages, true, max_tokens, temperature, true);
                return send_chat_request_stream(http, &url, api_key, retry_json).await;
            }
        }

        return resp;
    }

    // Non-streaming path (existing behavior)
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

async fn send_chat_request_stream(
    http: &Client,
    url: &str,
    api_key: Option<&str>,
    request_json: serde_json::Value,
) -> Result<String> {
    let mut req_builder = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");

    if let Some(key) = api_key {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
    }

    let response = req_builder
        .json(&request_json)
        .send()
        .await
        .context("Failed to send request")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.context("Failed to read error body")?;
        // Keep consistent error parsing behavior
        if let Ok(err) = serde_json::from_str::<ApiError>(&body) {
            if let Some(detail) = err.error {
                if let Some(msg) = detail.message {
                    bail!("API error ({}): {}", status, msg);
                }
            }
        }
        bail!("API error ({}): {}", status, &body[..body.len().min(500)]);
    }

    let mut full_text = String::new();
    let mut s = response.bytes_stream();

    while let Some(item) = s.next().await {
        let chunk = item.context("Error while reading stream")?;
        let text = String::from_utf8_lossy(&chunk);

        // OpenAI-compatible SSE usually sends "data: {...}" lines and "data: [DONE]"
        for line in text.lines() {
            let data = line
                .strip_prefix("data: ")
                .or_else(|| line.strip_prefix("data:"))
                .map(|x| x.trim());

            let Some(data) = data else { continue };

            if data == "[DONE]" {
                // End of stream
                println!();
                return Ok(full_text);
            }

            // Primary format: choices[].delta.content
            if let Ok(delta) = serde_json::from_str::<OpenAiStreamChunk>(data) {
                if let Some(t) = delta
                    .choices
                    .first()
                    .and_then(|c| c.delta.content.as_ref())
                {
                    print!("{}", t);
                    io::stdout().flush()?;
                    full_text.push_str(t);
                    continue;
                }
            }

            // Some providers may stream final content in `message.content` (rare). Best-effort:
            if let Ok(fallback) = serde_json::from_str::<ChatCompletionResponse>(data) {
                if let Some(t) = fallback
                    .choices
                    .first()
                    .and_then(|c| c.message.content.as_ref())
                {
                    let t = t.as_str();
                    print!("{}", t);
                    io::stdout().flush()?;
                    full_text.push_str(t);
                }
            }
        }
    }

    // If stream ends without [DONE], still return what we have.
    if full_text.is_empty() {
        bail!("No response content from API (stream ended without content)");
    }
    println!();
    Ok(full_text)
}

pub async fn list_models(http: &Client, base_url: &str, api_key: Option<&str>) -> Result<Vec<String>> {
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
// Helpers / stream types (local to this module)
// =============================================================================

pub(crate) fn build_chat_request_json(
    model: &str,
    messages: &[ChatMessage],
    is_reasoning_model: bool,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
) -> serde_json::Value {
    let mut v = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": stream
    });

    if is_reasoning_model {
        v["max_completion_tokens"] = serde_json::json!(max_tokens);
        // Some reasoning models reject temperature.
    } else {
        v["max_tokens"] = serde_json::json!(max_tokens);
        v["temperature"] = serde_json::json!(temperature);
    }

    v
}

#[derive(Debug, serde::Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, serde::Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiStreamDelta,
}

#[derive(Debug, serde::Deserialize)]
struct OpenAiStreamDelta {
    #[serde(default)]
    content: Option<String>,
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
    fn reasoning_models_starts_empty() {
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

    #[test]
    fn openai_request_json_stream_normal_model() {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: "sys".to_string() },
            ChatMessage { role: "user".to_string(), content: "hi".to_string() },
        ];

        let v = build_chat_request_json("gpt-4o", &messages, false, 123, 0.7, true);
        let vv: Value = serde_json::from_value(v).unwrap();

        assert_eq!(vv["model"], "gpt-4o");
        assert_eq!(vv["stream"], true);
        assert_eq!(vv["max_tokens"], 123);

        let temp = vv["temperature"].as_f64().expect("temperature should be number");
        assert_f64_approx(temp, 0.7, 1e-6);

        assert!(vv.get("max_completion_tokens").is_none());
    }

    #[test]
    fn openai_request_json_stream_reasoning_model() {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: "sys".to_string() },
            ChatMessage { role: "user".to_string(), content: "hi".to_string() },
        ];

        let v = build_chat_request_json("o1-preview", &messages, true, 999, 0.2, true);
        let vv: Value = serde_json::from_value(v).unwrap();

        assert_eq!(vv["model"], "o1-preview");
        assert_eq!(vv["stream"], true);
        assert_eq!(vv["max_completion_tokens"], 999);

        assert!(vv.get("max_tokens").is_none());
        assert!(vv.get("temperature").is_none());
    }
}
