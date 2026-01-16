[![Build status](https://github.com/sganis/gitar/actions/workflows/ci.yml/badge.svg)](https://github.com/sganis/gitar/actions)

# 🎸 gitar

**gitar** is an AI-powered Git assistant that generates **commit messages, PR descriptions, changelogs, explanations, and version bump suggestions** directly from your diffs and history.

It supports:
- **OpenAI**
- **Anthropic Claude**
- **Google Gemini**
- **Any OpenAI-compatible API** (Groq, Ollama, OpenRouter, Together, Mistral, etc.)

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

### From source

```bash
git clone https://github.com/sganis/gitar.git
cd gitar
cargo build --release
````

Binary will be at:

```
target/release/gitar
```

Add it to your PATH or copy to `/usr/local/bin`.

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

The correct variable is auto-selected based on `base_url`.

---

### Config file

Create config via CLI:

```bash
# OpenAI
gitar init --api-key "sk-..." --model "gpt-5-chat-latest" --base-branch "main"

# Anthropic
gitar init --api-key "sk-ant-..." --base-url "https://api.anthropic.com/v1" --model "claude-sonnet-4-5-20250929"
```

Or create `~/.gitar.toml` manually:

```toml
api_key = "sk-..."
model = "gpt-5-chat-latest"
max_tokens = 500
temperature = 0.5
base_branch = "main"
# base_url = "https://api.openai.com/v1"      # OpenAI (default)
# base_url = "https://api.anthropic.com/v1"   # Anthropic
# base_url = "https://generativelanguage.googleapis.com" # Gemini
```

---

### Configuration priority

| Priority | Source               | Notes                |
| -------: | -------------------- | -------------------- |
|        1 | `--api-key`          | CLI argument         |
|        2 | `~/.gitar.toml`      | Config file          |
|        3 | Environment variable | Auto-selected by API |

Environment variables checked:

* OpenAI: `OPENAI_API_KEY`
* Anthropic: `ANTHROPIC_API_KEY`
* Gemini: `GEMINI_API_KEY` (fallback: `GOOGLE_API_KEY`)
* Groq: `GROQ_API_KEY`

---

### View current config

```bash
gitar config
```

---

## Supported APIs

| Provider              | Base URL                                    | Default Model                |
| --------------------- | ------------------------------------------- | ---------------------------- |
| OpenAI                | `https://api.openai.com/v1`                 | `gpt-5-chat-latest`          |
| Anthropic             | `https://api.anthropic.com/v1`              | `claude-sonnet-4-5-20250929` |
| Google Gemini         | `https://generativelanguage.googleapis.com` | `gemini-2.5-flash`           |
| Groq                  | `https://api.groq.com/openai/v1`            | `openai/gpt-oss-20b`         |
| Ollama                | `http://localhost:11434/v1`                 | (choose model)               |
| Any OpenAI-compatible | Custom                                      | (choose model)               |

---

## Model Recommendations

For **gitar** (git diffs, summaries, commit messages):

* **Best Gemini default**: `gemini-2.5-flash`
* **Best quality (paid)**: `gemini-2.5-pro` or `claude-sonnet-4.5`
* **Best free/local**: Ollama `qwen2.5-coder:14b-instruct`
* **Best cheap API**: Groq `openai/gpt-oss-20b`

---

## Setup Examples

```bash
# Ollama (100% local, free)
ollama pull qwen2.5-coder:14b
gitar init --base-url "http://localhost:11434/v1" --model "qwen2.5-coder:14b-instruct"

# Groq (very cheap)
export GROQ_API_KEY="gsk_..."
gitar init --base-url "https://api.groq.com/openai/v1" --model "openai/gpt-oss-20b"

# Google Gemini
export GEMINI_API_KEY="AIza..."
gitar init --base-url "https://generativelanguage.googleapis.com" --model "gemini-2.5-flash"

# OpenAI
export OPENAI_API_KEY="sk-..."
gitar init --model "gpt-5-chat-latest"

# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."
gitar init --base-url "https://api.anthropic.com/v1" --model "claude-sonnet-4-5-20250929"
```

---

## Usage

### Quick reference

```bash
gitar commit
gitar commit -a -p

gitar staged
gitar unstaged

gitar history v1.0.0
gitar history v1.0.0 --to v1.1.0

gitar changelog v1.0.0
gitar pr
gitar explain
gitar version
gitar models
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

---

### staged / unstaged

```bash
gitar staged
gitar unstaged
```

Pipe to clipboard:

```bash
gitar staged | pbcopy
```

---

### history

```bash
gitar history [REF] [--to REF] [--since DATE] [--until DATE] [-n N] [--delay MS]
```

---

### pr

```bash
gitar pr [BASE] [--to REF] [--staged]
```

---

### changelog

```bash
gitar changelog [REF] [--to REF] [--since DATE] [--until DATE] [-n N]
```

---

### explain

```bash
gitar explain [REF] [--to REF] [--staged]
```

---

### version

```bash
gitar version [REF] [--to REF] [--current X.Y.Z]
```

---

### models

```bash
gitar models
```

---

### init

```bash
gitar init [--api-key KEY] [--model MODEL] [--base-url URL] ...
```

---

### config

```bash
gitar config
```

---

## Global Options

| Option          | Description          |
| --------------- | -------------------- |
| `--api-key`     | Override API key     |
| `--model`       | Override model       |
| `--max-tokens`  | Override max tokens  |
| `--temperature` | Override temperature |
| `--base-url`    | Override API URL     |
| `--base-branch` | Override base branch |

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
gitar init --base-url "https://api.groq.com/openai/v1" --model "openai/gpt-oss-20b"
gitar commit
```

---

## License

MIT

