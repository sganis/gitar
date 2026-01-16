// gitar - AI-powered Git assistant
// src/main.rs
mod tests;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use reqwest::{Client, Proxy};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::collections::HashSet;

static REASONING_MODELS: LazyLock<Mutex<HashSet<String>>> = 
    LazyLock::new(|| Mutex::new(HashSet::new()));

// =============================================================================
// CONFIG FILE
// =============================================================================

const CONFIG_FILENAME: &str = ".gitar.toml";

#[derive(Debug, Default, Serialize, Deserialize)]
struct Config {
    api_key: Option<String>,
    model: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    base_url: Option<String>,
    base_branch: Option<String>,
}

impl Config {
    fn path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(CONFIG_FILENAME))
    }

    fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(&p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) -> Result<()> {
        let path = Self::path().context("Could not determine home directory")?;
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(&path, content).context("Failed to write config file")?;
        println!("Config saved to: {}", path.display());
        Ok(())
    }
}

// =============================================================================
// OPENAI API TYPES
// =============================================================================
#[derive(Debug, Clone, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    error: Option<ApiErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    message: Option<String>,
}

// CLAUDE 
#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    messages: Vec<ChatMessage>,
    system: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContent {
    text: Option<String>,
}

// GEMINI
// === ADD: Gemini API types (place near other API TYPES) ===

#[derive(Debug, Serialize)]
struct GeminiGenerateContentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    contents: Vec<GeminiContent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateContentResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModelInfo>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelInfo {
    name: String, // e.g. "models/gemini-2.0-flash"
}

// =============================================================================
// PROMPTS
// =============================================================================

const HISTORY_SYSTEM_PROMPT: &str = r#"You are an expert software engineer who writes clear, informative Git commit messages.

## Commit Message Format
<Type>(<scope>):
<description line 1>
<description line 2 if needed>

## Types
- Feat: New feature
- Fix: Bug fix
- Refactor: Code restructuring without behavior change
- Docs: Documentation changes
- Style: Formatting, whitespace (no code logic change)
- Test: Adding or modifying tests
- Chore: Build process, dependencies, config
- Perf: Performance improvement

## Rules
1. First line: Type(scope): only, capitalized (no description on this line)
2. Following lines: describe WHAT changed and WHY
3. Scale detail to complexity: simple changes get 1-2 lines, complex changes get more
4. Use imperative mood ("Add" not "Added")
5. Be specific about impact and reasoning
6. Use plain ASCII characters only. Do not use emojis or Unicode symbols."#;

const HISTORY_USER_PROMPT: &str = r#"Generate a commit message for this diff.
First line: Type(scope): only (capitalized, nothing else on this line)
Following lines: describe what and why (1-5 lines depending on complexity)

**Original message (if any):** {original_message}

**Diff:**
```
{diff}
```
Respond with ONLY the commit message (no markdown, no extra explanation)."#;

const COMMIT_SYSTEM_PROMPT: &str = r#"You generate clear and informative Git commit messages from diffs.

Rules:
1. Focus on PURPOSE, not file listings
2. Ignore build/minified files
3. No markdown. Use plain ASCII characters only. Do not use emojis or Unicode symbols. Do not use empty lines between lines.
4. Be specific

Examples:
"Add user authentication with OAuth2 support"
"Fix payment timeout with retry logic"
"Refactor database queries for connection pooling"
"#;

const COMMIT_USER_PROMPT: &str = r#"Generate a commit message in a single-line.
```
{diff}
```
Respond with ONLY the commit message. (single-line)"#;

const PR_SYSTEM_PROMPT: &str = r#"Write a PR description.

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Format:
## Summary
Brief overview.

## What Changed
- Key changes

## Why
Motivation.

## Risks
- Issues or "None"

## Testing
- How tested

## Rollout
- Deploy notes or "Standard""#;

const PR_USER_PROMPT: &str = r#"Generate PR description.

**Branch:** {branch}
**Commits:**
{commits}

**Stats:**
{stats}

**Diff:**
```
{diff}
```
"#;

const CHANGELOG_SYSTEM_PROMPT: &str = r#"Create release notes.

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Format:
# Release Notes
## Features
## Fixes
## Improvements
## Breaking Changes
## Infrastructure

Group related changes, omit empty sections."#;

const CHANGELOG_USER_PROMPT: &str = r#"Generate release notes.

**Range:** {range}
**Count:** {count}

**Commits:**
{commits}"#;

const EXPLAIN_SYSTEM_PROMPT: &str = r#"Explain code changes to non-technical stakeholders.
No jargon, focus on user impact, be brief.

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Format:
## What's Changing
Summary.

