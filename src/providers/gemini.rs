// src/gemini.rs
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::io::{self, Write};

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
    stream: bool,
) -> Result<String> {
    let base = normalize_base_url(base_url);
    let model_path = normalize_model_path(model);

    let url = if stream {
        format!("{}/{}:streamGenerateContent", base, model_path)
    } else {
        format!("{}/{}:generateContent", base, model_path)
    };

    let request = GeminiGenerateContentRequest {
        system_instruction: if system.trim().is_empty() {
            None
        } else {
            Some(GeminiContent {
                parts: vec![GeminiPart {
                    text: system.to_string(),
                }],
            })
        },
        contents: vec![GeminiContent {
            parts: vec![GeminiPart {
                text: user.to_string(),
            }],
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
    if !status.is_success() {
        let body = response.text().await.context("Failed to read error body")?;
        if let Ok(err) = serde_json::from_str::<ApiError>(&body) {
            if let Some(detail) = err.error {
                if let Some(msg) = detail.message {
                    bail!("API error ({}): {}", status, msg);
                }
            }
        }
        bail!("API error ({}): {}", status, &body[..body.len().min(500)]);
    }

    if stream {
        let mut full_text = String::new();
        let mut buf = String::new();
        let mut s = response.bytes_stream();

        while let Some(item) = s.next().await {
            let chunk = item.context("Error while reading stream")?;
            buf.push_str(&String::from_utf8_lossy(&chunk));

            let vals =
                drain_gemini_stream_values(&mut buf).context("Failed to parse Gemini stream")?;

            for v in vals {
                let t = extract_gemini_text_from_value(&v);
                if !t.is_empty() {
                    print!("{}", t);
                    io::stdout().flush()?;
                    full_text.push_str(&t);
                }
            }
        }

        // Best-effort final drain/parsing
        let leftover = buf.trim();
        if !leftover.is_empty() {
            // Sometimes the full stream arrives as a complete JSON array at the end.
            if let Ok(arr) = serde_json::from_str::<Vec<Value>>(leftover) {
                for v in arr {
                    let t = extract_gemini_text_from_value(&v);
                    if !t.is_empty() {
                        print!("{}", t);
                        io::stdout().flush()?;
                        full_text.push_str(&t);
                    }
                }
            } else if let Ok(one) = serde_json::from_str::<Value>(leftover) {
                let t = extract_gemini_text_from_value(&one);
                if !t.is_empty() {
                    print!("{}", t);
                    io::stdout().flush()?;
                    full_text.push_str(&t);
                }
            }
        }
        
        println!();
        if full_text.is_empty() {
            bail!("No response content from Gemini API (stream ended without content)");
        }
        return Ok(full_text);
    }

    // Non-streaming (keep strict struct parsing)
    let body = response
        .text()
        .await
        .context("Failed to read response body")?;

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

pub async fn list_models(http: &Client, base_url: &str, api_key: Option<&str>) -> Result<Vec<String>> {
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
// Streaming helpers (Value-based, tolerant to metadata chunks)
// =============================================================================

fn extract_gemini_text_from_value(v: &Value) -> String {
    // candidates[0].content.parts[*].text
    let mut out = String::new();

    let parts = v
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|c0| c0.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(|p| p.as_array());

    let Some(parts) = parts else {
        return out; // metadata-only chunk (role/finishReason/usageMetadata/etc)
    };

    for p in parts {
        if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
            out.push_str(t);
        }
    }

    out
}

/// Drain complete JSON Values from a buffer that contains streamed JSON arrays.
///
/// Handles:
/// - leading '['
/// - commas between objects
/// - ']' at end
/// - arbitrary chunk boundaries (buffered)
fn drain_gemini_stream_values(buf: &mut String) -> Result<Vec<Value>> {
    let mut out = Vec::new();

    loop {
        // Trim leading whitespace
        let trimmed = buf.trim_start();
        if trimmed.len() != buf.len() {
            buf.drain(..(buf.len() - trimmed.len()));
        }
        if buf.is_empty() {
            break;
        }

        // Drop leading array separators: '[', ',', ']'
        loop {
            let Some(first) = buf.chars().next() else { break };
            match first {
                '[' | ',' | ']' => {
                    buf.drain(..first.len_utf8());
                    // trim again
                    let t = buf.trim_start();
                    if t.len() != buf.len() {
                        buf.drain(..(buf.len() - t.len()));
                    }
                    if buf.is_empty() {
                        break;
                    }
                }
                _ => break,
            }
        }
        if buf.is_empty() {
            break;
        }

        let mut it = serde_json::Deserializer::from_str(buf).into_iter::<Value>();
        match it.next() {
            Some(Ok(v)) => {
                let offset = it.byte_offset();
                if offset == 0 {
                    break;
                }
                buf.drain(..offset);
                out.push(v);
            }
            Some(Err(e)) => {
                if e.is_eof() {
                    break; // wait for more bytes
                }
                let preview = buf.chars().take(200).collect::<String>();
                bail!(
                    "Gemini stream JSON parse error: {}. Buffer starts with: {}",
                    e,
                    preview
                );
            }
            None => break,
        }
    }

    Ok(out)
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
    fn extract_text_from_value_parts() {
        let v: Value = serde_json::json!({
          "candidates": [
            { "content": { "parts": [ {"text":"Hello "}, {"text":"World"} ] } }
          ]
        });
        assert_eq!(super::extract_gemini_text_from_value(&v), "Hello World");
    }

    #[test]
    fn extract_text_from_value_metadata_only_is_empty() {
        let v: Value = serde_json::json!({
          "candidates": [
            { "content": { "role": "model" }, "finishReason": "STOP" }
          ],
          "usageMetadata": { "promptTokenCount": 1 }
        });
        assert_eq!(super::extract_gemini_text_from_value(&v), "");
    }

    #[test]
    fn drain_values_parses_array_across_chunks_and_ignores_metadata() {
        let mut buf = String::new();
        buf.push_str("[");
        buf.push_str(r#"{"candidates":[{"content":{"parts":[{"text":"Hi"}]}}]},"#);
        // not complete second yet
        let v = super::drain_gemini_stream_values(&mut buf).unwrap();
        assert_eq!(v.len(), 1);

        buf.push_str(
            r#"{"candidates":[{"content":{"role":"model"},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":1}}]"#,
        );

        let v = super::drain_gemini_stream_values(&mut buf).unwrap();
        assert_eq!(v.len(), 1);
        assert!(buf.trim().is_empty());
    }
}
