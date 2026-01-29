// src/provider/claudecode.rs
//! Claude Code CLI provider - uses local `claude` CLI subprocess.
//!
//! This provider allows users with Claude Max subscriptions to use their
//! subscription credits instead of paying separate API costs. The claude CLI
//! handles authentication via browser OAuth.
//!
//! Usage:
//! 1. Install: `curl -fsSL https://claude.ai/install.sh | bash`
//! 2. Authenticate: `claude login`
//! 3. Use: `gitar --provider claudecode commit`

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Message types from Claude CLI stream-json output
#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    #[serde(rename = "type")]
    msg_type: String,
    result: Option<String>,
    message: Option<MessageContent>,
}

#[derive(Debug, Deserialize)]
struct MessageContent {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

/// Check if the claude CLI is available
pub fn check_cli_available() -> Result<()> {
    let output = std::process::Command::new("claude")
        .arg("--version")
        .output();

    match output {
        Ok(o) if o.status.success() => Ok(()),
        Ok(_) => bail!(
            "Claude Code CLI is installed but not working properly.\n\
             Run 'claude login' to authenticate."
        ),
        Err(_) => bail!(
            "Claude Code CLI not found.\n\
             Install it with: curl -fsSL https://claude.ai/install.sh | bash\n\
             Then authenticate with: claude login"
        ),
    }
}

/// Chat with Claude via the local CLI subprocess.
///
/// Unlike other providers, this doesn't use HTTP - it spawns the `claude` CLI
/// and parses its stream-json output.
pub async fn chat(
    model: &str,
    system: &str,
    user: &str,
    stream: bool,
) -> Result<String> {
    // Verify CLI is available
    check_cli_available()?;

    // Build the prompt combining system and user messages
    let full_prompt = if system.is_empty() {
        user.to_string()
    } else {
        format!("{}\n\n{}", system, user)
    };

    // Build command
    // Note: --verbose is required when using -p with --output-format=stream-json
    let mut cmd = Command::new("claude");
    cmd.args([
        "-p",
        &full_prompt,
        "--model",
        model,
        "--output-format",
        "stream-json",
        "--max-turns",
        "1",
        "--verbose",
    ]);

    // CRITICAL: Unset ANTHROPIC_API_KEY to force subscription auth
    // If this env var is set, claude CLI will use API instead of subscription
    cmd.env_remove("ANTHROPIC_API_KEY");

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .context("Failed to spawn claude CLI process")?;

    let stdout = child
        .stdout
        .take()
        .context("Failed to capture claude CLI stdout")?;

    let mut reader = BufReader::new(stdout).lines();
    let mut result_text = String::new();
    let mut assistant_text = String::new();

    // Parse streaming JSON output line by line
    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(msg) = serde_json::from_str::<ClaudeMessage>(&line) {
            match msg.msg_type.as_str() {
                "assistant" => {
                    // Extract text from assistant messages
                    if let Some(message) = msg.message {
                        for block in message.content {
                            if block.block_type == "text" {
                                if let Some(text) = block.text {
                                    if stream {
                                        print!("{}", text);
                                        io::stdout().flush()?;
                                    }
                                    assistant_text.push_str(&text);
                                }
                            }
                        }
                    }
                }
                "result" => {
                    // Final result - prefer this if available
                    if let Some(result) = msg.result {
                        result_text = result;
                    }
                }
                "error" => {
                    bail!("Claude CLI error: {}", line);
                }
                _ => {
                    // Ignore system, init, and other message types
                }
            }
        }
    }

    // Wait for process to complete
    let status = child.wait().await?;

    if !status.success() {
        bail!(
            "Claude CLI exited with error (code {:?}).\n\
             Make sure you're authenticated: run 'claude login'",
            status.code()
        );
    }

    // Print newline if we were streaming
    if stream && !assistant_text.is_empty() {
        println!();
    }

    // Prefer result field, fall back to accumulated assistant text
    let final_text = if !result_text.is_empty() {
        result_text
    } else {
        assistant_text
    };

    if final_text.is_empty() {
        bail!("Empty response from Claude CLI");
    }

    Ok(final_text.trim().to_string())
}

/// List available models for Claude Code CLI.
///
/// The CLI supports simple model names: sonnet, opus, haiku.
/// We return these as the available options.
pub async fn list_models() -> Result<Vec<String>> {
    // Claude CLI accepts these simple model names
    Ok(vec![
        "sonnet".to_string(),
        "opus".to_string(),
        "haiku".to_string(),
    ])
}

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_assistant_message() {
        let json = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello world"}]}}"#;
        let msg: ClaudeMessage = serde_json::from_str(json).unwrap();

        assert_eq!(msg.msg_type, "assistant");
        let content = msg.message.unwrap().content;
        assert_eq!(content.len(), 1);
        assert_eq!(content[0].block_type, "text");
        assert_eq!(content[0].text.as_ref().unwrap(), "Hello world");
    }

    #[test]
    fn parse_result_message() {
        let json = r#"{"type":"result","subtype":"success","result":"Final output","duration_ms":1234}"#;
        let msg: ClaudeMessage = serde_json::from_str(json).unwrap();

        assert_eq!(msg.msg_type, "result");
        assert_eq!(msg.result.unwrap(), "Final output");
    }

    #[test]
    fn parse_system_message() {
        let json = r#"{"type":"system","subtype":"init","session_id":"abc123"}"#;
        let msg: ClaudeMessage = serde_json::from_str(json).unwrap();

        assert_eq!(msg.msg_type, "system");
        assert!(msg.result.is_none());
        assert!(msg.message.is_none());
    }

    #[tokio::test]
    async fn list_models_returns_expected() {
        let models = list_models().await.unwrap();
        assert!(models.contains(&"sonnet".to_string()));
        assert!(models.contains(&"opus".to_string()));
        assert!(models.contains(&"haiku".to_string()));
    }
}