## User Impact
- Effects

## Risk Level
Low/Medium/High

## Actions
- QA needed"#;

const EXPLAIN_USER_PROMPT: &str = r#"Explain for non-technical person.

**Stats:**
{stats}

**Diff:**
```
{diff}
```"#;

const VERSION_SYSTEM_PROMPT: &str = r#"Recommend semantic version bump.
- MAJOR: Breaking changes
- MINOR: New features
- PATCH: Fixes/refactors

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Output: Recommendation + Reasoning + Breaking: Yes/No"#;

const VERSION_USER_PROMPT: &str = r#"Recommend version bump.

**Current:** {version}
**Diff:**
```
{diff}
```"#;

// =============================================================================
// CLI
// =============================================================================

#[derive(Parser)]
#[command(
    name = "gitar",
    version,
    about = "AI-powered Git assistant\n\nAll commands accept an optional [REF] argument (tag, commit, branch) to specify the starting point.",
    after_help = "EXAMPLES:
    gitar changelog v1.0.0          # Release notes since tag
    gitar changelog HEAD~10         # Release notes for last 10 commits
    gitar changelog --since '1 week ago'
    
    gitar history v1.0.0            # Generate messages since tag
    gitar history -n 5              # Generate for last 5 commits
    
    gitar explain v1.0.0            # Explain changes since tag
    gitar explain --staged          # Explain staged changes
    
    gitar pr develop                # PR description against develop
    gitar pr --staged               # PR from staged changes
    
    gitar version v1.0.0            # Version bump since tag"
)]
struct Cli {
    /// API key (or set OPENAI_API_KEY / ANTHROPIC_API_KEY env var)
    #[arg(long, global = true)]  
    api_key: Option<String>,

    /// Model name [default: gpt-5-chat-latest]
    #[arg(long, global = true)]
    model: Option<String>,

    /// Maximum tokens [default: 500]
    #[arg(long, global = true)]
    max_tokens: Option<u32>,

    /// Temperature (0.0-2.0) [default: 0.5]
    #[arg(long, global = true)]
    temperature: Option<f32>,

    /// API base URL
    #[arg(long, env = "OPENAI_BASE_URL", global = true)]
    base_url: Option<String>,

    /// Default base branch
    #[arg(long, global = true)]
    base_branch: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive commit with AI message
    Commit {
        #[arg(short = 'p', long)]
        push: bool,
        #[arg(short = 'a', long)]
        all: bool,
        #[arg(long, default_value = "true")]
        tag: bool,
        #[arg(long = "no-tag")]
        no_tag: bool,
    },

    /// Generate commit message for staged changes
    Staged,

    /// Generate commit message for unstaged changes
    Unstaged,

    /// Generate messages for commit history
    History {
        /// Starting point (tag, commit, branch)
        #[arg(value_name = "REF")]
        from: Option<String>,
        /// Ending point (default: HEAD)
        #[arg(long)]
        to: Option<String>,
        /// Commits newer than date
        #[arg(long)]
        since: Option<String>,
        /// Commits older than date
        #[arg(long)]
        until: Option<String>,
        /// Max commits (default: 50 if no REF)
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        /// Delay between API calls (ms)
        #[arg(long, default_value = "500")]
        delay: u64,
    },

    /// Generate PR description
    Pr {
        /// Base ref to compare against (tag, commit, branch)
        #[arg(value_name = "REF")]
        base: Option<String>,
        /// Ending point (default: current branch)
        #[arg(long)]
        to: Option<String>,
        /// Use staged changes only
        #[arg(long)]
        staged: bool,
    },

    /// Generate changelog / release notes
    Changelog {
        /// Starting point (tag, commit, branch)
        #[arg(value_name = "REF")]
        from: Option<String>,
        /// Ending point (default: HEAD)
        #[arg(long)]
        to: Option<String>,
        /// Commits newer than date
        #[arg(long)]
        since: Option<String>,
        /// Commits older than date
        #[arg(long)]
        until: Option<String>,
        /// Max commits (default: 50 if no REF)
        #[arg(short = 'n', long)]
        limit: Option<usize>,
    },

    /// Explain changes for non-technical stakeholders
    Explain {
        /// Starting point (tag, commit, branch)
        #[arg(value_name = "REF")]
        from: Option<String>,
        /// Ending point (default: HEAD)
        #[arg(long)]
        to: Option<String>,
        /// Changes newer than date
        #[arg(long)]
        since: Option<String>,
        /// Changes older than date
        #[arg(long)]
        until: Option<String>,
        /// Use staged changes only
        #[arg(long)]
        staged: bool,
    },

