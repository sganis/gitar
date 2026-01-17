// src/types.rs
use serde::{Deserialize, Serialize};

// =============================================================================
// OPENAI API TYPES
// =============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
pub struct ChatMessageResponse {
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub id: String,
}

// =============================================================================
// COMMON ERROR TYPE
// =============================================================================
#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: Option<ApiErrorDetail>,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    pub message: Option<String>,
}

// =============================================================================
// CLAUDE API TYPES
// =============================================================================
#[derive(Debug, Serialize)]
pub struct ClaudeRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub system: String,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

// Streaming event chunks (delta-style)
#[derive(Debug, Deserialize)]
pub struct ClaudeStreamDelta {
    //pub r#type: String,
    pub delta: Option<ClaudeTextDelta>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeTextDelta {
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeResponse {
    pub content: Vec<ClaudeContent>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeContent {
    pub text: Option<String>,
}

// =============================================================================
// GEMINI API TYPES
// =============================================================================
#[derive(Debug, Serialize)]
pub struct GeminiGenerateContentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    pub contents: Vec<GeminiContent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiContent {
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiPart {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct GeminiGenerateContentResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiCandidate {
    pub content: Option<GeminiContent>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiModelsResponse {
    pub models: Vec<GeminiModelInfo>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiModelInfo {
    pub name: String,
}

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_serializes() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn chat_completion_request_serializes() {
        let req = ChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are helpful.".to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "Hi".to_string(),
                },
            ],
            max_tokens: Some(1024),
            max_completion_tokens: None,
            temperature: Some(0.7),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"gpt-4o\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(json.contains("\"role\":\"system\""));
        assert!(json.contains("\"role\":\"user\""));
        assert!(!json.contains("max_completion_tokens"));
    }

    #[test]
    fn chat_completion_response_deserializes() {
        let json = r#"{"choices": [{"message": {"content": "Hello! How can I help?"}}]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(
            resp.choices[0].message.content,
            Some("Hello! How can I help?".to_string())
        );
    }

    #[test]
    fn chat_completion_response_handles_null_content() {
        let json = r#"{"choices": [{"message": {"content": null}}]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert!(resp.choices[0].message.content.is_none());
    }

    #[test]
    fn models_response_deserializes() {
        let json =
            r#"{"data": [{"id": "gpt-4o"}, {"id": "gpt-5-chat-latest"}, {"id": "gpt-3.5-turbo"}]}"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 3);
        assert_eq!(resp.data[0].id, "gpt-4o");
        assert_eq!(resp.data[1].id, "gpt-5-chat-latest");
        assert_eq!(resp.data[2].id, "gpt-3.5-turbo");
    }

    #[test]
    fn models_response_handles_empty() {
        let json = r#"{"data": []}"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
    }

    #[test]
    fn api_error_deserializes() {
        let json = r#"{"error": {"message": "Invalid API key"}}"#;
        let err: ApiError = serde_json::from_str(json).unwrap();
        assert!(err.error.is_some());
        assert_eq!(
            err.error.unwrap().message,
            Some("Invalid API key".to_string())
        );
    }

    #[test]
    fn api_error_handles_missing_fields() {
        let json = r#"{"error": {}}"#;
        let err: ApiError = serde_json::from_str(json).unwrap();
        assert!(err.error.is_some());
        assert!(err.error.unwrap().message.is_none());
    }

    #[test]
    fn api_error_handles_null_error() {
        let json = r#"{"error": null}"#;
        let err: ApiError = serde_json::from_str(json).unwrap();
        assert!(err.error.is_none());
    }

    #[test]
    fn claude_request_serializes() {
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            system: "You are helpful.".to_string(),
            max_tokens: 1024,
            temperature: Some(0.7),
            stream: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"claude-sonnet-4-5-20250929\""));
        assert!(json.contains("\"system\":\"You are helpful.\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(json.contains("\"role\":\"user\""));
        assert!(!json.contains("\"stream\""));
    }

    #[test]
    fn claude_request_skips_none_temperature() {
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![],
            system: "test".to_string(),
            max_tokens: 500,
            temperature: None,
            stream: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("temperature"));
        assert!(!json.contains("\"stream\""));
    }

    #[test]
    fn claude_request_serializes_stream_true_when_set() {
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            system: "test".to_string(),
            max_tokens: 10,
            temperature: None,
            stream: Some(true),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"stream\":true"));
    }

    #[test]
    fn claude_response_deserializes() {
        let json = r#"{"content": [{"type": "text", "text": "Hello! How can I help?"}]}"#;
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content.len(), 1);
        assert_eq!(
            resp.content[0].text,
            Some("Hello! How can I help?".to_string())
        );
    }

    #[test]
    fn claude_response_handles_null_text() {
        let json = r#"{"content": [{"type": "text", "text": null}]}"#;
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert!(resp.content[0].text.is_none());
    }

    #[test]
    fn claude_response_handles_empty_content() {
        let json = r#"{"content": []}"#;
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert!(resp.content.is_empty());
    }

    #[test]
    fn claude_response_handles_multiple_content_blocks() {
        let json = r#"{"content": [{"text": "First part"}, {"text": "Second part"}]}"#;
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content.len(), 2);
        assert_eq!(resp.content[0].text, Some("First part".to_string()));
        assert_eq!(resp.content[1].text, Some("Second part".to_string()));
    }

    #[test]
    fn gemini_request_serializes() {
        let req = GeminiGenerateContentRequest {
            system_instruction: Some(GeminiContent {
                parts: vec![GeminiPart {
                    text: "You are helpful.".to_string(),
                }],
            }),
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: "Hello".to_string(),
                }],
            }],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"system_instruction\""));
        assert!(json.contains("You are helpful."));
        assert!(json.contains("\"contents\""));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn gemini_request_skips_none_system_instruction() {
        let req = GeminiGenerateContentRequest {
            system_instruction: None,
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: "Hello".to_string(),
                }],
            }],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("system_instruction"));
    }

    #[test]
    fn gemini_response_deserializes() {
        let json = r#"{"candidates": [{"content": {"parts": [{"text": "Hello! How can I help?"}]}}]}"#;
        let resp: GeminiGenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(resp.candidates.is_some());
        let candidates = resp.candidates.unwrap();
        assert_eq!(candidates.len(), 1);
        let text = candidates[0]
            .content
            .as_ref()
            .unwrap()
            .parts
            .first()
            .unwrap()
            .text
            .clone();
        assert_eq!(text, "Hello! How can I help?");
    }

    #[test]
    fn gemini_response_handles_null_candidates() {
        let json = r#"{"candidates": null}"#;
        let resp: GeminiGenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(resp.candidates.is_none());
    }

    #[test]
    fn gemini_response_handles_empty_candidates() {
        let json = r#"{"candidates": []}"#;
        let resp: GeminiGenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(resp.candidates.as_ref().unwrap().is_empty());
    }

    #[test]
    fn gemini_response_handles_null_content() {
        let json = r#"{"candidates": [{"content": null}]}"#;
        let resp: GeminiGenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(resp.candidates.unwrap()[0].content.is_none());
    }

    #[test]
    fn gemini_models_response_deserializes() {
        let json = r#"{"models": [{"name": "models/gemini-2.5-flash"}, {"name": "models/gemini-2.5-pro"}, {"name": "models/gemini-1.5-flash"}]}"#;
        let resp: GeminiModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.models.len(), 3);
        assert_eq!(resp.models[0].name, "models/gemini-2.5-flash");
        assert_eq!(resp.models[1].name, "models/gemini-2.5-pro");
    }

    #[test]
    fn gemini_models_response_handles_empty() {
        let json = r#"{"models": []}"#;
        let resp: GeminiModelsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.models.is_empty());
    }

    #[test]
    fn gemini_model_name_strip_prefix() {
        let name = "models/gemini-2.5-flash";
        let stripped = name.strip_prefix("models/").unwrap_or(name);
        assert_eq!(stripped, "gemini-2.5-flash");
    }

    #[test]
    fn gemini_model_name_no_prefix() {
        let name = "gemini-2.5-flash";
        let stripped = name.strip_prefix("models/").unwrap_or(name);
        assert_eq!(stripped, "gemini-2.5-flash");
    }
}
