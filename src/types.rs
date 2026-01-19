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

#[derive(Debug, Deserialize)]
pub struct ClaudeStreamDelta {
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
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerateContentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    pub max_output_tokens: u32,
    pub temperature: f32,
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
    fn chat_completion_request_serializes_correctly() {
        let req = ChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![
                ChatMessage { role: "system".to_string(), content: "You are helpful.".to_string() },
                ChatMessage { role: "user".to_string(), content: "Hi".to_string() },
            ],
            max_tokens: Some(1024),
            max_completion_tokens: None,
            temperature: Some(0.7),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"gpt-4o\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(!json.contains("max_completion_tokens"));
    }

    #[test]
    fn chat_completion_response_deserializes() {
        let json = r#"{"choices": [{"message": {"content": "Hello! How can I help?"}}]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content, Some("Hello! How can I help?".to_string()));
    }

    #[test]
    fn models_response_deserializes() {
        let json = r#"{"data": [{"id": "gpt-4o"}, {"id": "gpt-5-chat-latest"}]}"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].id, "gpt-4o");
    }

    #[test]
    fn claude_request_serializes_correctly() {
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![ChatMessage { role: "user".to_string(), content: "Hello".to_string() }],
            system: "You are helpful.".to_string(),
            max_tokens: 1024,
            temperature: Some(0.7),
            stream: Some(true),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"claude-sonnet-4-5-20250929\""));
        assert!(json.contains("\"system\":\"You are helpful.\""));
        assert!(json.contains("\"stream\":true"));
    }

    #[test]
    fn claude_response_deserializes() {
        let json = r#"{"content": [{"type": "text", "text": "Hello!"}]}"#;
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.content[0].text, Some("Hello!".to_string()));
    }

    #[test]
    fn gemini_request_serializes_with_generation_config() {
        let req = GeminiGenerateContentRequest {
            system_instruction: Some(GeminiContent {
                parts: vec![GeminiPart { text: "You are helpful.".to_string() }],
            }),
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: "Hello".to_string() }],
            }],
            generation_config: Some(GeminiGenerationConfig {
                max_output_tokens: 500,
                temperature: 0.7,
            }),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("generationConfig"));
        assert!(json.contains("\"maxOutputTokens\":500"));
        assert!(json.contains("\"temperature\":0.7"));
    }

    #[test]
    fn gemini_request_omits_none_fields() {
        let req = GeminiGenerateContentRequest {
            system_instruction: None,
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: "Hello".to_string() }],
            }],
            generation_config: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("systemInstruction"));
        assert!(!json.contains("generationConfig"));
    }

    #[test]
    fn gemini_response_deserializes() {
        let json = r#"{"candidates": [{"content": {"parts": [{"text": "Hello!"}]}}]}"#;
        let resp: GeminiGenerateContentResponse = serde_json::from_str(json).unwrap();
        let text = resp.candidates.unwrap()[0].content.as_ref().unwrap().parts[0].text.clone();
        assert_eq!(text, "Hello!");
    }

    #[test]
    fn gemini_models_response_deserializes() {
        let json = r#"{"models": [{"name": "models/gemini-2.5-flash"}]}"#;
        let resp: GeminiModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.models[0].name, "models/gemini-2.5-flash");
    }
}