    /// Suggest semantic version bump
    Version {
        /// Base ref to compare against (tag, commit, branch)
        #[arg(value_name = "REF")]
        base: Option<String>,
        /// Ending point (default: HEAD)
        #[arg(long)]
        to: Option<String>,
        /// Current version (default: from tags)
        #[arg(long)]
        current: Option<String>,
    },

    /// Save options to config file (~/.gitar.toml)
    Init {
        #[arg(long)]
        api_key: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        max_tokens: Option<u32>,
        #[arg(long)]
        temperature: Option<f32>,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long)]
        base_branch: Option<String>,
    },

    /// Show current config
    Config,

    /// List available models
    Models,
}

struct ResolvedConfig {
    api_key: Option<String>,
    model: String,
    max_tokens: u32,
    temperature: f32,
    base_url: String,
    base_branch: String,
}

impl ResolvedConfig {
    fn new(cli: &Cli, file: &Config) -> Self {
        let base_url = cli.base_url.clone()
            .or_else(|| file.base_url.clone())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        let is_claude = base_url.contains("anthropic.com");
        let is_groq = base_url.contains("api.groq.com");
        let is_gemini = base_url.contains("generativelanguage.googleapis.com");

        let default_model = if is_claude {
            "claude-sonnet-4-5-20250929"
        } else if is_gemini {
            "gemini-2.0-flash"
        } else {
            "gpt-5-chat-latest"
        };

        let api_key = cli.api_key.clone()
            .or_else(|| file.api_key.clone())
            .or_else(|| {
                if is_claude {
                    std::env::var("ANTHROPIC_API_KEY").ok()
                } else if is_groq {
                    std::env::var("GROQ_API_KEY").ok()
                        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                } else if is_gemini {
                    std::env::var("GEMINI_API_KEY").ok()
                        .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
                } else {
                    std::env::var("OPENAI_API_KEY").ok()
                }
            });

        Self {
            api_key,
            model: cli.model.clone()
                .or_else(|| file.model.clone())
                .unwrap_or_else(|| default_model.to_string()),
            max_tokens: cli.max_tokens.or(file.max_tokens).unwrap_or(500),
            temperature: cli.temperature.or(file.temperature).unwrap_or(0.5),
            base_url,
            base_branch: cli.base_branch.clone()
                .or_else(|| file.base_branch.clone())
                .unwrap_or_else(get_default_branch),
        }
    }
}


// =============================================================================
// LLM CLIENT
// =============================================================================

struct LlmClient {
    http: Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
    max_tokens: u32,
    temperature: f32,
}

impl LlmClient {
    fn new(config: &ResolvedConfig) -> Result<Self> {
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
    
    fn is_claude_api(&self) -> bool {
        self.base_url.contains("anthropic.com")
    }
    
    fn is_gemini_api(&self) -> bool {
        self.base_url.contains("generativelanguage.googleapis.com")
    }

    
    async fn chat(&self, system: &str, user: &str) -> Result<String> {
        if self.is_claude_api() {
            return self.chat_claude(system, user).await;
        }

        if self.is_gemini_api() {
            return self.chat_gemini(system, user).await;
        }

        let url = format!("{}/chat/completions", self.base_url);

        let is_reasoning_model = REASONING_MODELS
            .lock()
            .unwrap()
            .contains(&self.model);

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

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: messages.clone(),
            max_tokens: if is_reasoning_model { None } else { Some(self.max_tokens) },
            max_completion_tokens: if is_reasoning_model { Some(self.max_tokens) } else { None },
            temperature: if is_reasoning_model { None } else { Some(self.temperature) },
        };

        let response = self.send_chat_request(&url, &request).await;

        // Check if we need to retry as a reasoning model
        if let Err(e) = &response {
            let err_str = e.to_string();
            if (err_str.contains("max_completion_tokens") || err_str.contains("temperature")) 
                && !is_reasoning_model 
            {
                REASONING_MODELS
                    .lock()
                    .unwrap()
                    .insert(self.model.clone());

                let retry_request = ChatCompletionRequest {
                    model: self.model.clone(),
                    messages,
                    max_tokens: None,
                    max_completion_tokens: Some(self.max_tokens),
                    temperature: None,
                };

                return self.send_chat_request(&url, &retry_request).await;
            }
        }

        response
    }

