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

## 🚀 Automatic Commit Messages (Git Hook)

gitar can **automatically generate commit messages every time you run `git commit`** by installing a Git hook.

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

That’s it. The message is generated automatically.

### What exactly happens?

On `git commit`:

* Git calls the `prepare-commit-msg` hook
* The hook runs:

```bash
gitar commit --write-to .git/COMMIT_EDITMSG --silent
```

* Your editor opens with a **ready-to-use AI commit message**

You can still edit it before saving.

### Windows notes

On Git for Windows:

* Git hooks are executed via Git’s internal shell, not directly by `cmd.exe`
* gitar installs an extensionless `prepare-commit-msg` entrypoint
  
This works transparently even if you use Git from **cmd** or **PowerShell**.

### Uninstall

Run **gitar uninstall** or simply delete **.git/hooks/prepare-commit-msg** 


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

## Smart Diff Algorithms (Context Optimization)

Large diffs can blow up context windows and cost tokens. Gitar can **shape** the diff before sending it to your LLM, using one of four algorithms.

Most commands accept:

```bash
--alg <1..4>
```

### Algorithms

* **1 — Full Diff**
  Sends the raw `git diff` (best fidelity, worst token usage).

* **2 — Selective Files**
  Splits the diff by file, filters out obvious noise (lockfiles / vendored / generated paths), ranks files by importance, and packs whole-file patches until the size limit is hit.

* **3 — Selective Hunks**
  Extracts hunks across files, scores them (structural changes, meaningful additions/removals, etc.), then packs the highest scoring hunks first. Includes a per-file cap so one file can’t dominate.

* **4 — Semantic JSON** *(default)*
  Produces a compact JSON “intermediate representation” with a file summary (path, status, adds/dels, priority) and a top-ranked hunks with short previews. It adaptively shrinks previews / hunk count until it fits the size budget.

### Examples

Use a different algorithm when you know you’re doing a big refactor:

```bash
gitar commit --alg 3
gitar pr --alg 4
gitar changelog v1.0.0 --alg 2
gitar explain --staged --alg 4
```

Debug what will be sent to the model:

```bash
gitar diff --alg 2 --max-chars 15000 --stats
gitar diff --compare
```

---

## Security & Privacy

gitar sends **only what it needs** for the command you run (for example: a diff, a commit range log, or staged changes).

Tips:

* Use **Ollama** for **100% local** inference (no network calls).
* If you must run through restricted networks, use a proxy.
* If you work in sensitive repos, prefer smaller scopes (staged changes, specific ranges).

---

## License

MIT


