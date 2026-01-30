[![Build status](https://github.com/sganis/gitar/actions/workflows/ci.yml/badge.svg)](https://github.com/sganis/gitar/actions)

# 🎸 Gitar — The Git AI Agent

Plan, understand, fix, and release your Git history — safely.

Gitar is an AI-native Git interface that turns high-level intent into planned, auditable Git operations.

Instead of memorizing Git commands and manually staging, splitting, and rewriting commits, you ask Gitar to analyze your changes, propose a plan, and execute it safely — with previews and dry-runs by default.

Under the hood, Gitar is built around a **skill-first architecture**: most operations are implemented as deterministic, reusable **skills**—structured pipelines that take diffs, files, or commit ranges, call the LLM only when needed, and validate and post-process the results to produce reliable outputs (commit messages, changelogs, explanations, conflict resolutions, etc.). For higher-level tasks that require planning and multi-step reasoning, Gitar uses lightweight **agents** as orchestrators that coordinate multiple skills in a controlled, auditable way.

All layers share a powerful **context optimization engine**:

- Smart diff shaping algorithms (4 modes)
- Secret detection and redaction
- Language-specific style presets
- Token budget management

**Supported LLM Providers:**

- **openai** — OpenAI & compatible APIs (OpenRouter, Together, Mistral, …)
- **anthropic** — Anthropic Claude API
- **claudecode** — Claude via Max subscription (no API key needed)
- **gemini** — Google
- **groq** — Fast hosted inference
- **ollama** — Local models (100% private)

The name combines **Git** + **AI** + **Rust** (and sounds like *guitar*).

---

## Conceptual Model

Gitar manages Git history through **four actions**:

> **plan** (`p`) — create or reshape history
> **tell** (`t`) — understand and communicate history
> **fix** (`f`) — repair broken history (conflicts)
> **release** (`r`) — ship history

Single-letter shortcuts: `gitar p`, `gitar t`, `gitar f`, `gitar r`

All operations are **dry-run by default**.
**Nothing mutates without `--apply`.**

---

## Installation

### Quick Install (recommended)

#### Linux / macOS

```bash
curl -fsSL https://raw.githubusercontent.com/sganis/gitar/main/install/install.sh | bash
````

#### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/sganis/gitar/main/install/install.ps1 | iex
```

#### Windows CMD

```cmd
curl -fsSL https://raw.githubusercontent.com/sganis/gitar/main/install/install.cmd -o "%TEMP%\gitar-install.cmd" && "%TEMP%\gitar-install.cmd"
```

---

## Manual Installation

### Option 1 — Download Prebuilt Binary

1. Go to the **Releases** page on GitHub.
2. Download the archive matching your platform:

* **Linux (glibc):** `gitar-<version>-linux-x86_64.tar.gz`
* **Linux (musl, static):** `gitar-<version>-linux-x86_64-musl.tar.gz`
* **macOS (Apple Silicon):** `gitar-<version>-macos-aarch64.tar.gz`
* **Windows:** `gitar-<version>-windows-x86_64.zip`

3. Extract the archive.
4. Put `gitar` (or `gitar.exe`) somewhere in your `PATH`.

Verify:

```bash
gitar --version
```

---

### Option 2 — Build From Source (Rust)

Requirements:

* Rust 1.75+ (install from [https://rustup.rs](https://rustup.rs))

```bash
git clone https://github.com/sganis/gitar.git
cd gitar
cargo build --release
```

Binary will be at:

```bash
target/release/gitar
```

You can either:

```bash
cargo install --path .
```

or manually copy it to your PATH:

```bash
# Linux / macOS
cp target/release/gitar /usr/local/bin/

# Windows (PowerShell, admin)
copy target\release\gitar.exe C:\Windows\System32\
```

Verify:

```bash
gitar --version
```

---

### Option 3 — Cargo Install (Advanced)

```bash
cargo install --git https://github.com/sganis/gitar.git
```

---

## Quick Start

First-time setup:

```bash
gitar init
```

Or pick provider directly:

```bash
gitar init --provider openai
gitar init --provider anthropic
gitar init --provider claudecode  # uses Claude Max subscription
gitar init --provider gemini
gitar init --provider groq
gitar init --provider ollama
```

Then:

```bash
gitar          # == gitar plan (dry-run)
gitar --apply  # execute the plan
```

For power users who want commands to execute by default:

```bash
gitar init --auto-apply true
gitar          # now executes immediately
gitar --dry-run  # use this to preview
```

---

## Global Rules

* **Dry-run by default** (unless `auto_apply` is enabled)
* **Nothing mutates without `--apply`** (or use `--dry-run` to override `auto_apply`)
* Same scope flags everywhere:

```bash
--working
--staged
--history <ref>
```

Inference:

* If `--history` → history
* Else if staged exists → staged
* Else → working

### Auto-Apply Mode

For confident users who prefer commands to execute by default:

```bash
gitar init --auto-apply true   # Enable
gitar init --auto-apply false  # Disable
```

Or set in `~/.gitar/gitar.toml`:

```toml
auto_apply = true
```

When enabled:
- `gitar plan` executes immediately (like `gitar plan --apply`)
- `gitar fix` applies resolutions (like `gitar fix --apply`)
- `gitar release` executes the release (like `gitar release --apply`)

Use `--dry-run` to preview without executing when `auto_apply` is enabled:

```bash
gitar plan --dry-run    # Preview only
gitar fix --dry-run     # Preview only
gitar release --dry-run # Preview only
```

---

## Usage

## 1) 🚀 `gitar plan` — Core Product

Plan and reshape commits (working tree, staged, or history):

```bash
gitar plan
gitar plan -i
gitar plan --apply
gitar plan --history v1.0.0
```

What it does:

* Analyzes changes or history
* Builds a multi-commit plan
* Shows preview
* Allows interactive editing
* Executes only with `--apply`

---

## 2) 📝 `gitar tell` — Read-only Understanding & Communication

Everything that **describes, summarizes, or explains** goes here.

Instead of subcommands, `tell` uses **selector flags** (exactly one):

| Flag | Description |
|------|-------------|
| `--explain` | Plain English explanation for stakeholders (default) |
| `--weekly` | Executive weekly highlights report for leadership |
| `--commit` | Generate AI commit message |
| `--pr` | Generate PR description |
| `--changelog` | Generate release notes |
| `--history` | Describe a range of commits |

### Default Scope

When no reference is provided, commands default to:
1. **Latest tag to HEAD** (if a tag exists) — for most commands
2. **Last 7 days** — for `--weekly` (capped at 50 commits)
3. **Last 50 commits** (if no tags)
4. **Working tree** (for `--explain` without tags)

### Examples

```bash
# Explain changes (defaults to latest tag..HEAD or working tree)
gitar tell
gitar tell --explain
gitar tell --explain --staged
gitar tell --explain v1.0.0        # From specific ref

# Weekly executive highlights (defaults to last 7 days)
gitar tell --weekly
gitar tell --weekly --since "3 days ago"
gitar tell --weekly v1.0.0         # From specific ref

# Generate commit message
gitar tell --commit
gitar tell --commit -a             # Stage all first
gitar tell --commit --no-ai-author # Without [AI:model] tag

# PR description
gitar tell --pr
gitar tell --pr main               # Against specific base

# Changelog / release notes
gitar tell --changelog             # From latest tag
gitar tell --changelog v1.0.0      # From specific ref
gitar tell --changelog -n 20       # Last 20 commits

# History (rewrite commit messages)
gitar tell --history               # From latest tag
gitar tell --history v1.0.0        # From specific ref
gitar tell --history -n 10         # Last 10 commits
```

### Commit-specific Flags

| Flag | Description |
|------|-------------|
| `-a, --all` | Stage all changes before committing |
| `-p, --push` | Push after committing |
| `--amend` | Amend the last commit |
| `--ai-author` | Add `[AI:model]` tag to message (default: true) |
| `--no-ai-author` | Omit the AI tag |

---

## 3) 🩹 `gitar fix` — Conflict Resolution

```bash
gitar fix
gitar fix --apply
gitar fix --apply --yes
```

Strategy:

1. Heuristics (fast, deterministic)
2. Per-region LLM
3. Full-file LLM fallback

Safety:

* Always preview
* Ensures no markers remain
* Applies only with `--apply`

---

## 4) 🧰 `gitar release` — Guided Release Workflow

```bash
gitar release
gitar release --apply
gitar release --from v1.0.0
gitar release --skip-changelog
```

Does:

* Analyzes commits
* Suggests version bump
* Generates changelog
* Updates version files
* Creates commit + tag
* Never auto-pushes

### Supported Version Files

By default, gitar detects and updates:
- `Cargo.toml` (Rust)
- `package.json` (Node.js)
- `pyproject.toml` (Python)

### Custom Version Files

For projects with non-standard version locations (Tauri, monorepos, etc.), configure in `~/.gitar/gitar.toml`:

```toml
[release]
# Single file with JSON path
version_file = "src-tauri/tauri.conf.json"
version_json_path = "version"

# Or multiple files
version_files = [
    { file = "Cargo.toml" },
    { file = "src-tauri/tauri.conf.json", json_path = "version" },
    { file = "package.json", json_path = "version" },
]
```

For non-JSON files, use regex patterns:

```toml
# version.h with: #define VERSION "1.2.3"
version_files = [
    { file = "version.h", pattern = "#define VERSION \"(\\d+\\.\\d+\\.\\d+)\"", template = "#define VERSION \"${1}\"" },
]
```

---

## Utilities

```bash
gitar init
gitar init --show      # View resolved configuration
gitar models
gitar hook --install
gitar hook --uninstall
gitar diff
```

---

## Style Presets

```bash
gitar tell --commit --preset rust
gitar tell --commit --preset js
gitar tell --commit --preset python
```

Or set default:

```toml
preset = "rust"
```

---

## Custom Prompts

Override default LLM prompts by creating `~/.gitar/prompts.toml`:

```toml
[commit]
system = """
You generate Git commit messages.
Rules: single line, imperative mood, max 72 chars
"""
user = """
Generate a commit message for:
{diff}
"""
```

Run `gitar init` to generate a template with all available prompts.

### Available Prompt Types

| Type | Placeholders | Used By |
|------|--------------|---------|
| `commit` | `{diff}` | `gitar tell --commit`, `gitar plan` |
| `history` | `{diff}`, `{original_message}` | `gitar tell --history` |
| `pr` | `{branch}`, `{commits}`, `{stats}`, `{diff}` | `gitar tell --pr` |
| `changelog` | `{range}`, `{count}`, `{commits}` | `gitar tell --changelog` |
| `explain` | `{stats}`, `{diff}` | `gitar tell --explain` |
| `version` | `{version}`, `{diff}` | `gitar release` |
| `weekly` | `{stats}`, `{diff}` | `gitar tell --weekly` |

Each prompt type has a `system` and `user` field. Only override what you need — missing keys use built-in defaults.

### Project-Level Prompts

Create `.gitar/prompts.toml` in your repo root for project-specific prompts. Project prompts override user prompts.

---

## User Context

Add personal preferences that get injected into all prompts via `~/.gitar/context.md`:

```markdown
## Preferences
- Prefer short, imperative commit messages
- Use conventional commit format
- Avoid emojis
```

Project-level context in `.gitar/context.md` overrides user context.

---

## Secret Detection & Protection

Gitar scans diffs **before sending to any LLM**:

* API keys
* Tokens
* Passwords
* Certificates
* Connection strings

Config:

```toml
secret_action = "redact"  # or "warn" or "block"
```

---

## Smart Diff Algorithms

```bash
--algo <1..4>
```

1 = Full diff
2 = Selective files
3 = Selective hunks
4 = Semantic JSON (default)

Debug:

```bash
gitar diff --compare
```

---

## Configuration

### User Configuration

Stored in `~/.gitar/gitar.toml`:

```toml
default_provider = "anthropic"
base_branch = "main"
preset = "rust"
auto_apply = false        # Set to true to execute by default
secret_action = "redact"  # or "warn" or "block"

[anthropic]
model = "claude-sonnet-4-5-20250514"

# Or use Claude via Max subscription (no API key needed):
# default_provider = "claudecode"
# [claudecode]
# model = "sonnet"  # or "opus" for Max $200 users
```

### Shared Configuration (Enterprise/Cluster)

For shared environments (clusters, teams), set a config directory:

```bash
export GITAR_CONFIG_PATH=/shared/gitar-config
```

All config files resolve relative to this path:
- `$GITAR_CONFIG_PATH/gitar.toml` — Main configuration
- `$GITAR_CONFIG_PATH/prompts.toml` — Custom prompts (optional)
- `$GITAR_CONFIG_PATH/context.md` — User preferences (optional)

Falls back to `~/.gitar/` if not set.

**Config precedence** (highest to lowest):
1. CLI flags
2. User config (`~/.gitar/gitar.toml`)
3. System config (`$GITAR_CONFIG_PATH/gitar.toml`)
4. Built-in defaults

This allows admins to pre-configure gitar for users who can then use it immediately without running `gitar init`. Users can still override system settings with their own `~/.gitar/gitar.toml`.

### View Configuration

```bash
gitar init --show
```

Shows system config path, user config path, and the merged result.

---

## Git Hook

Install:

```bash
gitar hook --install
```

Uninstall:

```bash
gitar hook --uninstall
```

After installing:

```bash
git add .
git commit
```

And the message is generated automatically.

---

## Enterprise Deployment

### Environment Variables

| Variable | Purpose |
|----------|---------|
| `GITAR_CONFIG_PATH` | Path to config directory (contains gitar.toml, prompts.toml, context.md) |
| `GITAR_CA_FILE` | Path to custom CA certificate (PEM or DER) |
| `GITAR_PROXY` | HTTP or SOCKS5 proxy URL |

### Proxy / SSH Tunnel

```bash
export GITAR_PROXY="socks5h://localhost:8000"
export GITAR_PROXY="http://proxy.internal:8080"
```

### Custom CA Certificate

For internal HTTPS endpoints with custom certificate authorities:

```bash
export GITAR_CA_FILE=/path/to/internal-ca.cer
```

Supports both PEM and DER formats (`.cer`, `.crt`, `.pem`).

### Example Cluster Setup

```bash
# /etc/profile.d/gitar.sh (or module load script)
export GITAR_CONFIG_PATH=/shared/gitar
export GITAR_CA_FILE=/shared/certs/internal-ca.cer
export GITAR_PROXY=http://proxy.internal:8080  # optional
```

The shared directory should contain:
- `gitar.toml` — Main configuration (provider, API keys, defaults)
- `prompts.toml` — Custom prompts (optional)
- `context.md` — Organization preferences (optional)

With this setup, users can run gitar immediately without any configuration.

---

## Security & Privacy

* Sends only minimal data
* Secret scanning
* Diff shaping
* No telemetry
* Ollama supported for 100% local inference

---

## License

MIT