    async fn chat_claude(&self, system: &str, user: &str) -> Result<String> {
        let url = format!("{}/messages", self.base_url);

        let request = ClaudeRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: user.to_string(),
            }],
            system: system.to_string(),
            max_tokens: self.max_tokens,
            temperature: Some(self.temperature),
        };

        let mut req_builder = self.http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01");

        if let Some(ref api_key) = self.api_key {
            req_builder = req_builder.header("x-api-key", api_key);
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

        let resp: ClaudeResponse = serde_json::from_str(&body)
            .context("Failed to parse Claude response")?;

        resp.content
            .first()
            .and_then(|c| c.text.as_ref())
            .map(|s| s.trim().to_string())
            .context("No response content from Claude API")
    }

    async fn chat_gemini(&self, system: &str, user: &str) -> Result<String> {
        let base = self.base_url.trim_end_matches('/');

        let base = if base.ends_with("/v1beta") {
            base.to_string()
        } else {
            format!("{}/v1beta", base)
        };

        let model_path = if self.model.starts_with("models/") {
            self.model.clone()
        } else {
            format!("models/{}", self.model)
        };

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

        let mut req_builder = self.http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");

        if let Some(ref api_key) = self.api_key {
            req_builder = req_builder.header("X-goog-api-key", api_key);
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

        let resp: GeminiGenerateContentResponse = serde_json::from_str(&body)
            .context("Failed to parse Gemini response")?;

        let text = resp.candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .and_then(|c| c.parts.first())
            .map(|p| p.text.trim().to_string());

        text.context("No response content from Gemini API")
    }

    async fn send_chat_request(&self, url: &str, request: &ChatCompletionRequest) -> Result<String> {
        let mut req_builder = self.http
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");

        if let Some(ref api_key) = self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
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

        let resp: ChatCompletionResponse = serde_json::from_str(&body)
            .context("Failed to parse response")?;

        resp.choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .map(|s| s.trim().to_string())
            .context("No response content from API")
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        if self.is_gemini_api() {
            return self.list_models_gemini().await;
        }

        let url = format!("{}/models", self.base_url);

        let mut req_builder = self.http
            .get(&url)
            .header("Accept", "application/json");

        if let Some(ref api_key) = self.api_key {
            if self.is_claude_api() {
                req_builder = req_builder
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01");
            } else {
                req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
            }
        }

        let response = req_builder
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

        let resp: ModelsResponse = serde_json::from_str(&body)
            .context("Failed to parse models response")?;

        Ok(resp.data.into_iter().map(|m| m.id).collect())
    }

    async fn list_models_gemini(&self) -> Result<Vec<String>> {
        let base = self.base_url.trim_end_matches('/');

        let base = if base.ends_with("/v1beta") {
            base.to_string()
        } else {
            format!("{}/v1beta", base)
        };

        let url = format!("{}/models", base);

        let mut req_builder = self.http
            .get(&url)
            .header("Accept", "application/json");

        if let Some(ref api_key) = self.api_key {
            req_builder = req_builder.header("X-goog-api-key", api_key);
        }

        let response = req_builder
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

        let resp: GeminiModelsResponse = serde_json::from_str(&body)
            .context("Failed to parse Gemini models response")?;

        Ok(resp.models.into_iter().map(|m| {
            // convert "models/gemini-2.0-flash" -> "gemini-2.0-flash"
            m.name.strip_prefix("models/").unwrap_or(&m.name).to_string()
        }).collect())
    }
}

// =============================================================================
// GIT UTILITIES
// =============================================================================

const EXCLUDE_PATTERNS: &[&str] = &[
    ":(exclude)*.lock",
    ":(exclude)package-lock.json",
    ":(exclude)yarn.lock",
    ":(exclude)pnpm-lock.yaml",
    ":(exclude)dist/*",
    ":(exclude)build/*",
    ":(exclude)*.min.js",
    ":(exclude)*.min.css",
    ":(exclude)*.map",
    ":(exclude).env*",
    ":(exclude)target/*",
];

