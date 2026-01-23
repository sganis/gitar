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

### Quick Install (recommended)

#### Linux/Mac:

```bash
curl -fsSL https://raw.githubusercontent.com/sganis/gitar/main/install/install.sh | bash
```

#### Windows PowerShell

```bash
irm https://raw.githubusercontent.com/sganis/gitar/main/istall/install.ps1 | iex
```

#### Windows CMD

```bash
curl -fsSL https://raw.githubusercontent.com/sganis/gitar/main/install/install.cmd -o install.cmd && install.cmd && del install.cmd
```


### Manual Install (download pre-built binary)

Download the latest release for your platform from the [Releases page](https://github.com/sganis/gitar/releases).

#### Linux (x64)
```bash
gh release download --repo sganis/gitar --pattern "gitar-linux-x64-*.tar.gz"
tar -xzf gitar-linux-x64-*.tar.gz
chmod +x gitar
sudo mv gitar /usr/local/bin/
gitar --version
```

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



### Uninstall

Run **gitar hook uninstall** or simply delete **.git/hooks/prepare-commit-msg**

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


---

# Future

# Gitar — AI-Native Git Interface

**Gitar** is an AI-powered Git CLI designed to **replace most direct Git usage** with a smarter, safer, AI-assisted workflow.

Instead of memorizing Git commands and manually staging, splitting, and explaining changes, you:

> **Ask Gitar to explain, plan, and safely execute.**

---

# Mental Model

Gitar has **four conceptual layers**:

1. 📝 **Narrate** → Explain and summarize Git state (read-only)
2. 🚀 **Plan** → Design commit structure and history (the product)
3. 🧰 **Release** → Guided, safe release workflow
4. 🩹 **Resolve** → Fix merge conflicts safely

Under the hood, all of them share:

> A powerful **diff shaping + secret detection + context optimization engine**.

---

# What Gitar Is Great At

- Writing high-quality commit messages
- Explaining diffs to non-technical people
- Generating changelogs and PR descriptions
- Planning clean commit history
- Splitting monster commits
- Safely guiding releases
- Fixing merge conflicts with fallback-heavy safety

---

# Quick Start

```bash
gitar init        # Create ~/.gitar.toml
gitar config      # Show current configuration

git add .
gitar commit      # Create a commit with AI-generated message
````

---

# Subsystems

---

## 📝 1) Narration Layer (Read-only, already mature)

> **Explain Git state in human language.**

These commands **do not modify history**. They only describe it.

### Available commands

```bash
gitar staged                    # Message for staged changes
gitar unstaged                  # Message for unstaged changes

gitar history v1.0.0            # Regenerate messages since tag
gitar history v1.0.0 --to v1.1.0

gitar changelog v1.0.0          # Release notes since tag
gitar pr                        # PR description
gitar explain                   # Explain for non-technical audience
gitar version                   # Suggest version bump

gitar diff --compare            # Compare smart diff algorithms side-by-side
gitar models                    # List available models (when supported)
```

All of these are:

> `Git state → shaped diff → LLM → text`

---

## 🚀 2) Planning Layer — `gitar plan` (THE PRODUCT)

> **AI-native commit planning and history shaping engine.**

This is the **core of Gitar**.

`gitar plan` (currently implemented as `split`) will:

* Analyze:

  * working tree
  * staged
  * untracked
  * or history ranges
* Propose:

  * number of commits
  * grouping
  * ordering
  * messages
* Let the human:

  * accept / reject
  * move files between groups
  * exclude files
  * regenerate plan
* Then:

  * execute Git plumbing safely

### Long-term goals

* Clean commit creation
* Splitting monster commits
* History rewrite
* Review-first workflows
* Staging strategy
* Commit message generation
* Changelog generation

> **AI proposes. Human approves. Git executes.**

---

## 🧰 3) Release Layer — `gitar release` (Guided workflow)

> **Safe, deterministic, boring release automation.**

This command will:

* Analyze commits since last tag
* Decide version bump
* Draft changelog
* Update version file(s)
* Commit
* Tag

It **never pushes**.

Think of it as:

> **“`gitar plan`, but specialized for releases.”**

---

## 🩹 4) Resolve Layer — `gitar resolve`

> **Merge conflict resolver with heavy safety rails.**

Features:

* Detect conflicts
* Parse conflict regions
* Try:

  * heuristics
  * per-region LLM
  * fallback to full-file LLM
* Enforce safety:

  * no markers remain
  * no unmerged files remain

Philosophy:

> **Safe. Cheap. Deterministic. Fallback-heavy.**

No AST. No heroics.

---

# Diff Shaping & Context Optimization (Core Infrastructure)

Large diffs can blow up context windows and cost tokens. Gitar can **shape** the diff before sending it to your LLM.

Most commands accept:

```bash
--algo <1..4>
```

### Algorithms

* **1 — Full Diff**
  Sends the raw `git diff` (best fidelity, worst token usage).

* **2 — Selective Files**
  Splits the diff by file, filters out noise, ranks files by importance, packs whole-file patches.

* **3 — Selective Hunks**
  Extracts and ranks hunks across files, caps dominance per file.

* **4 — Semantic JSON** *(default)*
  Produces a compact JSON IR with file summaries and top-ranked hunks, adaptively shrinking until it fits.

### Debug

```bash
gitar diff --algo 2 --max-chars 15000 --stats
gitar diff --compare
```

---

# Secret Detection & Protection

Gitar scans diffs **before sending to any LLM**:

* API keys
* Private keys
* DB URLs
* Passwords
* Tokens
* JWTs

Configure in `~/.gitar.toml`:

```toml
secret_action = "redact"   # "redact" | "warn" | "block"
```

| Action | Behavior                                   |
| ------ | ------------------------------------------ |
| redact | Replace secrets with `[REDACTED:TYPE:Nch]` |
| warn   | Warn but send                              |
| block  | Abort                                      |

---

# Style Presets

Gitar adapts message style to your project:

```bash
gitar commit --preset rust
gitar commit --preset js
gitar commit --preset python
gitar commit --preset auto
```

Auto-detection:

| File                      | Preset     |
| ------------------------- | ---------- |
| Cargo.toml                | rust       |
| package.json              | javascript |
| pyproject.toml / setup.py | python     |

---

# Git Hook

Install once per repo:

```bash
gitar hook install
```

Then:

```bash
git add .
git commit
```

Gitar auto-generates the message via `prepare-commit-msg`.

---

# Configuration

`~/.gitar.toml` example:

```toml
default_provider = "openai"
base_branch = "main"
max_diff_chars = 50000
preset = "auto"
secret_action = "redact"

[openai]
model = "gpt-4o"
max_tokens = 500
temperature = 0.5
stream = true

[claude]
model = "claude-sonnet-4-5-20250929"

[ollama]
model = "llama3.2:latest"
base_url = "http://localhost:11434/v1"
```

Show config:

```bash
gitar config
```

---

# Philosophy

* AI proposes
* Human approves
* Git executes

Never:

* Auto-rewrite history
* Auto-commit without showing a plan
* Hide what Git is doing

---

# Roadmap (Clear Priorities)

1. 🚀 **`gitar plan`** (core product)
2. 🧰 `gitar release`
3. 📝 Narration improvements
4. 🩹 `gitar resolve` maintenance only

---

# One-Line Summary

> **Gitar is an AI-native Git interface that helps you understand your history, plan it, and safely execute it.**

```
