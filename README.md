[![Build status](https://github.com/sganis/gitar/actions/workflows/ci.yml/badge.svg)](https://github.com/sganis/gitar/actions)

# 🎸 Gitar

**Gitar** is an AI-native Git interface that replaces manual Git workflows with intelligent, AI-assisted operations. Instead of memorizing Git commands and manually staging, splitting, and explaining changes, you ask Gitar to **analyze, plan, and safely execute**.

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

Gitar organizes Git workflows into **four layers**:

### 🚀 Plan (Core Product)
AI-native commit planning engine that analyzes your changes and proposes optimal commit structure:
- **Multi-commit planning** — Group changes into logical, reviewable commits
- **Interactive editing** — Review, reorder, merge, or split commits before execution
- **Multiple modes** — Working tree, staged changes, or historical commits
- **Safe execution** — Dry-run by default, explicit `--apply` to execute

### 📝 Narrate (Read-only)
Explain and communicate Git state without modifying history:
- **Commit messages** — Generate high-quality messages for staged/unstaged changes
- **PR descriptions** — Auto-generate pull request descriptions from branch diffs
- **Changelogs** — Create release notes from commit ranges
- **Explanations** — Translate technical changes into plain English
- **Version analysis** — Suggest semantic version bumps

### 🧰 Release (Guided Workflow)
Safe, deterministic release automation:
- **Version management** — Analyze commits and suggest version bumps
- **Changelog generation** — Auto-generate release notes
- **Version file updates** — Update Cargo.toml, package.json, etc.
- **Git tagging** — Create release commits and tags (never auto-pushes)

### 🩹 Resolve (Conflict Resolution)
Merge conflict resolver with heavy safety rails:
- **Heuristic resolution** — Fast, deterministic conflict resolution
- **LLM fallback** — Per-region and full-file AI-assisted resolution
- **Safety checks** — Ensures no markers remain, no unmerged files
- **Preview mode** — Inspect before applying

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

Configure Gitar interactively or via command line:

```bash
# Interactive setup (recommended for first-time users)
gitar init

# Or specify provider directly
export OPENAI_API_KEY="sk-..."
gitar init --provider openai

# Claude
export ANTHROPIC_API_KEY="sk-ant-..."
gitar init --provider claude

# Gemini
export GEMINI_API_KEY="AIza..."
gitar init --provider gemini

# Groq
export GROQ_API_KEY="gsk_..."
gitar init --provider groq

# Ollama (local, no API key needed)
gitar init --provider ollama
```

Then start using Gitar:

```bash
# The core workflow: AI-powered commit execution
gitar run                       # Analyze changes and create multi-commit strategy
gitar run --apply               # Execute the strategy

# Traditional workflow: generate commit messages
gitar commit                    # Create commit with AI-generated message
```

---

## Usage

Gitar has four conceptual workflows:

### 1. 🚀 Run (Core Workflow) — Smart Commit Execution

AI-powered commit strategy and history shaping:

```bash
gitar run                       # Auto-detect changes and create multi-commit strategy
gitar run --apply               # Execute the strategy after review
gitar run -i                    # Interactive mode (edit, reorder, merge commits)

gitar run --mode working        # Strategy for working tree changes
gitar run --mode staged         # Strategy for staged changes
gitar run --mode history --from v1.0.0  # Strategy for historical commits
```

### 2. 📝 Narrate (Read-only) — Understand & Communicate

Generate messages, descriptions, and explanations without modifying history:

```bash
# Commit messages
gitar commit                    # Interactive commit with AI message
gitar commit -a -p              # Stage all, commit, push
gitar staged                    # Message for staged changes
gitar unstaged                  # Message for unstaged changes

# Communication
gitar pr                        # PR description
gitar pr main                   # PR against specific branch
gitar changelog v1.0.0          # Release notes since tag
gitar explain                   # Explain changes in plain English
gitar explain --staged          # Explain staged changes

# Analysis
gitar history v1.0.0            # Generate messages for commit range
gitar version                   # Suggest semantic version bump
```

### 3. 🧰 Release — Guided Release Workflow

Safe, deterministic release automation:

```bash
gitar release                   # Dry-run: preview version bump and changelog
gitar release --apply           # Execute release (version, changelog, tag)
gitar release --skip-changelog  # Skip changelog generation
gitar release --from v1.0.0     # Specify base version
```

### 4. 🩹 Resolve — Merge Conflict Resolution

AI-assisted conflict resolution with safety checks:

```bash
gitar resolve                   # Detect conflicts and propose resolutions
gitar resolve --apply           # Apply resolutions and stage files
gitar resolve --apply --yes     # Non-interactive mode
```

### Utilities

```bash
gitar hook install              # Install git commit hook
gitar models                    # List available models (when supported)
gitar diff --compare            # Compare smart diff algorithms side-by-side
gitar config                    # Show resolved configuration
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


### Uninstall

Run **gitar hook uninstall** or simply delete **.git/hooks/prepare-commit-msg**


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

## Advanced Commands

### Run Command

The `run` command analyzes your repository state and creates an intelligent multi-commit strategy:

```bash
gitar run                       # Auto-detect changes and create strategy
gitar run --mode working        # Strategy for working tree changes
gitar run --mode staged         # Strategy for staged changes
gitar run --mode history --from v1.0.0  # Strategy for historical commits

gitar run -i                    # Interactive mode (default)
gitar run --apply               # Execute strategy after approval
```

**Features:**
- AI-powered grouping of changes into logical commits
- Interactive editor to review, reorder, merge, or split commits
- Safe execution with dry-run by default
- Multiple analysis modes (auto, working, staged, history)

### Resolve Command

AI-assisted merge conflict resolution with safety-first approach:

```bash
gitar resolve                   # Inspect conflicts and propose resolutions
gitar resolve --apply           # Apply resolutions and stage files
gitar resolve --apply --yes     # Non-interactive mode
```

**Resolution strategy:**
1. Heuristic-based resolution (fast, deterministic)
2. Per-region LLM resolution (precise, context-aware)
3. Full-file LLM fallback (comprehensive)

**Safety checks:**
- Ensures no conflict markers remain
- Verifies no unmerged files
- Preview before applying

### Release Command

Guided release workflow with version management:

```bash
gitar release                   # Dry-run: preview version bump and changelog
gitar release --apply           # Execute release (commit, tag, but never push)
gitar release --skip-changelog  # Skip changelog generation
gitar release --from v1.0.0     # Specify base version
```

**What it does:**
- Analyzes commits since last tag
- Suggests semantic version bump
- Generates changelog
- Updates version files (Cargo.toml, package.json, etc.)
- Creates git commit and tag

---

## License

MIT