fn run_git(args: &[&str]) -> Result<String> {
    let output = Command::new("git").args(args).output().context("Failed to execute git")?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_status(args: &[&str]) -> (String, String, bool) {
    match Command::new("git").args(args).output() {
        Ok(o) => (
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
            o.status.success(),
        ),
        Err(e) => (String::new(), e.to_string(), false),
    }
}

fn is_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn get_current_branch() -> String {
    // 1) Fast path: works on normal checkouts
    if let Ok(out) = run_git(&["branch", "--show-current"]) {
        let b = out.trim().to_string();
        if !b.is_empty() {
            return b;
        }
    }

    // 2) Fallback: works even when detached (returns "HEAD" in that case)
    if let Ok(out) = run_git(&["rev-parse", "--abbrev-ref", "HEAD"]) {
        let b = out.trim().to_string();
        if !b.is_empty() {
            return b;
        }
    }

    // 3) Last resort: never return empty
    "HEAD".to_string()
}


fn get_default_branch() -> String {
    for b in ["main", "master"] {
        if run_git(&["rev-parse", "--verify", b]).is_ok() {
            return b.into();
        }
    }
    "main".into()
}

#[derive(Debug)]
struct CommitInfo {
    hash: String,
    author: String,
    date: String,
    message: String,
}

fn get_commit_logs(
    limit: Option<usize>,
    since: Option<&str>,
    until: Option<&str>,
    range: Option<&str>,
) -> Result<Vec<CommitInfo>> {
    let mut args_vec: Vec<String> = vec![
        "log".into(),
        "--pretty=format:%H|%an|%ad|%s".into(),
        "--date=iso".into(),
    ];

    if let Some(n) = limit {
        args_vec.push(format!("-n{}", n));
    }
    if let Some(s) = since {
        args_vec.push(format!("--since={}", s));
    }
    if let Some(u) = until {
        args_vec.push(format!("--until={}", u));
    }
    if let Some(r) = range {
        args_vec.push(r.to_string());
    }

    let args: Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();
    let output = run_git(&args)?;

    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let p: Vec<&str> = l.splitn(4, '|').collect();
            if p.len() >= 4 {
                Some(CommitInfo {
                    hash: p[0].into(),
                    author: p[1].into(),
                    date: p[2].into(),
                    message: p[3].into(),
                })
            } else {
                None
            }
        })
        .collect())
}

fn get_commit_diff(hash: &str, max_chars: usize) -> Result<Option<String>> {
    let parent_ref = format!("{}^", hash);
    let has_parent = run_git(&["rev-parse", &parent_ref]).is_ok();

    let diff = if has_parent {
        let diff_ref = format!("{}^!", hash);
        let mut args = vec!["diff", &diff_ref, "--unified=3", "--", "."];
        args.extend(EXCLUDE_PATTERNS);
        run_git(&args)?
    } else {
        let mut args = vec!["diff-tree", "--patch", "--unified=3", "--root", hash, "--", "."];
        args.extend(EXCLUDE_PATTERNS);
        run_git(&args)?
    };

    if diff.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(truncate_diff(diff, max_chars)))
}

fn get_diff(target: Option<&str>, staged: bool, max_chars: usize) -> Result<String> {
    let mut args = vec!["diff", "--unified=3"];
    if staged {
        args.push("--cached");
    } else if let Some(t) = target {
        args.push(t);
    }
    args.extend(&["--", "."]);
    args.extend(EXCLUDE_PATTERNS);
    Ok(truncate_diff(run_git(&args)?, max_chars))
}

fn get_diff_stats(target: Option<&str>, staged: bool) -> Result<String> {
    let mut args = vec!["diff", "--stat"];
    if staged {
        args.push("--cached");
    } else if let Some(t) = target {
        args.push(t);
    }
    run_git(&args)
}

fn get_current_version() -> String {
    run_git(&["describe", "--tags", "--abbrev=0"])
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "0.0.0".into())
}

fn truncate_diff(diff: String, max: usize) -> String {
    if diff.len() <= max {
        return diff;
    }
    let mut t = diff[..max].to_string();
    if let Some(p) = t.rfind("\ndiff --git") {
        if p > max / 2 {
            t.truncate(p);
        }
    }
    t.push_str("\n\n[... truncated ...]");
    t
}

fn build_range(from: Option<&str>, to: Option<&str>, base_branch: &str) -> Option<String> {
    let end = to.unwrap_or("HEAD");
    from.map(|r| format!("{}..{}", r, end))
        .or_else(|| {
            let branch = get_current_branch();
            if branch != base_branch {
                Some(format!("{}..{}", base_branch, if to.is_some() { end } else { &branch }))
            } else {
                None
            }
        })
}

fn build_diff_target(from: Option<&str>, to: Option<&str>, base_branch: &str) -> String {
    let end = to.unwrap_or("HEAD");
    match from {
        Some(r) => format!("{}..{}", r, end),
        None => {
            let branch = get_current_branch();
            if branch != base_branch {
                format!("{}...{}", base_branch, if to.is_some() { end } else { &branch })
            } else {
                let tag = get_current_version();
                if tag != "0.0.0" {
                    format!("{}..{}", tag, end)
                } else {
                    String::new()
                }
            }
        }
    }
}

// =============================================================================
// COMMANDS
// =============================================================================

