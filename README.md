[![Build status](https://github.com/sganis/gitar/actions/workflows/ci.yml/badge.svg)](https://github.com/sganis/gitar/actions)

# gitar

AI-powered Git assistant that generates commit messages, PR descriptions, changelogs, and more using OpenAI and Anthropic (Claude) APIs. The name combines **git** + **AI** + **Rust** (and happens to sound like guitar 🎸).

## Features

- **commit** - Interactive commit with AI-generated message
- **staged / unstaged** - Generate commit message for staged or unstaged changes
- **history** - Generate meaningful messages for existing commit history
- **pr** - Generate PR descriptions from branch changes
- **changelog** - Generate release notes from commits
- **explain** - Explain changes in plain English for non-technical stakeholders
- **version** - Suggest semantic version bumps based on changes
- **models** - List available models from the API

## Why Rust?

gitar is built with Rust for:

- **Performance** - Fast startup and low memory footprint, no runtime overhead
- **Single binary** - No dependencies, no Python/Node.js runtime required
- **Cross-platform** - Compiles to native binaries for Linux, macOS, and Windows
- **Reliability** - Memory safety guarantees without garbage collection

## Installation

### From source

```bash
git clone https://github.com/sganis/gitar.git
cd gitar
cargo build --release
```

The binary will be at `target/release/gitar`. Add it to your PATH or copy to `/usr/local/bin/`.

## Configuration

### Environment variables

```bash
# For OpenAI
export OPENAI_API_KEY="sk-..."

# For Anthropic (Claude)
export ANTHROPIC_API_KEY="sk-ant-..."
```

The appropriate env var is auto-selected based on your `base_url`.

### Config file

Create a config file at `~/.gitar.toml`:

```bash
# For OpenAI
gitar init --api-key "sk-..." --model "gpt-5-chat-latest" --base-branch "main"

# For Anthropic
gitar init --api-key "sk-ant-..." --base-url "https://api.anthropic.com/v1" --model "claude-sonnet-4-5-20250929"
```

Or manually create `~/.gitar.toml`:

```toml
api_key = "sk-..."
model = "gpt-5-chat-latest"
max_tokens = 500
temperature = 0.5
base_branch = "main"
# base_url = "https://api.openai.com/v1"      # OpenAI (default)
# base_url = "https://api.anthropic.com/v1"   # Anthropic (Claude)
```

### Configuration priority

| Priority | Source | Notes |
|----------|--------|-------|
| 1 (highest) | `--api-key` | CLI argument |
| 2 | `~/.gitar.toml` | Config file |
| 3 (lowest) | Environment variable | Auto-selected based on API |

Environment variables checked:
- **OpenAI API**: `OPENAI_API_KEY`
- **Anthropic API**: `ANTHROPIC_API_KEY`

### View current config

```bash
gitar config
```

**Output:**
```
Config file: /home/user/.gitar.toml

api_key:     sk-abc12...
model:       gpt-5-chat-latest
max_tokens:  500
temperature: 0.5
base_url:    (not set)
base_branch: main

Priority: --api-key > config file > env var
Env vars checked: OPENAI_API_KEY
```

### Supported APIs

| Provider | Base URL | Default Model |
|----------|----------|---------------|
| OpenAI | `https://api.openai.com/v1` (default) | `gpt-5-chat-latest` |
| Anthropic | `https://api.anthropic.com/v1` | `claude-sonnet-4-5-20250929` |
| Ollama | `http://localhost:11434/v1` | (specify with `--model`) |
| Any OpenAI-compatible | Custom URL | (specify with `--model`) |

## Usage

All commands accept an optional `[REF]` argument (tag, commit, branch) as the starting point.

### Quick reference

```bash
gitar commit                    # Interactive commit with AI message
gitar commit -a -p              # Stage all, commit, and push

gitar staged                    # Generate message for staged changes
gitar unstaged                  # Generate message for unstaged changes

gitar history v1.0.0            # Commit history since tag
gitar history --since "1 week ago"

gitar changelog v1.0.0          # Release notes since tag
gitar changelog --since "1 week ago"

gitar pr                        # PR description vs main
gitar pr develop                # PR description vs develop

gitar explain v1.0.0            # Explain changes since tag
gitar explain --staged          # Explain staged changes

gitar version                   # Suggest version bump

gitar models                    # List available models
```

---

## Commands

### commit

Interactive commit with AI-generated message.

```bash
gitar commit [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `-a, --all` | Stage all changes before commit |
| `-p, --push` | Push after commit |
| `--no-tag` | Don't add `[AI:model]` suffix to message |

**Examples:**

```bash
gitar commit                    # Generate message, confirm, commit
gitar commit -p                 # Commit and push
gitar commit -a -p              # Stage all, commit, and push
gitar commit --no-tag           # Don't tag message with AI model
```

**Interactive prompt:**
```
Add user authentication with OAuth2 support

==================================================
  [Enter] Accept | [g] Regenerate | [e] Edit | [other] Cancel
==================================================
>
```

---

### staged / unstaged

Generate a commit message for staged or unstaged changes (non-interactive).

```bash
gitar staged                    # Message for staged changes
gitar unstaged                  # Message for unstaged changes
```

Useful for scripting or piping to clipboard:

```bash
gitar staged | pbcopy           # macOS
gitar staged | xclip            # Linux
```

---

### history

Generate meaningful commit messages for existing commit history.

```bash
gitar history [REF] [OPTIONS]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `[REF]` | Starting point (tag, commit, branch) |

**Options:**
| Option | Description |
|--------|-------------|
| `--since <DATE>` | Commits newer than date |
| `--until <DATE>` | Commits older than date |
| `-n, --limit <N>` | Max commits (default: 50 if no REF) |
| `--delay <MS>` | Delay between API calls (default: 500) |

**Examples:**

```bash
gitar history                   # Last 50 commits
gitar history -n 10             # Last 10 commits
gitar history v1.0.0            # All commits since tag
gitar history HEAD~5            # Last 5 commits
gitar history --since "1 week ago"
gitar history --since "2024-01-01" --until "2024-06-01"
```

**Output:**
```
[1/10] abc12345 | 2024-03-15 | San             | Added OAuth
  - Feat(auth):
    Add OAuth2 authentication with Google provider.
    Enables social login to reduce signup friction.
[2/10] def67890 | 2024-03-14 | San             | Fixed bug
  - Fix(api):
    Handle null response from payment gateway.
```

---

### pr

Generate a PR description from branch changes.

```bash
gitar pr [REF] [OPTIONS]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `[REF]` | Base ref to compare against (default: base_branch from config) |

**Options:**
| Option | Description |
|--------|-------------|
| `--staged` | Use only staged changes |

**Examples:**

```bash
gitar pr                        # PR against main (or configured base)
gitar pr develop                # PR against develop branch
gitar pr v1.0.0                 # PR against tag
gitar pr --staged               # PR from staged changes only
```

**Output:**
```
PR: feature/oauth -> main

## Summary
Adds OAuth2 authentication with Google provider.

## What Changed
- New OAuth middleware in src/auth/oauth.rs
- Login route handler in src/routes/login.rs
- Environment config for OAuth credentials

## Why
Users requested social login to reduce friction during signup.

## Risks
- First auth implementation - security review recommended

## Testing
- Manual testing with Google test account
- Verify token refresh after 1 hour

## Rollout
- Set GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET before deploy
```

---

### changelog

Generate release notes from commit history.

```bash
gitar changelog [REF] [OPTIONS]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `[REF]` | Starting point (tag, commit, branch) |

**Options:**
| Option | Description |
|--------|-------------|
| `--since <DATE>` | Commits newer than date |
| `--until <DATE>` | Commits older than date |
| `-n, --limit <N>` | Max commits (default: 50 if no REF, unlimited if REF given) |

**Examples:**

```bash
gitar changelog v1.0.0          # All commits since tag
gitar changelog HEAD~20         # Last 20 commits
gitar changelog                 # Recent 50 commits
gitar changelog --since "1 week ago"
gitar changelog --since "2024-01-01" --until "2024-03-01"
gitar changelog v1.0.0 -n 100   # Max 100 commits since tag
```

**Output:**
```
# Release Notes

## Features
- Add OAuth2 authentication with Google provider
- Add user profile page with avatar upload

## Fixes
- Fix payment timeout with retry logic
- Fix session expiry redirect loop

## Improvements
- Refactor database queries for connection pooling
- Improve error messages for validation failures

## Infrastructure
- Update Docker base image to Node 20
- Add GitHub Actions workflow for CI
```

---

### explain

Explain changes in plain English for non-technical stakeholders (PMs, designers, executives).

```bash
gitar explain [REF] [OPTIONS]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `[REF]` | Starting point (tag, commit, branch) |

**Options:**
| Option | Description |
|--------|-------------|
| `--since <DATE>` | Changes newer than date |
| `--until <DATE>` | Changes older than date |
| `--staged` | Use only staged changes |

**Examples:**

```bash
gitar explain                   # Explain current branch vs main
gitar explain v1.0.0            # Explain changes since tag
gitar explain HEAD~5            # Explain last 5 commits
gitar explain --staged          # Explain staged changes only
gitar explain --since "1 week ago"
```

**Output:**
```
## What's Changing
Users can now log in with their Google account instead of creating a password.

## User Impact
- New "Sign in with Google" button on login page
- Faster signup flow (one click instead of form)
- Existing users can link their Google account in settings

## Risk Level
Medium - New authentication system, recommend QA testing all login scenarios.

## Actions
- QA: Test login, logout, session timeout, account linking
- Docs: Update help center with new login option
```

---

### version

Suggest semantic version bump based on changes.

```bash
gitar version [REF] [OPTIONS]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `[REF]` | Base ref to compare against |

**Options:**
| Option | Description |
|--------|-------------|
| `--current <VERSION>` | Current version (default: from git tags) |

**Examples:**

```bash
gitar version                   # Analyze vs main
gitar version v1.0.0            # Analyze since tag
gitar version --current 1.2.3   # Specify current version
```

**Output:**
```
Recommendation: MINOR

Reasoning:
- New OAuth authentication feature added
- No breaking changes to existing APIs
- All changes are backwards compatible

Breaking changes: No
```

---

### models

List available models from the configured API.

```bash
gitar models
```

**Output (OpenAI):**
```
Fetching available models...

Available models:
  gpt-5-chat-latest
  gpt-5-chat-latest
  gpt-5-chat-latest-mini
  gpt-4-turbo
  ...
```

**Output (Anthropic):**
```
Fetching available models...

Available models:
  claude-opus-4-5-20251101
  claude-sonnet-4-5-20250929
  claude-haiku-4-5-20251001
  claude-opus-4-1-20250805
  claude-sonnet-4-20250514
  claude-opus-4-20250514
  ...
```

---

### init

Save configuration to `~/.gitar.toml`.

```bash
gitar init [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `--api-key <KEY>` | API key |
| `--model <MODEL>` | Model name |
| `--max-tokens <N>` | Max tokens (default: 500) |
| `--temperature <F>` | Temperature 0.0-2.0 (default: 0.5) |
| `--base-url <URL>` | API base URL |
| `--base-branch <BRANCH>` | Default base branch (default: main) |

**Examples:**

```bash
# OpenAI setup
gitar init --api-key "sk-..."
gitar init --model "gpt-5-chat-latest" --base-branch "develop"

# Anthropic (Claude) setup
gitar init --base-url "https://api.anthropic.com/v1" --api-key "sk-ant-..."
gitar init --base-url "https://api.anthropic.com/v1" --model "claude-opus-4-5-20251101"

# Local LLM (Ollama)
gitar init --base-url "http://localhost:11434/v1" --model "llama3"
```

---

### config

Display current configuration.

```bash
gitar config
```

---

## Global Options

These options can be used with any command:

| Option | Description |
|--------|-------------|
| `--api-key <KEY>` | Override API key |
| `--model <MODEL>` | Override model |
| `--max-tokens <N>` | Override max tokens |
| `--temperature <F>` | Override temperature |
| `--base-url <URL>` | Override API base URL |
| `--base-branch <BRANCH>` | Override base branch |

**Example:**

```bash
gitar --model gpt-5-chat-latest changelog v1.0.0
gitar --base-branch develop pr
gitar --base-url "https://api.anthropic.com/v1" --model claude-sonnet-4-5-20250929 staged
```

---

## API Pattern

All commands follow a consistent pattern mirroring Git's interface:

| Argument | Description | Commands |
|----------|-------------|----------|
| `[REF]` | Starting point (tag, commit, branch) | All |
| `--since` | Date filter (like `git log --since`) | changelog, history, explain |
| `--until` | Date filter (like `git log --until`) | changelog, history, explain |
| `-n, --limit` | Max items | changelog, history |
| `--staged` | Use staged changes only | pr, explain |

**Date formats** (same as Git):
- `"2024-01-01"`
- `"1 week ago"`
- `"yesterday"`
- `"2024-01-01 12:00:00"`

---

## Examples

### Release workflow

```bash
# See what changed since last release
gitar changelog v1.2.0

# Generate release notes for CHANGELOG.md
gitar changelog v1.2.0 > CHANGELOG.md

# Determine version bump
gitar version v1.2.0
```

### PR workflow

```bash
# Create feature branch
git checkout -b feature/oauth

# Make changes, commit with AI
gitar commit -a

# Generate PR description
gitar pr

# Or explain for PM review
gitar explain
```

### Daily development

```bash
# Quick commit with AI message
gitar commit -a -p

# See what you did yesterday
gitar changelog --since "yesterday"

# Review recent commit quality
gitar history -n 5
```

### Using different providers

```bash
# Use Claude for a single command
gitar --base-url "https://api.anthropic.com/v1" --model claude-sonnet-4-5-20250929 commit

# Switch default provider
gitar init --base-url "https://api.anthropic.com/v1"

# List available models
gitar models
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | OpenAI API key (used when base_url is OpenAI) |
| `ANTHROPIC_API_KEY` | Anthropic API key (used when base_url is Anthropic) |
| `OPENAI_BASE_URL` | Override default base URL |
| `GITAR_PROXY` | HTTP proxy for API requests |

---

## License

MIT