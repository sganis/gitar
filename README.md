[![Build status](https://github.com/sganis/gitar/actions/workflows/ci.yml/badge.svg)](https://github.com/sganis/gitar/actions)

# 🎸 Gitar

**Gitar** is an AI-native Git interface to **plan, tell, fix, and release** your history.

Instead of memorizing Git commands and manually staging, splitting, and rewriting commits, you ask Gitar to **analyze, propose a plan, and safely execute it**.

Under the hood, Gitar is built around a **skill-first architecture**: most operations are implemented as deterministic, reusable **skills**—structured pipelines that take diffs, files, or commit ranges, call the LLM only when needed, and validate and post-process the results to produce reliable outputs (commit messages, changelogs, explanations, conflict resolutions, etc.). For higher-level tasks that require planning and multi-step reasoning, Gitar uses lightweight **agents** as orchestrators that coordinate multiple skills in a controlled, auditable way.

All layers share a powerful **context optimization engine**:

- Smart diff shaping algorithms (4 modes)
- Secret detection and redaction
- Language-specific style presets
- Token budget management

**Supported LLM Providers:**

- **openai** — OpenAI & compatible APIs (OpenRouter, Together, Mistral, …)
- **claude** — Anthropic
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
curl -fsSL https://raw.githubusercontent.com/sganis/gitar/main/install/install.cmd -o install.cmd && install.cmd && del install.cmd
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
gitar init --provider claude
gitar init --provider gemini
gitar init --provider groq
gitar init --provider ollama
```

Then:

```bash
gitar          # == gitar plan (dry-run)
gitar --apply  # execute the plan
```

---

## Global Rules

* **Dry-run by default**
* **Nothing mutates without `--apply`**
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
| `--commit` | Generate AI commit message |
| `--pr` | Generate PR description |
| `--changelog` | Generate release notes |
| `--history` | Describe a range of commits |

### Default Scope

When no reference is provided, commands default to:
1. **Latest tag to HEAD** (if a tag exists)
2. **Last 50 commits** (if no tags)
3. **Working tree** (for `--explain` without tags)

### Examples

```bash
# Explain changes (defaults to latest tag..HEAD or working tree)
gitar tell
gitar tell --explain
gitar tell --explain --staged
gitar tell --explain v1.0.0        # From specific ref

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

---

## Utilities

```bash
gitar init
gitar config
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

Stored in:

```bash
~/.gitar.toml
```

View:

```bash
gitar config
```

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

## Behind Firewalls (Proxy / SSH Tunnel)

```bash
export ALL_PROXY="socks5h://localhost:8000"
```

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