async fn cmd_commit(client: &LlmClient, push: bool, all: bool, tag: bool) -> Result<()> {
    let staged = run_git(&["diff", "--cached"]).unwrap_or_default();
    let unstaged = run_git(&["diff"]).unwrap_or_default();

    let mut diff = String::new();
    if !staged.trim().is_empty() {
        diff.push_str(&staged);
    }
    if !unstaged.trim().is_empty() {
        if !diff.is_empty() {
            diff.push('\n');
        }
        diff.push_str(&unstaged);
    }

    if diff.trim().is_empty() {
        println!("Nothing to commit.");
        return Ok(());
    }

    let diff = truncate_diff(diff, 100000);

    let commit_message = loop {
        let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
        let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt).await?;

        println!("\n{}\n", msg);
        println!("{}", "=".repeat(50));
        println!("  [Enter] Accept | [g] Regenerate | [e] Edit | [other] Cancel");
        println!("{}", "=".repeat(50));
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "" => break msg,
            "g" => {
                println!("Regenerating...\n");
                continue;
            }
            "e" => {
                print!("New message: ");
                io::stdout().flush()?;
                let mut ed = String::new();
                io::stdin().read_line(&mut ed)?;
                break if ed.trim().is_empty() { msg } else { ed.trim().into() };
            }
            _ => {
                println!("Canceled.");
                return Ok(());
            }
        }
    };

    if all {
        println!("Staging all...");
        run_git(&["add", "-A"])?;
    }

    println!("Committing...");
    let full_msg = if tag {
        format!("{} [AI:{}]", commit_message, client.model)
    } else {
        commit_message
    };

    let (out, err, ok) = if all {
        run_git_status(&["commit", "-am", &full_msg])
    } else {
        run_git_status(&["commit", "-m", &full_msg])
    };
    println!("{}{}", out, err);

    if !ok {
        println!("Commit failed.");
        return Ok(());
    }

    if push {
        println!("Pushing...");
        let (o, e, _) = run_git_status(&["push"]);
        println!("{}{}", o, e);
    }

    Ok(())
}

async fn cmd_staged(client: &LlmClient) -> Result<()> {
    let diff = get_diff(None, true, 100000)?;
    if diff.trim().is_empty() {
        bail!("No staged changes.");
    }
    let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
    let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", msg);
    Ok(())
}

async fn cmd_unstaged(client: &LlmClient) -> Result<()> {
    let diff = get_diff(None, false, 100000)?;
    if diff.trim().is_empty() {
        bail!("No unstaged changes.");
    }
    let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
    let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", msg);
    Ok(())
}

async fn cmd_history(
    client: &LlmClient,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    limit: Option<usize>,
    delay: u64,
) -> Result<()> {
    let limit = match (&from, limit) {
        (Some(_), None) => None,
        (None, None) => Some(50),
        (_, Some(n)) => Some(n),
    };

    let end = to.as_deref().unwrap_or("HEAD");
    let range = from.as_ref().map(|r| format!("{}..{}", r, end));

    let display = match (&from, &to, &since, &until) {
        (Some(r), Some(t), _, _) => format!("{}..{}", r, t),
        (Some(r), None, _, _) => format!("{}..HEAD", r),
        (None, None, Some(s), _) => format!("--since {}", s),
        _ => "recent".into(),
    };

    println!("Fetching commits ({})...", display);

    let commits = get_commit_logs(limit, since.as_deref(), until.as_deref(), range.as_deref())?;

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    println!("Processing {} commits...\n", commits.len());

    for (i, c) in commits.iter().enumerate() {
        let h = &c.hash[..8.min(c.hash.len())];
        let d = &c.date[..10.min(c.date.len())];
        let a = if c.author.len() > 15 { &c.author[..15] } else { &c.author };
        let m = if c.message.len() > 40 { &c.message[..40] } else { &c.message };

        println!("[{}/{}] {} | {} | {:15} | {}", i + 1, commits.len(), h, d, a, m);

        let diff = match get_commit_diff(&c.hash, 12000)? {
            Some(d) if !d.trim().is_empty() => d,
            _ => {
                println!("  - No diff");
                continue;
            }
        };

        let prompt = HISTORY_USER_PROMPT
            .replace("{original_message}", &c.message)
            .replace("{diff}", &diff);

        match client.chat(HISTORY_SYSTEM_PROMPT, &prompt).await {
            Ok(r) => {
                for (j, l) in r.lines().enumerate() {
                    if !l.trim().is_empty() {
                        println!("{}{}", if j == 0 { "  - " } else { "    " }, l);
                    }
                }
            }
            Err(e) => println!("  x {}", e),
        }

        if i < commits.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
    }

    Ok(())
}

