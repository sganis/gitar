[![Build status](https://github.com/sganis/gitar/actions/workflows/ci.yml/badge.svg)](https://github.com/sganis/gitar/actions)

# 🎸 Gitar

**Gitar** is an AI-powered Git assistant that generates **commit messages, PR descriptions, changelogs, explanations, and version bump suggestions** directly from your diffs and history.

Gitar supports:

- **openai** — OpenAI & compatible APIs (OpenRouter, Together, Mistral, …)
- **claude** — Anthropic
- **gemini** — Google
- **groq**   — hosted LLM inference API
- **ollama** — local models

The name combines **Git** + **Ai** + **Rust** (and happens to sound like *guitar*).

---

## Features

- **commit** — Interactive commit with AI-generated message
- **staged / unstaged** — Generate commit message for staged or unstaged changes
- **history** — Generate meaningful messages for existing commit history
- **pr** — Generate PR descriptions from branch changes
- **changelog** — Generate release notes from commits
- **explain** — Explain changes in plain English for non-technical stakeholders
- **version** — Suggest semantic version bumps based on changes
- **models** — List available models (when the provider exposes a models endpoint)
- **hook** — Install Git hook to auto-generate commit messages on `git commit`
- **diff** — Preview/compare what would be sent to the LLM (debug tool)

---

## Installation

### Download pre-built binary (recommended)

Download the latest release for your platform from the [Releases page](https://github.com/sganis/gitar/releases).

If you have the GitHub CLI installed, you can download the latest matching asset without hardcoding a version:

#### Linux (x64)
```bash
gh release download --repo sganis/gitar --pattern "gitar-linux-x64-*.tar.gz"
tar -xzf gitar-linux-x64-*.tar.gz
chmod +x gitar
sudo mv gitar /usr/local/bin/
gitar --version
````

#### macOS (Apple Silicon)

```bash
gh release download --repo sganis/gitar --pattern "gitar-macos-arm64-*.tar.gz"
tar -xzf gitar-macos-arm64-*.tar.gz
chmod +x gitar
sudo mv gitar /usr/local/bin/
gitar --version
```

#### Windows (x64)

```powershell
gh release download --repo sganis/gitar --pattern "gitar-windows-x64-*.zip"
Expand-Archive -Path (Get-ChildItem gitar-windows-x64-*.zip).Name -DestinationPath .
# Move gitar.exe to a folder in your PATH (or add its folder to PATH)
gitar.exe --version
```

> Prefer manual downloads? Just grab the correct asset from the Releases page.

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

# Groq (OpenAI-compatible)
export GROQ_API_KEY="gsk_..."
gitar init --provider groq --model llama-3.3-70b-versatile

# Ollama (local, no API key needed)
gitar init --provider ollama --model llama3.2:latest
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
gitar models                    # List available models (when supported)

gitar hook install              # Install git commit hook

gitar diff --compare            # Compare smart diff algorithms side-by-side
```

---

## Style Presets

Gitar can tailor commit message style to your project's language conventions:

```bash
gitar commit --preset rust      # Rust: crate/module focused
gitar commit --preset js        # JavaScript: component/hook focused  
gitar commit --preset python    # Python: module/endpoint focused
gitar commit --preset auto      # Auto-detect from project files (default)
```

### Auto-detection

When `--preset auto` (or not specified), Gitar detects your project type:

| File Present | Detected Preset |
|--------------|-----------------|
| `Cargo.toml` | rust |
| `package.json` | javascript |
| `pyproject.toml`, `setup.py`, `requirements.txt` | python |

You can also set a default preset in `~/.gitar.toml`:

```toml
preset = "rust"
```

---

## Secret Detection & Protection

Gitar automatically scans diffs for sensitive data **before sending to any LLM**:

- API keys (OpenAI, Anthropic, GitHub, AWS, Stripe, etc.)
- Private keys and certificates
- Database connection strings
- Passwords and tokens
- JWTs and bearer tokens

### Actions

Configure how Gitar handles detected secrets in `~/.gitar.toml`:

```toml
# Options: "redact" (default), "warn", "block"
secret_action = "redact"
```

| Action | Behavior |
|--------|----------|
| `redact` | Replace secrets with `[REDACTED:TYPE:Nch]` before sending |
| `warn` | Show warning but send original content |
| `block` | Abort the command entirely |

### Example output

When secrets are detected:

```
⚠️  Found 2 potential secret(s):
   ● 1 HIGH
   ● 1 MEDIUM

   1. [OpenAI Key:L3] +API_KEY=sk-p...5pqr[REDACTED]
   2. [Password:L7] +DB_PASS=secr...[REDACTED]

✓ Secrets redacted before sending to LLM
```

---

## Smart Diff Algorithms (Context Optimization)

Large diffs can blow up context windows and cost tokens. Gitar can **shape** the diff before sending it to your LLM, using one of four algorithms.

Most commands accept:

```bash
--algo <1..4>
```

### Algorithms

* **1 — Full Diff**
  Sends the raw `git diff` (best fidelity, worst token usage).

* **2 — Selective Files**
  Splits the diff by file, filters out obvious noise (lockfiles / vendored / generated paths), ranks files by importance, and packs whole-file patches until the size limit is hit.

* **3 — Selective Hunks**
  Extracts hunks across files, scores them (structural changes, meaningful additions/removals, etc.), then packs the highest scoring hunks first. Includes a per-file cap so one file can't dominate.

* **4 — Semantic JSON** *(default)*
  Produces a compact JSON "intermediate representation" with a file summary (path, status, adds/dels, priority) and a top-ranked hunks with short previews. It adaptively shrinks previews / hunk count until it fits the size budget.

### Examples

Use a different algorithm when you know you're doing a big refactor:

```bash
gitar commit --algo 3
gitar pr --algo 4
gitar changelog v1.0.0 --algo 2
gitar explain --staged --algo 4
```

Debug what will be sent to the model:

```bash
gitar diff --algo 2 --max-chars 15000 --stats
gitar diff --compare
```

---

## Using gitar Behind Firewalls (SSH Tunnel / Proxy)

If your machine **does not have direct internet access** (corporate / air-gapped / restricted network), you can still use gitar by tunneling traffic through another machine.

Gitar supports **HTTP and SOCKS proxies** via the `ALL_PROXY` environment variable.

On your **local machine**, open a SOCKS proxy tunnel:

```bash
ssh -N -D 8000 user@machine-with-internet
```

This opens a local SOCKS5 proxy at:

```
socks5h://localhost:8000
```

Now tell gitar to use it:

#### Linux / macOS

```bash
export ALL_PROXY="socks5h://localhost:8000"
```

Gitar will now route **all LLM API traffic** through the SSH tunnel.

---

## Git Hook

Gitar can **automatically generate commit messages every time you run `git commit`** by installing a Git hook.

### Install the hook

Run this **once per repository**:

```bash
gitar hook install
```

This installs a `prepare-commit-msg` hook that:

* Runs `gitar commit` automatically
* Writes the AI-generated message into the commit message file
* Opens your editor with the message already filled in
* Does **nothing** if you use `git commit -m` or `git commit -F`

### Daily usage

After installing the hook, your workflow becomes:

```bash
git add .
git commit
```

That's it. The message is generated automatically.

### What exactly happens?

On `git commit`:

* Git calls the `prepare-commit-msg` hook
* The hook runs:

```bash
gitar commit --write-to .git/COMMIT_EDITMSG --silent
```

* Your editor opens with a **ready-to-use AI commit message**

You can still edit it before saving.

### Uninstall

Run **gitar hook uninstall** or simply delete **.git/hooks/prepare-commit-msg**

---

## Configuration

Gitar stores configuration in `~/.gitar.toml`. Here's a complete example:

```toml
# Global settings
default_provider = "openai"
base_branch = "main"
max_diff_chars = 50000
preset = "auto"
secret_action = "redact"

# Provider-specific settings
[openai]
model = "gpt-4o"
max_tokens = 500
temperature = 0.5
stream = true

[claude]
model = "claude-sonnet-4-5-20250929"
max_tokens = 500
temperature = 0.5

[gemini]
model = "gemini-2.5-flash"

[groq]
model = "llama-3.3-70b-versatile"

[ollama]
model = "llama3.2:latest"
base_url = "http://localhost:11434/v1"
```

View your current configuration:

```bash
gitar config
```

---

## Security & Privacy

Gitar sends **only what it needs** for the command you run (for example: a diff, a commit range log, or staged changes).

**Built-in protections:**

* **Secret scanning** — Detects and optionally redacts API keys, passwords, and tokens before sending
* **Diff shaping** — Smart algorithms minimize what's sent while preserving context
* **No telemetry** — Gitar doesn't phone home or collect any data

**Tips:**

* Use **Ollama** for **100% local** inference (no network calls).
* Set `secret_action = "block"` in sensitive repositories.
* If you must run through restricted networks, use a proxy.
* If you work in sensitive repos, prefer smaller scopes (staged changes, specific ranges).

---

## License

MIT