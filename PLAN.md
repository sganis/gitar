# Gitar: Claude Code Provider Integration Handoff

## Goal
Add Claude Code CLI as a provider option in gitar, allowing users with Max subscriptions ($100-$200/mo) to use their subscription instead of paying separate API costs.

## Why This Matters
- Max subscribers get substantial usage included in their subscription
- No per-token API costs when using Claude Code provider
- Same Claude models (Sonnet, Opus) available

---

## Architecture Overview

### How It Works
```
┌─────────┐     ┌──────────────┐     ┌─────────────────┐
│  gitar  │────▶│ claude CLI   │────▶│ Claude (via     │
│         │     │ (subprocess) │     │ Max subscription)│
└─────────┘     └──────────────┘     └─────────────────┘
```

Claude Code CLI handles authentication via browser OAuth. Users run `claude login` once, then gitar shells out to `claude -p` for completions.

### Provider Selection
```
gitar --provider claude-code commit
gitar --provider anthropic commit    # existing API provider
gitar --provider openai commit       # existing
```

---

## Implementation

### 1. Add Provider Enum Variant

```rust
// src/config.rs or providers/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    OpenAI,
    Anthropic,
    Gemini,
    Groq,
    Ollama,
    ClaudeCode,  // NEW
}
```

### 2. Claude Code Provider Implementation

```rust
// src/providers/claude_code.rs

use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use serde::Deserialize;
use crate::error::GitarError;

#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    #[serde(rename = "type")]
    msg_type: String,
    subtype: Option<String>,
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

pub struct ClaudeCodeProvider {
    model: String,  // "sonnet" or "opus"
}

impl ClaudeCodeProvider {
    pub fn new(model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| "sonnet".to_string()),
        }
    }

    /// Check if claude CLI is available and authenticated
    pub fn check_availability() -> Result<(), GitarError> {
        let output = Command::new("claude")
            .arg("--version")
            .output()
            .map_err(|e| GitarError::ProviderError(
                format!("Claude Code CLI not found. Install: curl -fsSL https://claude.ai/install.sh | bash\nError: {}", e)
            ))?;

        if !output.status.success() {
            return Err(GitarError::ProviderError(
                "Claude Code CLI not working. Run 'claude login' to authenticate.".into()
            ));
        }
        Ok(())
    }

    pub async fn complete(&self, prompt: &str) -> Result<String, GitarError> {
        // Build command
        let mut cmd = Command::new("claude");
        cmd.args([
            "-p", prompt,
            "--model", &self.model,
            "--output-format", "stream-json",
            "--max-turns", "1",
            "--no-user-input",
        ]);

        // IMPORTANT: Unset API key to force subscription auth
        cmd.env_remove("ANTHROPIC_API_KEY");

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| GitarError::ProviderError(format!("Failed to spawn claude: {}", e)))?;

        let stdout = child.stdout.take()
            .ok_or_else(|| GitarError::ProviderError("Failed to capture stdout".into()))?;

        let reader = BufReader::new(stdout);
        let mut result_text = String::new();

        // Parse streaming JSON output
        for line in reader.lines() {
            let line = line.map_err(|e| GitarError::ProviderError(format!("Read error: {}", e)))?;
            
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
                                        result_text.push_str(&text);
                                    }
                                }
                            }
                        }
                    }
                    "result" => {
                        // Final result - prefer this if available
                        if let Some(result) = msg.result {
                            return Ok(result);
                        }
                    }
                    "error" => {
                        return Err(GitarError::ProviderError(
                            format!("Claude error: {:?}", msg)
                        ));
                    }
                    _ => {}
                }
            }
        }

        let status = child.wait()
            .map_err(|e| GitarError::ProviderError(format!("Wait error: {}", e)))?;

        if !status.success() {
            return Err(GitarError::ProviderError(
                "Claude CLI exited with error. Check 'claude login' status.".into()
            ));
        }

        if result_text.is_empty() {
            return Err(GitarError::ProviderError("Empty response from Claude".into()));
        }

        Ok(result_text.trim().to_string())
    }
}
```

