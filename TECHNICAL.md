# Gitar Technical Documentation

This document provides detailed technical explanations of how each gitar command works internally, including data flow, algorithms, and implementation details.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Configuration System](#configuration-system)
3. [LLM Client](#llm-client)
4. [Context System](#context-system)
5. [Commands](#commands)
   - [plan](#plan-command)
   - [tell](#tell-command)
   - [fix](#fix-command)
   - [release](#release-command)
   - [squash](#squash-command)
   - [rewrite](#rewrite-command)
   - [init](#init-command)
   - [hook](#hook-command)
   - [diff](#diff-command)
   - [models](#models-command)

---

## Architecture Overview

Gitar follows a skill-first architecture where most operations are implemented as deterministic, reusable pipelines:

```
User Input (CLI)
    |
    v
Config Resolution (system -> user -> CLI args)
    |
    v
Git Operations (diff, log, status)
    |
    v
Diff Shaping (Algorithm 1-4)
    |
    v
Secret Scanning (detect, redact/warn/block)
    |
    v
Context Loading (project + user context)
    |
    v
Prompt Construction (system + preset + context + diff)
    |
    v
LLM Call (streaming or non-streaming)
    |
    v
Interactive Loop (accept/regenerate/edit/cancel)
    |
    v
Git Execution (only with --apply)
    |
    v
Output
```

### Key Design Principles

1. **Dry-run by default**: Nothing mutates without `--apply`
2. **Unified scope flags**: `--working`, `--staged`, `--history <REF>`
3. **Provider abstraction**: Single `LlmClient` interface for all providers
4. **Context-aware prompts**: Language presets + project/user conventions
5. **Secret safety**: All diffs scanned before LLM submission

---

## Configuration System

**File**: `src/config.rs`

### Configuration Cascade

Configuration is resolved with the following priority (highest to lowest):

1. CLI arguments (`--model`, `--provider`, etc.)
2. Environment variables (`OPENAI_API_KEY`, etc.)
3. User config file (`~/.gitar.toml`)
4. Provider defaults

### Config Structure

```toml
# ~/.gitar.toml
default_provider = "openai"
base_branch = "main"
max_diff_chars = 50000
preset = "auto"
secret_action = "redact"
auto_apply = false

[openai]
model = "gpt-4.1-2025-04-14"
max_tokens = 4096
temperature = 0.7

[anthropic]
model = "claude-sonnet-4-5-20250514"

[gemini]
model = "gemini-2.5-flash"

[claudecode]
model = "sonnet"  # uses Claude Max subscription via CLI
```

### Default Models by Provider

| Provider | Default Model |
|----------|---------------|
| OpenAI | `gpt-4.1-2025-04-14` |
| Anthropic | `claude-sonnet-4-5-20250514` |
| Claude Code | `sonnet` |
| Gemini | `gemini-2.5-flash` |
| Groq | `llama-3.3-70b-versatile` |
| Ollama | `llama3.2:latest` |

### Environment Variables

- `OPENAI_API_KEY` - OpenAI API key
- `ANTHROPIC_API_KEY` - Claude API key
- `GEMINI_API_KEY` / `GOOGLE_API_KEY` - Gemini API key
- `GROQ_API_KEY` - Groq API key
- `GITAR_PROXY` - HTTP/SOCKS5 proxy URL
- `GITAR_CA_FILE` - Custom CA certificate path
- `GITAR_SYSTEM_CONFIG_ENV` - System-wide config file path

---

## LLM Client

**File**: `src/client.rs`

### Provider Detection

The client auto-detects provider from:
1. Explicit `--provider` flag
2. Base URL patterns (anthropic.com -> anthropic, googleapis.com -> gemini)
3. API key prefix (`sk-ant-*` -> anthropic)

### Supported Providers

```rust
LlmClient {
    http: Client,           // reqwest HTTP client
    provider: String,       // openai|anthropic|gemini|groq|ollama|claudecode
    base_url: String,       // API endpoint
    api_key: Option<String>,
    model: String,
    max_tokens: u32,
    temperature: f32,
}
```

Note: `claudecode` provider uses local Claude CLI subprocess instead of HTTP API.

### Key Methods

- `chat(system, user, stream)` - Send message to LLM
- `list_models()` - Fetch available models from provider API

### Network Features

- Custom CA certificates via `GITAR_CA_FILE`
- Proxy support via `GITAR_PROXY` (HTTP and SOCKS5)
- TLS verification (disable with `--insecure`)
- 120-second timeout
- Exponential backoff retry (see `src/provider/retry.rs`)

---

## Context System

**Directory**: `src/context/`

### Diff Shaping Algorithms

**Directory**: `src/context/algo/`

Four algorithms optimize token usage when sending diffs to LLMs:

#### Algorithm 1: Full (`full.rs`)
- Complete git diff
- Truncates only at character limit
- Best for small diffs

#### Algorithm 2: Files (`file.rs`)
- Selects files by priority score
- Excludes low-value files (lockfiles, vendor, etc.)
- Priority scoring:
  - `main.rs`: 100
  - `mod.rs`: 80
  - `.rs`: 70
  - `.ts/.go`: 65
  - `.js`: 60
  - `Cargo.toml`: 50
  - `README`: 40

#### Algorithm 3: Hunks (`hunk.rs`)
- Selects individual diff hunks by importance
- Preserves most significant changes
- Good for large files with mixed changes

#### Algorithm 4: Semantic (Default) (`semantic.rs`)
- JSON intermediate representation
- Most token-efficient
- Structured format for LLM comprehension

### Excluded Patterns

```rust
EXCLUDE_PATTERNS = [
    "Cargo.lock", "package-lock.json", "yarn.lock",
    "node_modules/", "vendor/", "__pycache__/",
    ".min.js", ".min.css", ".bundle.js"
]
```

### Secret Detection

**File**: `src/context/secret.rs`

Scans diffs for sensitive data before LLM submission:

| Severity | Patterns |
|----------|----------|
| High | API keys (`sk-*`, `AIza*`), private keys, AWS keys |
| Medium | Passwords, connection strings, tokens |
| Low | Email addresses, URLs with credentials |

**Actions**:
- `Redact` (default): Replace with `[REDACTED]`
- `Warn`: Print warning but continue
- `Block`: Abort operation

### Preset System

**File**: `src/context/preset.rs`

Auto-detects language from project files:

| File | Preset |
|------|--------|
| `Cargo.toml` | Rust |
| `package.json` | JavaScript |
| `pyproject.toml`, `setup.py` | Python |
| `go.mod` | Go |

Each preset provides:
- Tone hints (formal, casual)
- Domain nouns (crate, module, component)
- Example commit messages
- Anti-patterns to avoid

### Context Files

**File**: `src/context/repo.rs`

Two context files inject conventions into prompts:

1. **User context** (`~/.gitar/gitar.md`): Personal preferences
2. **Project context** (`.gitar/gitar.md`): Repository conventions

Both are Markdown files loaded and injected into LLM system prompts.

---

## Commands

### Plan Command

**Directory**: `src/command/plan/`
**Default command when running `gitar` without arguments**

#### Purpose
Analyze repository changes and create a multi-commit execution plan with LLM-powered grouping.

#### Submodules

| File | Purpose |
|------|---------|
| `mod.rs` | Main entry point and orchestration |
| `analyze.rs` | Repository state analysis |
| `context.rs` | Planning context builder |
| `group.rs` | LLM-based commit grouping |
| `model.rs` | Data structures (PlanCandidate, PlanGroup, PlanScore) |
| `prompt.rs` | LLM prompt templates for multi-candidate output |
| `scoring.rs` | Deterministic plan scorer |
| `editor.rs` | Interactive strategy editing |
| `execute.rs` | Git operations for plan execution |

#### Data Flow

```
1. Analyze repository state (detect_and_analyze)
   - Detect mode: working tree, staged, or history
   - Scan files with git status/diff
   - Categorize files (code, test, doc, config)

2. Build planning context (build_planning_context)
   - Extract file metadata (kind, churn, renamed)
   - Detect signals (large_rename_set, docs_only, etc.)
   - Set constraints (max_files_per_group, max_groups)

3. Generate candidates (LLM call)
   - Request 2-3 alternative grouping strategies
   - Parse JSON response with fallback heuristics

4. Score candidates (score_plan)
   - Penalty: mixing docs/src, format with functional
   - Bonus: single-module cohesion, tests with src
   - Select best candidate

5. Generate commit messages (per group)
   - Get diff for group's files
   - Apply diff shaping + secret detection
   - LLM generates commit message

6. Interactive editing (optional)
   - Move files between commits
   - Edit commit messages
   - Exclude files

7. Execute plan (with --apply)
   - Stage files per group
   - Create commits with messages
```

#### Scoring Heuristics

**Penalties**:
- Mix docs and src: -15 points
- Mix formatting with functional: -20 points
- Mix renames with features: -10 points
- Many unrelated directories: -8 points
- Exceed size limits: -25 points

**Bonuses**:
- Single-module cohesion: +10 points
- Tests paired with src: +8 points
- Clear rationale: +5 points

#### Analysis Modes

| Mode | Flag | Description |
|------|------|-------------|
| Auto | (default) | Detect from repo state |
| Working Tree | `--working` | Unstaged + untracked |
| Staged | `--staged` | Only staged changes |
| History | `--history <REF>` | Commit range |

---

### Tell Command

**Directory**: `src/command/explain/` (dispatcher)
**Subcommands**: `src/command/commit/`, `src/command/pr/`, `src/command/changelog/`, `src/command/history/`

The `tell` command is a dispatcher that routes to specialized subcommands based on selector flags.

#### Selectors

| Flag | Subcommand | Purpose |
|------|------------|---------|
| `--commit` | commit | Generate AI commit message |
| `--pr` | pr | Generate PR description |
| `--changelog` | changelog | Generate release notes |
| `--history` | history | Describe commit range |
| `--explain` | explain | Plain English explanation (default) |

---

#### tell --commit

**File**: `src/command/commit/mod.rs`

##### Purpose
Generate and apply AI-powered commit messages.

##### Data Flow

```
1. Get diff
   - Normal: staged changes (git diff --cached)
   - Amend: HEAD~1..HEAD diff
   - All flag: stage all first

2. Apply diff shaping (algorithm 1-4)

3. Scan for secrets

4. Load context (project + user)

5. Construct prompt
   - System: commit template + preset hints
   - User: shaped diff

6. LLM call (streaming optional)

7. Interactive loop
   - Accept: proceed to commit
   - Regenerate: call LLM again
   - Edit: manual message entry
   - Cancel: abort

8. Execute commit
   - git commit -m "message"
   - Optional: --amend, --push
   - Optional: [AI:model] suffix
```

##### Special Modes

- **Hook mode** (`--write-to`): Write message to file, no commit
- **Silent mode**: Suppress all interactive prompts
- **Amend mode**: Modify last commit

---

#### tell --pr

**File**: `src/command/pr/mod.rs`

##### Purpose
Generate comprehensive PR descriptions.

##### Data Flow

```
1. Determine diff range
   - Staged mode: git diff --cached
   - Normal: branch vs base branch

2. Fetch commits in range (max 20)

3. Get diff stats (git diff --stat)

4. Apply diff shaping + secret scanning

5. Construct prompt
   - Branch names
   - Commit list
   - Stats
   - Shaped diff

6. LLM generates PR description

7. Output (streaming or full)
```

##### Output Format

```markdown
## Summary
Brief overview.

## What Changed
- Key changes

## Why
Motivation.

## Risks
- Issues or "None"

## Testing
- How tested

## Rollout
- Deploy notes
```

---

#### tell --changelog

**File**: `src/command/changelog/mod.rs`

##### Purpose
Generate release notes from commit history.

##### Data Flow

```
1. Resolve reference
   - Explicit --from: use it
   - Default: latest git tag
   - Fallback: last 50 commits

2. Fetch commits in range

3. Build commit list (hash + message)

4. Get combined diff (base^..HEAD)

5. Apply diff shaping + secret scanning

6. LLM generates changelog

7. Output grouped by type:
   - Features
   - Fixes
   - Improvements
   - Breaking Changes
```

---

#### tell --history

**File**: `src/command/history/mod.rs`

##### Purpose
Describe each commit in a range with LLM-generated explanations.

##### Data Flow

```
For each commit in range:
  1. Display header: [i/n] hash | date | author | message
  2. Get commit diff
  3. Apply diff shaping + secret scanning
  4. LLM explains the commit
  5. Display indented explanation
  6. Wait delay (--delay, default 500ms)
```

##### Rate Limiting

Configurable delay between LLM calls prevents rate limiting:
```bash
gitar tell --history v1.0 --delay 1000  # 1 second between calls
```

---

#### tell --explain (Default)

**File**: `src/command/explain/mod.rs`

##### Purpose
Plain-English explanation of changes for non-technical stakeholders.

##### Output Format

```markdown
## What's Changing
Summary.

## User Impact
- Effects

## Risk Level
Low/Medium/High

## Actions
- QA needed
```

---

### Fix Command

**Directory**: `src/command/fix/`

#### Purpose
Resolve merge/rebase/cherry-pick conflicts using AI-powered semantic synthesis.

#### Submodules

| File | Purpose |
|------|---------|
| `mod.rs` | Main orchestration |
| `parser.rs` | Parse conflict markers into regions |
| `heuristic.rs` | Fast deterministic resolution |
| `llm.rs` | AI-powered resolution |
| `diff_preview.rs` | Preview changes before applying |
| `git_helper.rs` | Git operations |

#### Three-Tier Resolution Strategy

```
Tier 1: Heuristics (fast, deterministic)
  - Identical sides -> keep one
  - One side empty -> use other
  - Whitespace-only -> prefer ours

Tier 2: Per-Region LLM
  - Analyze each conflict region
  - Preserve both intents
  - Fallback to Tier 3 on failure

Tier 3: Full-File LLM
  - Send entire file with markers
  - Request complete resolution
```

#### Conflict Parsing

```
<<<<<<< HEAD (ours)
  code from current branch
=======
  code from incoming branch
>>>>>>> feature (theirs)
```

Parser extracts:
- `ours`: Current branch content
- `theirs`: Incoming branch content
- Context: Lines before/after region

#### Data Flow

```
1. Detect conflicts (git ls-files -u)

2. For each conflicted file:
   a. Read file with markers
   b. Parse into regions
   c. Try heuristic resolution
   d. If failed, try per-region LLM
   e. If failed, try full-file LLM

3. Validate resolution
   - No markers remain
   - File is syntactically valid

4. Preview diff (unless --yes)

5. Apply (with --apply)
   - Write resolved content
   - Stage file (git add)

6. Verify no unmerged entries remain
```

#### Environment Variables

- `GITAR_RESOLVE_ACCEPT=ours|theirs|both`: Auto-accept mode

---

### Release Command

**Directory**: `src/command/release/`

#### Purpose
Automate version bumps, changelog generation, and git tagging.

#### Submodules

| File | Purpose |
|------|---------|
| `mod.rs` | Main orchestration |
| `version.rs` | Detect and update version files |
| `tag.rs` | Git tag operations |

#### Supported Version Files

| File | Format |
|------|--------|
| `Cargo.toml` | `version = "1.2.3"` |
| `package.json` | `"version": "1.2.3"` |
| `pyproject.toml` | `version = "1.2.3"` |

#### Version Bump Strategy

| Strategy | When |
|----------|------|
| `major` | Breaking API changes |
| `minor` | New features (backwards compatible) |
| `patch` | Bug fixes, docs, refactoring |
| `auto` | LLM analyzes diff to decide |

#### Data Flow

```
1. Find starting point
   - Explicit --from
   - Latest git tag
   - Initial commit

2. Collect commits since starting point

3. Determine version bump
   - Auto: LLM analyzes diff
   - Explicit: use provided strategy

4. Detect version files (searches current directory for monorepo support)

5. Compute new version (1.2.3 + minor = 1.3.0)

6. Generate changelog
   - LLM summarizes commits
   - Groups by type

7. Preview changes

8. Execute (with --apply)
   - Update version files
   - Create/prepend CHANGELOG.md
   - Create annotated git tag (v1.3.0)
```

#### LLM Version Analysis

The LLM receives the diff and returns:
```
Recommendation: minor
Reasoning: Added new user authentication feature
Breaking: No
```

---

### Squash Command

**File**: `src/command/squash/mod.rs`

#### Purpose
Combine multiple commits into one with an AI-generated unified message.

#### Target Formats

```bash
gitar squash 3          # Last 3 commits
gitar squash HEAD~5     # From HEAD~5 to HEAD
gitar squash v1.0.0     # From tag to HEAD
```

#### Data Flow

```
1. Parse target (number or ref)
   - Number: HEAD~N
   - Ref: resolve to commit

2. List commits to squash

3. Get combined diff

4. Apply diff shaping + secret scanning

5. LLM generates unified message

6. Interactive confirmation

7. Execute
   - git reset --soft <base>
   - git commit -m "unified message"
```

---

### Rewrite Command

**File**: `src/command/rewrite/mod.rs`

#### Purpose
Interactively regenerate commit messages for a range of commits.

#### Data Flow

```
1. Parse target (same as squash)

2. List commits (oldest first)

3. For each commit:
   a. Show original message
   b. Generate new message via LLM
   c. User chooses:
      - Accept new
      - Keep original
      - Regenerate
      - Edit manually
      - Skip remaining

4. Collect approved messages

5. Execute
   - git reset --soft <base>
   - For each commit:
     - Stage commit's files
     - Commit with new/original message

6. Output force-push instructions
```

#### Warning

Rewriting history requires `git push --force`. The command displays a warning before proceeding.

---

### Init Command

**File**: `src/command/init/mod.rs`

#### Purpose
Interactive configuration setup for gitar.

#### Modes

| Mode | Flag | Description |
|------|------|-------------|
| Setup | (default) | Interactive provider/model selection |
| Show | `--show` | Display resolved configuration |

#### Data Flow (Setup Mode)

```
1. Create context files
   - ~/.gitar/gitar.md (user preferences)
   - .gitar/gitar.md (project conventions)

2. Select provider (interactive)

3. Fetch available models from provider API

4. Select or enter model

5. Save to ~/.gitar.toml
```

#### Context File Templates

**User context** (`~/.gitar/gitar.md`):
```markdown
<!-- Personal preferences for commit messages -->
<!-- Example: "Use imperative mood, be concise" -->
```

**Project context** (`.gitar/gitar.md`):
```markdown
<!-- Repository-specific conventions -->
<!-- Example: "Prefix commits with JIRA ticket" -->
```

---

### Hook Command

**File**: `src/command/hook/mod.rs`

#### Purpose
Install/uninstall git hooks for automatic commit message generation.

#### Hook Script

Installs `prepare-commit-msg` hook:

```bash
#!/bin/sh
# Skip if gitar not in PATH
if ! command -v gitar >/dev/null 2>&1; then
    exit 0
fi

# Skip if message provided via -m, -F, or merge/squash
if [ -n "$COMMIT_SOURCE" ]; then
    exit 0
fi

# Generate message
gitar tell --commit --write-to "$COMMIT_MSG_FILE" --silent
```

#### Safety Checks

- Won't overwrite existing hooks from other tools
- Sets executable permissions (0o755 on Unix)
- Uninstall only removes gitar-created hooks

---

### Diff Command

**File**: `src/command/diff/mod.rs`

#### Purpose
Debug and preview diff shaping algorithms.

#### Modes

| Flag | Description |
|------|-------------|
| `--compare` | Show all 4 algorithms side-by-side |
| `--algo N` | Show specific algorithm output |
| `--stats` | Include git diff --stat header |
| `--stats-only` | Show only stats |

#### Output

```
Algorithm 4 (Semantic):
  Files: 5 (included: 5)
  Chars: 12,345 (output: 8,432)
  Tokens: ~2,108
  Truncated: No

[shaped diff output]
```

---

### Models Command

**File**: `src/command/models/mod.rs`

#### Purpose
List available models from the configured provider.

#### Data Flow

```
1. Call client.list_models()
2. Fetch from provider API
3. Print formatted list
```

---

## Appendix: File Structure

```
src/
├── main.rs           # Entry point, command routing
├── lib.rs            # Library exports
├── cli.rs            # CLI definitions (clap)
├── client.rs         # Unified LLM client
├── config.rs         # Configuration management
├── git.rs            # Git operations
├── plan.rs           # Plan data structures
├── executor.rs       # Execute git commands from plans
├── types.rs          # Shared API types
├── color.rs          # Terminal styling
├── prompt.rs         # Interactive input helpers
├── command/
│   ├── mod.rs        # Command exports
│   ├── plan/         # Multi-commit planning
│   │   ├── mod.rs
│   │   ├── analyze.rs
│   │   ├── context.rs
│   │   ├── group.rs
│   │   ├── model.rs
│   │   ├── prompt.rs
│   │   ├── scoring.rs
│   │   ├── editor.rs
│   │   └── execute.rs
│   ├── commit/       # tell --commit
│   ├── pr/           # tell --pr
│   ├── changelog/    # tell --changelog
│   ├── history/      # tell --history
│   ├── explain/      # tell --explain
│   ├── fix/          # Conflict resolution
│   │   ├── mod.rs
│   │   ├── parser.rs
│   │   ├── heuristic.rs
│   │   ├── llm.rs
│   │   ├── diff_preview.rs
│   │   └── git_helper.rs
│   ├── release/      # Version + changelog + tag
│   │   ├── mod.rs
│   │   ├── version.rs
│   │   └── tag.rs
│   ├── squash/
│   ├── rewrite/
│   ├── init/
│   ├── hook/
│   ├── diff/
│   └── models/
├── context/
│   ├── mod.rs
│   ├── algo/         # Diff shaping algorithms
│   │   ├── mod.rs
│   │   ├── full.rs
│   │   ├── file.rs
│   │   ├── hunk.rs
│   │   └── semantic.rs
│   ├── diff.rs
│   ├── secret.rs     # Secret detection
│   ├── preset.rs     # Language presets
│   ├── template.rs   # LLM prompt templates
│   └── repo.rs       # Context file loading
├── provider/
│   ├── openai.rs
│   ├── anthropic.rs
│   ├── claudecode.rs
│   ├── gemini.rs
│   └── retry.rs
└── util/
    ├── mod.rs
    ├── diff.rs
    └── file.rs
```
