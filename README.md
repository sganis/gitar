[![Build status](https://github.com/sganis/gitar/actions/workflows/ci.yml/badge.svg)](https://github.com/sganis/gitar/actions)

# 🎸 gitar

**gitar** is an AI-powered Git assistant that generates **commit messages, PR descriptions, changelogs, explanations, and version bump suggestions** directly from your diffs and history.

It supports:
- **OpenAI**
- **Anthropic Claude**
- **Google Gemini**
- **Groq**
- **Ollama** (local models)
- **Any OpenAI-compatible API** (OpenRouter, Together, Mistral, etc.)

The name combines **git** + **AI** + **Rust** (and happens to sound like *guitar*).

---

## Features

- **commit** — Interactive commit with AI-generated message
- **staged / unstaged** — Generate commit message for staged or unstaged changes
- **history** — Generate meaningful messages for existing commit history
- **pr** — Generate PR descriptions from branch changes
- **changelog** — Generate release notes from commits
- **explain** — Explain changes in plain English for non-technical stakeholders
- **version** — Suggest semantic version bumps based on changes
- **models** — List available models from the API

---

## Why Rust?

gitar is built with Rust for:

- **Performance** — Fast startup, low memory footprint
- **Single binary** — No Python/Node.js runtime, no dependencies
- **Cross-platform** — Linux, macOS, Windows
- **Reliability** — Memory safety without garbage collection

---

## Installation

### Download pre-built binary (recommended)

Download the latest release for your platform from the [Releases page](https://github.com/sganis/gitar/releases).


#### Linux
```bash
# Download (replace URL with latest release)
curl -LO https://github.com/sganis/gitar/releases/latest/download/gitar-linux-x64-1.0.0.tar.gz

# Extract
tar -xzf gitar-linux-x64-1.0.0.tar.gz

# Make executable and move to PATH
chmod +x gitar
sudo mv gitar /usr/local/bin/

# Verify installation
gitar --version
```

#### macOS (Apple Silicon)
```bash
curl -LO https://github.com/sganis/gitar/releases/latest/download/gitar-macos-arm64-1.0.0.tar.gz
tar -xzf gitar-macos-arm64-1.0.0.tar.gz
chmod +x gitar
sudo mv gitar /usr/local/bin/
```

#### Windows

1. Download `gitar-windows-x64-1.0.0.zip` from [Releases](https://github.com/sganis/gitar/releases)
2. Extract the zip file
3. Move `gitar.exe` to a folder in your PATH, or add its location to PATH
4. Open a new terminal and run `gitar --version`

---

### Build from source

Requires [Rust](https://rustup.rs/) toolchain.
```bash
git clone https://github.com/sganis/gitar.git
cd gitar
cargo build --release
```

Binary will be at:
```
target/release/gitar
```

Add it to your PATH or copy to `/usr/local/bin`.

---

### Install via Cargo
```bash
cargo install --git https://github.com/sganis/gitar.git
```

---


## Quick Start

The easiest way to configure gitar is using the `--provider` option:
```bash
# OpenAI
export OPENAI_API_KEY="sk-..."
gitar init --provider openai

# Anthropic Claude
export ANTHROPIC_API_KEY="sk-ant-..."
gitar init --provider claude

# Google Gemini
export GEMINI_API_KEY="AIza..."
gitar init --provider gemini

# Groq
export GROQ_API_KEY="gsk_..."
gitar init --provider groq --model llama-3.3-70b-versatile

# Ollama (local, no API key needed)
gitar init --provider ollama --model llama3.2:latest
```

Then start using it:
```bash
gitar commit      # Interactive commit with AI message
gitar staged      # Generate message for staged changes
gitar changelog   # Generate release notes
```

---

## Configuration

### Environment variables
```bash
# OpenAI
export OPENAI_API_KEY="sk-..."

# Anthropic (Claude)
export ANTHROPIC_API_KEY="sk-ant-..."

# Google Gemini
export GEMINI_API_KEY="AIza..."

# Groq
export GROQ_API_KEY="gsk_..."
```

The correct variable is auto-selected based on the provider/base URL.

---

### Config file

Create config via CLI using `--provider` (recommended):
```bash
# OpenAI
gitar init --provider openai --model "gpt-5-chat-latest"

# Anthropic Claude
gitar init --provider claude --model "claude-sonnet-4-5-20250929"

# Google Gemini
gitar init --provider gemini --model "gemini-2.5-flash"

# Groq
gitar init --provider groq --model "llama-3.3-70b-versatile"

# Ollama (local)
gitar init --provider ollama --model "llama3.2:latest"
```

Or using `--base-url` directly:
```bash
gitar init --base-url "https://api.anthropic.com/v1" --model "claude-sonnet-4-5-20250929"
```

Or create `~/.gitar.toml` manually:
```toml
api_key = "sk-..."
model = "gpt-5-chat-latest"
max_tokens = 500
temperature = 0.5
base_branch = "main"
# base_url = "https://api.openai.com/v1"                   # OpenAI (default)
# base_url = "https://api.anthropic.com/v1"                # Anthropic
# base_url = "https://generativelanguage.googleapis.com"   # Gemini
# base_url = "https://api.groq.com/openai/v1"              # Groq
# base_url = "http://localhost:11434/v1"                   # Ollama
```

---

### Provider shortcuts

The `--provider` option is a convenient shortcut for setting the base URL:

| Provider     | Aliases              | Base URL                                    |
| ------------ | -------------------- | ------------------------------------------- |
| `openai`     |                      | `https://api.openai.com/v1`                 |
| `claude`     | `anthropic`          | `https://api.anthropic.com/v1`              |
| `gemini`     |                      | `https://generativelanguage.googleapis.com` |
| `groq`       |                      | `https://api.groq.com/openai/v1`            |
| `ollama`     | `local`              | `http://localhost:11434/v1`                 |

Use with any command:
```bash
gitar --provider claude staged
gitar --provider gemini commit -a
gitar --provider ollama --model codellama:13b history -n 5
```

---

### Configuration priority

| Priority | Source               | Notes                        |
| -------: | -------------------- | ---------------------------- |
|        1 | `--api-key`          | CLI argument                 |
|        2 | Environment variable | Auto-selected by provider    |
|        3 | `~/.gitar.toml`      | Config file                  |

Environment variables checked:

* OpenAI: `OPENAI_API_KEY`
* Anthropic: `ANTHROPIC_API_KEY`
* Gemini: `GEMINI_API_KEY`
* Groq: `GROQ_API_KEY` (fallback: `OPENAI_API_KEY`)
* Ollama: (none required)

---

### View current config
```bash
gitar config
```

Output shows detected provider:
```
Config file: /home/user/.gitar.toml

api_key:     sk-ant-a...
model:       claude-sonnet-4-5-20250929
max_tokens:  500
temperature: 0.5
base_url:    https://api.anthropic.com/v1 (claude)
base_branch: main

Priority: --api-key > env var > config file
Env vars checked: ANTHROPIC_API_KEY
```

---

## Supported APIs

| Provider              | Base URL                                    | Default Model                |
| --------------------- | ------------------------------------------- | ---------------------------- |
| OpenAI                | `https://api.openai.com/v1`                 | `gpt-5-chat-latest`          |
| Anthropic             | `https://api.anthropic.com/v1`              | `claude-sonnet-4-5-20250929` |
| Google Gemini         | `https://generativelanguage.googleapis.com` | `gemini-2.5-flash`           |
| Groq                  | `https://api.groq.com/openai/v1`            | `gpt-5-chat-latest`          |
| Ollama                | `http://localhost:11434/v1`                 | (specify with `--model`)     |
| Any OpenAI-compatible | Custom                                      | (specify with `--model`)     |

---

## Model Recommendations

For **gitar** (git diffs, summaries, commit messages):

* **Best quality (paid)**: `claude-sonnet-4-5-20250929` or `gemini-2.5-pro`
* **Best Gemini default**: `gemini-2.5-flash`
* **Best free/local**: Ollama `qwen2.5-coder:14b-instruct` or `llama3.2:latest`
* **Best cheap API**: Groq `llama-3.3-70b-versatile`

---

## Setup Examples
```bash
# Ollama (100% local, free)
ollama pull llama3.2
gitar init --provider ollama --model "llama3.2:latest"

# Groq (very fast, cheap)
export GROQ_API_KEY="gsk_..."
gitar init --provider groq --model "llama-3.3-70b-versatile"

# Google Gemini
export GEMINI_API_KEY="AIza..."
gitar init --provider gemini --model "gemini-2.5-flash"

# OpenAI
export OPENAI_API_KEY="sk-..."
gitar init --provider openai --model "gpt-5-chat-latest"

# Anthropic Claude
export ANTHROPIC_API_KEY="sk-ant-..."
gitar init --provider claude --model "claude-sonnet-4-5-20250929"
```

---

## Usage

### Quick reference
```bash
gitar commit                    # Interactive commit
gitar commit -a -p              # Stage all, commit, push

gitar staged                    # Message for staged changes
gitar unstaged                  # Message for unstaged changes

gitar history v1.0.0            # Regenerate messages since tag
gitar history v1.0.0 --to v1.1.0

gitar changelog v1.0.0          # Release notes since tag
gitar pr                        # PR description
gitar explain                   # Explain for non-technical audience
gitar version                   # Suggest version bump
gitar models                    # List available models
```

### Using different providers per command
```bash
gitar --provider claude staged
gitar --provider gemini --model gemini-2.5-pro pr
gitar --provider ollama --model codellama:13b explain
```

---

## Range Support

All commands support `--to`:
```bash
gitar changelog v1.0.0
gitar changelog v1.0.0 --to v1.0.1
gitar changelog v1.0.1 --to v2.0.0
```

---

## Commands Overview

### commit

Interactive AI commit:
```bash
gitar commit [-a] [-p] [--no-tag]
```

| Flag       | Description                    |
| ---------- | ------------------------------ |
| `-a`       | Stage all changes              |
| `-p`       | Push after commit              |
| `--no-tag` | Don't add AI model tag to message |

---

### staged / unstaged
```bash
gitar staged
gitar unstaged
```

Pipe to clipboard:
```bash
gitar staged | pbcopy      # macOS
gitar staged | xclip       # Linux
```

---

### history

Regenerate commit messages for existing commits:
```bash
gitar history [REF] [--to REF] [--since DATE] [--until DATE] [-n N] [--delay MS]
```

| Option    | Description                          |
| --------- | ------------------------------------ |
| `REF`     | Starting point (tag, commit, branch) |
| `--to`    | Ending point (default: HEAD)         |
| `--since` | Commits newer than date              |
| `--until` | Commits older than date              |
| `-n`      | Max commits (default: 50)            |
| `--delay` | Delay between API calls (ms)         |

---

### pr

Generate PR description:
```bash
gitar pr [BASE] [--to REF] [--staged]
```

| Option     | Description                    |
| ---------- | ------------------------------ |
| `BASE`     | Base ref to compare against    |
| `--to`     | Ending point                   |
| `--staged` | Use staged changes only        |

---

### changelog

Generate release notes:
```bash
gitar changelog [REF] [--to REF] [--since DATE] [--until DATE] [-n N]
```

---

### explain

Explain changes for non-technical stakeholders:
```bash
gitar explain [REF] [--to REF] [--staged]
```

---

### version

Suggest semantic version bump:
```bash
gitar version [REF] [--to REF] [--current X.Y.Z]
```

---

### models

List available models from the configured API:
```bash
gitar models
```

---

### init

Save configuration to `~/.gitar.toml`:
```bash
gitar init [--provider PROVIDER] [--api-key KEY] [--model MODEL] [--base-url URL] ...
```

---

### config

Show current configuration:
```bash
gitar config
```

---

## Global Options

| Option          | Description                                        |
| --------------- | -------------------------------------------------- |
| `--provider`    | Provider shortcut: openai, claude, gemini, groq, ollama |
| `--api-key`     | Override API key                                   |
| `--model`       | Override model                                     |
| `--max-tokens`  | Override max tokens                                |
| `--temperature` | Override temperature                               |
| `--base-url`    | Override API URL                                   |
| `--base-branch` | Override base branch                               |

All global options work with any command:
```bash
gitar --provider claude --model claude-opus-4-5-20251101 staged
gitar --temperature 0.8 commit
```

---

## Environment Variables

| Variable            | Description               |
| ------------------- | ------------------------- |
| `OPENAI_API_KEY`    | OpenAI API key            |
| `ANTHROPIC_API_KEY` | Anthropic API key         |
| `GEMINI_API_KEY`    | Gemini API key            |
| `GROQ_API_KEY`      | Groq API key              |
| `OPENAI_BASE_URL`   | Override default base URL |
| `GITAR_PROXY`       | HTTP/SOCKS proxy          |

---

## Using SSH Tunnel / SOCKS5 (Restricted Networks)
```bash
ssh -N -D 8000 user@jump-host
export GITAR_PROXY="socks5h://localhost:8000"
```

Example:
```bash
export GROQ_API_KEY="gsk_..."
gitar init --provider groq --model "llama-3.3-70b-versatile"
gitar commit
```

---

## Examples

### Daily workflow
```bash
# Make changes, then commit with AI message
gitar commit -a

# Or review the message first
gitar staged
gitar commit
```

### Generate changelog for a release
```bash
gitar changelog v1.0.0
gitar changelog v1.0.0 --to v1.1.0 > CHANGELOG.md
```

### Explain changes to non-technical team
```bash
gitar explain v1.0.0
```

### Switch providers on the fly
```bash
# Use Claude for complex explanations
gitar --provider claude explain

# Use fast local model for quick commits
gitar --provider ollama --model llama3.2 commit -a
```

---

## Project Structure
```
src/
├── main.rs      CLI definition, command handlers, entry point
├── config.rs    Configuration loading, provider constants, settings resolution
├── client.rs    LlmClient - routes requests to appropriate provider
├── git.rs       Git operations: diff, logs, branches, version detection
├── prompts.rs   System and user prompt templates for all commands
├── types.rs     API request/response structs for all providers
├── openai.rs    OpenAI-compatible API (also used by Groq, Ollama)
├── claude.rs    Anthropic Claude API
└── gemini.rs    Google Gemini API
```

| File | Description |
|------|-------------|
| `main.rs` | CLI parsing with clap, command implementations, async entry point |
| `config.rs` | Loads `~/.gitar.toml`, resolves config priority (CLI > env > file), provider URL mapping |
| `client.rs` | Unified `LlmClient` that detects provider from URL and delegates to appropriate module |
| `git.rs` | Wrappers for git commands, diff truncation, commit log parsing, exclude patterns |
| `prompts.rs` | Prompt constants for commit, PR, changelog, explain, and version commands |
| `types.rs` | Serde structs for OpenAI, Claude, and Gemini API request/response formats |
| `openai.rs` | OpenAI chat completions with reasoning model auto-detection and retry logic |
| `claude.rs` | Anthropic Messages API with proper headers and response parsing |
| `gemini.rs` | Google Generative AI API with URL/model path normalization |

--- 

## License

MIT