### 3. Integration with Existing Provider Dispatch

```rust
// src/providers/mod.rs

impl Provider {
    pub async fn complete(&self, prompt: &str, config: &Config) -> Result<String, GitarError> {
        match self {
            Provider::OpenAI => { /* existing */ }
            Provider::Anthropic => { /* existing */ }
            Provider::ClaudeCode => {
                let provider = ClaudeCodeProvider::new(config.model.clone());
                provider.complete(prompt).await
            }
            // ...
        }
    }
}
```

### 4. Config File Support

```toml
# ~/.config/gitar/config.toml

provider = "claude-code"
model = "sonnet"  # or "opus" for Max $200 users

# Falls back to anthropic API if claude CLI not available
fallback_provider = "anthropic"
```

---

## CLI Output Format Reference

### stream-json format (recommended)
Each line is a separate JSON object:

```json
{"type":"system","subtype":"init","session_id":"abc123","tools":["Read","Write"]}
{"type":"assistant","message":{"content":[{"type":"text","text":"Here's the commit message..."}]}}
{"type":"result","subtype":"success","result":"feat: add user authentication","duration_ms":1234}
```

### Key message types to handle:
| Type | When | Action |
|------|------|--------|
| `system` | Start | Ignore or log |
| `assistant` | Response chunks | Extract `.message.content[].text` |
| `result` | Completion | Use `.result` as final output |
| `error` | Failure | Return error with details |

---

## Edge Cases & Gotchas

### 1. API Key Conflict
```rust
// CRITICAL: If ANTHROPIC_API_KEY is set, CLI uses API instead of subscription
cmd.env_remove("ANTHROPIC_API_KEY");
```

### 2. Authentication Check
```rust
// Before first use, verify auth status
pub fn is_authenticated() -> bool {
    Command::new("claude")
        .args(["-p", "hi", "--max-turns", "1", "--output-format", "text"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

### 3. Model Availability
- **Pro ($20)**: Sonnet default, limited Opus
- **Max $100**: Full Sonnet, more Opus
- **Max $200**: Full Opus access

Consider adding model validation:
```rust
if self.model == "opus" {
    eprintln!("Note: Opus requires Max subscription tier");
}
```

### 4. Rate Limits
Subscription usage is shared between web UI and CLI. Heavy gitar usage counts against their claude.ai limits.

### 5. No Windows Native (yet)
Claude Code CLI requires WSL on Windows. Add platform check:
```rust
#[cfg(target_os = "windows")]
compile_error!("Claude Code provider requires WSL on Windows");
```

---

## Testing Checklist

- [ ] `claude --version` works
- [ ] `claude login` completed (browser auth)
- [ ] `ANTHROPIC_API_KEY` unset when using this provider
- [ ] Handles empty diff gracefully
- [ ] Handles large diffs (token limits)
- [ ] Error messages are actionable
- [ ] Fallback to API provider works

---

## User-Facing Documentation

Add to README:
```markdown
### Using Claude Code Provider (Max Subscription)

If you have a Claude Max subscription ($100-$200/mo), you can use it with gitar:

1. Install Claude Code CLI:
   ```bash
   curl -fsSL https://claude.ai/install.sh | bash
   ```

2. Authenticate:
   ```bash
   claude login
   ```

3. Configure gitar:
   ```bash
   gitar config set provider claude-code
   gitar config set model sonnet  # or opus
   ```

4. Use normally:
   ```bash
   gitar commit
   ```

**Note**: Unset `ANTHROPIC_API_KEY` env var to use subscription auth.
```

---

## Future Enhancements

1. **Streaming output** - Show progress for long operations
2. **Session continuity** - Use `--continue` for multi-turn refinement
3. **Auto-detection** - Check if claude CLI exists and auto-select provider
4. **Usage tracking** - Warn when approaching subscription limits