async fn cmd_pr(
    client: &LlmClient,
    base: Option<String>,
    to: Option<String>,
    base_branch: &str,
    staged: bool,
) -> Result<()> {
    let branch = to.clone().unwrap_or_else(get_current_branch);
    let target_base = base.as_deref().unwrap_or(base_branch);

    println!("PR: {} -> {}\n", branch, target_base);

    let (diff, stats, commits_text) = if staged {
        (
            get_diff(None, true, 15000)?,
            get_diff_stats(None, true)?,
            "(staged changes)".into(),
        )
    } else {
        let diff_target = build_diff_target(base.as_deref(), to.as_deref(), base_branch);
        let range = build_range(base.as_deref(), to.as_deref(), base_branch);

        let commits = get_commit_logs(Some(20), None, None, range.as_deref())?;
        let ct = commits
            .iter()
            .map(|c| format!("- {}", c.message))
            .collect::<Vec<_>>()
            .join("\n");

        let diff_target_ref = if diff_target.is_empty() { None } else { Some(diff_target.as_str()) };

        (
            get_diff(diff_target_ref, false, 15000)?,
            get_diff_stats(diff_target_ref, false)?,
            if ct.is_empty() { "(no commits)".into() } else { ct },
        )
    };

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = PR_USER_PROMPT
        .replace("{branch}", &branch)
        .replace("{commits}", &commits_text)
        .replace("{stats}", &stats)
        .replace("{diff}", &diff);

    let r = client.chat(PR_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", r);

    Ok(())
}

// Update cmd_changelog function signature and implementation
async fn cmd_changelog(
    client: &LlmClient,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let limit = match (&from, limit) {
        (Some(_), None) => None,
        (None, None) => Some(50),
        (_, Some(n)) => Some(n),
    };

    let end = to.as_deref().unwrap_or("HEAD");
    let range = from.as_ref().map(|r| format!("{}..{}", r, end));

    // Build display string
    let display = match (&from, &to, &since, &until) {
        (Some(r), Some(t), _, _) => format!("{}..{}", r, t),
        (Some(r), None, _, _) => format!("{}..HEAD", r),
        (None, Some(t), _, _) => format!("..{}", t),
        (None, None, Some(s), Some(u)) => format!("--since {} --until {}", s, u),
        (None, None, Some(s), None) => format!("--since {}", s),
        (None, None, None, Some(u)) => format!("--until {}", u),
        (None, None, None, None) => "recent (last 50)".into(),
    };

    println!("Changelog for {}...\n", display);

    let commits = get_commit_logs(limit, since.as_deref(), until.as_deref(), range.as_deref())?;

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    println!("Found {} commits.\n", commits.len());

    let ct = commits
        .iter()
        .map(|c| format!("- [{}] {}", &c.hash[..8.min(c.hash.len())], c.message))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = CHANGELOG_USER_PROMPT
        .replace("{range}", &display)
        .replace("{count}", &commits.len().to_string())
        .replace("{commits}", &ct);

    let r = client.chat(CHANGELOG_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", r);

    Ok(())
}

async fn cmd_explain(
    client: &LlmClient,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    base_branch: &str,
    staged: bool,
) -> Result<()> {
    println!("Explaining changes...\n");

    let (diff, stats) = if staged {
        (get_diff(None, true, 15000)?, get_diff_stats(None, true)?)
    } else {
        let effective_from = match (&from, &since, &until) {
            (Some(_), _, _) => from.clone(),
            (None, Some(_), _) | (None, None, Some(_)) => {
                let commits = get_commit_logs(None, since.as_deref(), until.as_deref(), None)?;
                commits.last().map(|c| c.hash.clone())
            }
            _ => None,
        };

        let diff_target = build_diff_target(effective_from.as_deref(), to.as_deref(), base_branch);
        let diff_target_ref = if diff_target.is_empty() { None } else { Some(diff_target.as_str()) };
        (
            get_diff(diff_target_ref, false, 15000)?,
            get_diff_stats(diff_target_ref, false)?,
        )
    };

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = EXPLAIN_USER_PROMPT
        .replace("{stats}", &stats)
        .replace("{diff}", &diff);

    let r = client.chat(EXPLAIN_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", r);

    Ok(())
}

async fn cmd_version(
    client: &LlmClient,
    base: Option<String>,
    to: Option<String>,
    base_branch: &str,
    current: Option<String>,
) -> Result<()> {
    let current = current.unwrap_or_else(get_current_version);
    println!("Version analysis (current: {})...\n", current);

    let diff_target = build_diff_target(base.as_deref(), to.as_deref(), base_branch);
    let diff_target_ref = if diff_target.is_empty() { None } else { Some(diff_target.as_str()) };

    let diff = get_diff(diff_target_ref, false, 15000)?;

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = VERSION_USER_PROMPT
        .replace("{version}", &current)
        .replace("{diff}", &diff);

    let r = client.chat(VERSION_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", r);

    Ok(())
}

fn cmd_init(
    api_key: Option<String>,
    model: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    base_url: Option<String>,
    base_branch: Option<String>,
) -> Result<()> {
    let mut config = Config::load();

    if api_key.is_some() { config.api_key = api_key; }
    if model.is_some() { config.model = model; }
    if max_tokens.is_some() { config.max_tokens = max_tokens; }
    if temperature.is_some() { config.temperature = temperature; }
    if base_url.is_some() { config.base_url = base_url; }
    if base_branch.is_some() { config.base_branch = base_branch; }

    config.save()
}

fn cmd_config() -> Result<()> {
    let config = Config::load();
    let path = Config::path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(unknown)".into());

    let base_url = config.base_url.as_deref().unwrap_or("https://api.openai.com/v1");
    let is_claude = base_url.contains("anthropic.com");
    let is_groq = base_url.contains("api.groq.com");
    let is_gemini = base_url.contains("generativelanguage.googleapis.com");

    println!("Config file: {}\n", path);
    println!("api_key:     {}", config.api_key.as_deref()
        .map(|k| format!("{}...", &k[..8.min(k.len())]))
        .unwrap_or_else(|| "(not set)".into()));
    println!("model:       {}", config.model.as_deref().unwrap_or("(not set)"));
    println!("max_tokens:  {}", config.max_tokens
        .map(|t| t.to_string())
        .unwrap_or_else(|| "(not set)".into()));
    println!("temperature: {}", config.temperature
        .map(|t| t.to_string())
        .unwrap_or_else(|| "(not set)".into()));
    println!("base_url:    {}", config.base_url.as_deref().unwrap_or("(not set)"));
    println!("base_branch: {}", config.base_branch.as_deref().unwrap_or("(not set)"));

    println!("\nPriority: --api-key > config file > env var");
    println!(
        "Env vars checked: {}",
        if is_claude {
            "ANTHROPIC_API_KEY"
        } else if is_groq {
            "GROQ_API_KEY (fallback: OPENAI_API_KEY)"
        } else if is_gemini {
            "GEMINI_API_KEY (fallback: GOOGLE_API_KEY)"
        } else {
            "OPENAI_API_KEY"
        }
    );

    Ok(())
}

async fn cmd_models(client: &LlmClient) -> Result<()> {
    println!("Fetching available models...\n");

    let models = client.list_models().await?;

    if models.is_empty() {
        println!("No models found.");
    } else {
        println!("Available models:");
        for model in models {
            println!("  {}", model);
        }
    }

    Ok(())
}

// =============================================================================
// MAIN
// =============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();
    let file_config = Config::load();

    // Handle non-git commands first
    match &cli.command {
        Commands::Init { api_key, model, max_tokens, temperature, base_url, base_branch } => {
            return cmd_init(
                api_key.clone().or_else(|| cli.api_key.clone()),
                model.clone().or_else(|| cli.model.clone()),
                max_tokens.or(cli.max_tokens),
                temperature.or(cli.temperature),
                base_url.clone().or_else(|| cli.base_url.clone()),
                base_branch.clone().or_else(|| cli.base_branch.clone()),
            );
        }
        Commands::Config => return cmd_config(),
        _ => {}
    }

    if !is_git_repo() {
        bail!("Not a git repository");
    }

    let config = ResolvedConfig::new(&cli, &file_config);
    let client = LlmClient::new(&config)?;

    match cli.command {
        Commands::Commit { push, all, tag, no_tag } => {
            cmd_commit(&client, push, all, tag && !no_tag).await?
        }
        Commands::Staged => cmd_staged(&client).await?,
        Commands::Unstaged => cmd_unstaged(&client).await?,
        Commands::History { from, to, since, until, limit, delay } => {
            cmd_history(&client, from, to, since, until, limit, delay).await?
        }
        Commands::Pr { base, to, staged } => {
            cmd_pr(&client, base, to, &config.base_branch, staged).await?
        }
        Commands::Changelog { from, to, since, until, limit } => {
            cmd_changelog(&client, from, to, since, until, limit).await?
        }
        Commands::Explain { from, to, since, until, staged } => {
            cmd_explain(&client, from, to, since, until, &config.base_branch, staged).await?
        }
        Commands::Version { base, to, current } => {
            cmd_version(&client, base, to, &config.base_branch, current).await?
        }
        Commands::Init { .. } | Commands::Config => unreachable!(),
        Commands::Models => cmd_models(&client).await?,
    }

    Ok(())
}
