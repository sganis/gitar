# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Gitar is an AI-powered Git assistant written in Rust that generates commit messages, PR descriptions, changelogs, and explanations from Git diffs and history. It supports multiple LLM providers (OpenAI, Claude, Gemini, Groq, Ollama) and includes advanced features like secret detection, smart diff shaping algorithms, and style presets for different programming languages.

## Product Philosophy

Gitar is an **AI-native interface to Git history** with a small, stable command language:

```
gitar plan      # Create/reshape history (default command)
gitar tell      # Understand/communicate history (read-only)
gitar fix       # Repair conflicts
gitar release   # Ship releases
```

**Single-letter shortcuts:** `p`, `t`, `f`, `r` (e.g., `gitar t --commit`)

**Core Principles:**
- Dry-run by default — nothing mutates without `--apply`
- Unified scope flags: `--working`, `--staged`, `--history <REF>`
- No magic behavior or hidden mode switches

All commands share a **context optimization engine** with smart diff shaping, secret detection, language presets, and token budget management.

## Build & Test Commands

```bash
# Build the project
cargo build --release

# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_name

# Check code (fast, no build)
cargo check

# Format code
cargo fmt

# Lint
cargo clippy

# Run the binary (after building)
./target/release/gitar --help

# Run without building (debug mode)
cargo run -- <command>
```

## Development Build Targets

The project supports cross-compilation for multiple platforms (see .github/workflows/ci.yml):
- Linux: x86_64-unknown-linux-gnu, x86_64-unknown-linux-musl
- macOS: aarch64-apple-darwin
- Windows: x86_64-pc-windows-msvc

## Architecture

### Module Structure

```
src/
├── main.rs           # Entry point, command routing
├── lib.rs            # Library entry point (module exports)
├── cli.rs            # CLI argument definitions (clap)
├── client.rs         # Unified LLM client interface
├── config.rs         # Configuration management (~/.gitar.toml)
├── git.rs            # Git operations (diffs, logs, commits)
├── color.rs          # Terminal color/styling utilities
├── prompt.rs         # Interactive TTY input (select, confirm, input)
├── types.rs          # Shared data structures
├── plan.rs           # Plan data structures (Plan, Action)
├── executor.rs       # Execute git commands from plans
├── command/          # Command implementations
│   ├── mod.rs        # Module exports and shared utilities
│   ├── plan/         # Core: multi-commit planning engine (default command)
│   │   ├── mod.rs       # Main plan logic & command entry
│   │   ├── analyze.rs   # Repository state analysis
│   │   ├── group.rs     # LLM-based commit grouping
│   │   ├── editor.rs    # Interactive strategy editing
│   │   └── execute.rs   # Strategy execution (git operations)
│   ├── explain/      # Read-only narration (dispatches to subcommands) - CLI: "tell"
│   ├── commit/       # tell --commit: AI commit message generation
│   ├── pr/           # tell --pr: PR description generation
│   ├── changelog/    # tell --changelog: Release notes generation
│   ├── history/      # tell --history: Commit range description
│   ├── fix/          # Merge conflict resolution
│   │   ├── mod.rs       # Main fix logic
│   │   ├── parser.rs    # Parse conflict markers
│   │   ├── heuristic.rs # Heuristic conflict resolution
│   │   ├── llm.rs       # LLM-based resolution
│   │   ├── diff_preview.rs  # Preview resolution diffs
│   │   └── git_helper.rs    # Git operations for conflicts
│   ├── release/      # Release workflow (version, changelog, tag)
│   │   ├── mod.rs       # Main release orchestration
│   │   ├── version.rs   # Version file detection & updates
│   │   └── tag.rs       # Git tag operations
│   ├── squash/       # Squash commits with AI message
│   ├── rewrite/      # Rewrite commit history with AI messages
│   ├── diff/         # Debug diff algorithms
│   ├── hook/         # Git hook installation
│   ├── init/         # Create/update ~/.gitar.toml (--show displays config)
│   └── models/       # List available models
├── util/             # Shared utilities
│   ├── mod.rs        # Module exports
│   ├── diff.rs       # Smart diff utilities (used by commands)
│   └── file.rs       # File system utilities (path handling, read/write)
├── context/          # Context management (LLM & repository)
│   ├── mod.rs        # Module exports
│   ├── algo/         # Diff shaping algorithms (1-4)
│   │   ├── mod.rs       # Algorithm dispatch and coordination
│   │   ├── full.rs      # Algorithm 1: Full diff
│   │   ├── file.rs      # Algorithm 2: Selective files
│   │   ├── hunk.rs      # Algorithm 3: Selective hunks
│   │   └── semantic.rs  # Algorithm 4: Semantic JSON (default)
│   ├── diff.rs       # Diff processing and context building
│   ├── secret.rs     # Secret detection and redaction
│   ├── preset.rs     # Language-specific commit styles
│   ├── template.rs   # System prompt templates
│   └── repo.rs       # Repository context (repo root, user/project context)
└── provider/         # LLM provider implementations
    ├── openai.rs     # OpenAI & compatible APIs
    ├── claude.rs     # Anthropic Claude
    ├── gemini.rs     # Google Gemini
    └── retry.rs      # Exponential backoff retry logic
```

### Key Design Patterns

**Skill-First Architecture**
- Most operations are implemented as deterministic, reusable **skills** — structured pipelines that take diffs, files, or commit ranges, call the LLM only when needed, and validate results
- For higher-level tasks requiring planning and multi-step reasoning, lightweight **agents** orchestrate multiple skills in a controlled, auditable way

**1. Command Flow**
- CLI parsing (clap) → Config resolution → Git operations → Diff shaping → Secret scanning → LLM call → Output
- Most commands follow this pipeline: `cmd_<name>` in `command/` module

**2. Config Resolution Cascade**
- CLI args → Environment variables → ~/.gitar.toml → Provider defaults
- `ResolvedConfig` in config.rs merges all sources with proper precedence

**3. Provider Abstraction**
- `LlmClient` in client.rs provides unified interface
- Provider-specific implementations in provider/ handle API differences
- Automatic retry with exponential backoff (retry.rs) for transient failures

**4. Diff Shaping Algorithms** (context/algo/)
- Algorithm 1: Full diff (truncate only) — full.rs
- Algorithm 2: Selective files by priority — file.rs
- Algorithm 3: Selective hunks by importance score — hunk.rs
- Algorithm 4: Semantic JSON (default, most token-efficient) — semantic.rs
- All algorithms are pure functions: raw diff in, shaped diff out

**5. Secret Detection** (context/secret.rs)
- Regex-based detection of API keys, tokens, private keys, passwords
- Three actions: redact (default), warn, block
- Runs before any data is sent to LLM
- Detection patterns cover major providers (OpenAI, Anthropic, AWS, GitHub, etc.)

**6. Execution Infrastructure**
- `plan.rs`: Data structures for representing execution plans (Plan, Action enum)
- `executor.rs`: Executes git commands from plans with dry-run support
- `context/repo.rs`: Repository context/state management (repo root detection, home dir, user/project context loading)
- Used by new commands (split, run, resolve) for safe, reviewable git operations

### Important Implementation Details

**Streaming vs Non-Streaming**
- OpenAI and Claude support streaming responses via Server-Sent Events (SSE)
- Gemini currently implemented without streaming
- Streaming controlled by config.stream or --stream flag

**Git Operations** (git.rs)
- All git operations use `std::process::Command` to shell out to git CLI
- Three helper functions:
  - `run_git()`: Returns stdout or fails
  - `run_git_status()`: Returns (stdout, stderr, success) without failing
  - `run_git_optional()`: Returns Option<String> for commands where failure is expected
- EXCLUDE_PATTERNS filter out lockfiles, vendored code, minified files

**Style Presets** (context/preset.rs)
- Auto-detected from project files (Cargo.toml → Rust, package.json → JS, etc.)
- Can be overridden via CLI (--preset) or config (preset = "rust")
- Provides language-specific hints to LLM for commit message style

**Configuration**
- Single config file: ~/.gitar.toml
- Provider-specific sections: [openai], [claude], [gemini], etc.
- Global defaults: default_provider, base_branch, max_diff_chars, preset, secret_action
- API keys read from environment variables (OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.)

**Proxy Support**
- Respects ALL_PROXY environment variable
- Supports HTTP and SOCKS5 proxies (socks5h:// for DNS through proxy)
- Useful for corporate/air-gapped environments via SSH tunnel

**Windows Path Handling**
- Git on Windows returns paths with forward slashes (/), not backslashes (\)
- Always normalize paths when comparing or matching file patterns
- Use Path::new() and to_string_lossy() for cross-platform compatibility

## Testing

Tests are in `tests/` directory:
- `cli.rs`: Core integration tests using assert_cmd
- `cli_parse.rs`: CLI argument parsing tests
- `commands.rs`: Command-specific integration tests
- `diff.rs`: Unit tests for diff shaping algorithms
- `resolve.rs`: Integration tests for conflict resolution

When adding new commands, add corresponding tests in tests/cli.rs using the pattern:
```rust
#[test]
fn test_command_name() {
    Command::new(env!("CARGO_BIN_EXE_gitar"))
        .args(&["command", "args"])
        .assert()
        .success();
}
```

## Command Reference

**Primary Commands (Product Layer)**

| Command | Alias | Purpose | Mutates? |
|---------|-------|---------|----------|
| `gitar plan` | `p` | Multi-commit planning from changes or history | Only with `--apply` |
| `gitar tell` | `t` | Read-only narration (commit, pr, changelog, history, explain) | No |
| `gitar fix` | `f` | Merge conflict resolution | Only with `--apply` |
| `gitar release` | `r` | Version bump, changelog, tag creation | Only with `--apply` |

**Plan Command** (command/plan/) — Default when running `gitar` with no args
- Scope flags: `--working`, `--staged`, `--history <REF>`
- `--apply` to execute, `-i` for interactive editing
- Architecture: analyze.rs → group.rs → editor.rs → execute.rs

**Tell Subcommands** (gitar tell --<selector>)
- `--commit` — Generate AI commit message and commit
- `--pr [BASE]` — Generate PR description
- `--changelog [REF]` — Generate release notes
- `--history [REF]` — Describe commit range
- `--explain` — Plain English explanation for stakeholders (default)

**Fix Command** (command/fix/)
- Three-tier resolution: heuristics → per-region LLM → full-file LLM
- Safety checks: no markers remain, no unmerged files remain

**Utility Commands**
- `squash <N|REF>` — Squash commits with AI message
- `rewrite <N|REF>` — Rewrite commit messages
- `release` — Version bump + changelog + tag
- `init` (with `--show` to display resolved config), `models`, `hook install|uninstall`, `diff`

## Common Development Patterns

**Adding a New Command**
1. Add command variant to `Commands` enum in cli.rs
2. Create new subdirectory: `command/<name>/`
3. Implement `cmd_<name>` function in `command/<name>/mod.rs`
4. For complex commands, split logic into submodules within the command directory
5. Export from `command/mod.rs`: `pub use <name>::cmd_<name>;`
6. Add route in main.rs match statement
7. Add tests in tests/cli.rs

**Command Structure Guidelines**
- Simple commands: Single `command/<name>/mod.rs` file
- Complex commands: Multiple submodules (see `plan/` and `fix/` as examples)
  - `mod.rs`: Main command entry point and orchestration
  - Submodules: Logical separation (analyze, execute, editor, etc.)
- Keep command modules focused and under 500 lines per file
- Re-export utilities from `util/` module when shared across commands

**Adding a New Provider**
1. Add provider module in provider/
2. Implement `chat()` and `list_models()` functions
3. Add provider detection in client.rs (is_<provider>)
4. Add provider URL constant in config.rs
5. Update normalize_provider() and default_model_for_provider()

**Modifying Diff Algorithms**
- Each algorithm has its own module in context/algo/
- Algorithm dispatch in context/algo/mod.rs
- Keep algorithms pure (no I/O, no git calls)
- Add new algorithm module and update dispatch in shape_diff()
- Add comparison logic to compare_algorithms() for --compare flag

## Key Dependencies

- **clap**: CLI argument parsing with derive macros
- **reqwest**: HTTP client with JSON, SOCKS, and streaming support
- **tokio**: Async runtime (full features)
- **serde/serde_json**: Serialization for API requests/responses
- **anyhow**: Error handling with context
- **regex**: Secret detection patterns
- **toml**: Config file parsing
- **dialoguer**: Interactive prompts (select, confirm, input)

## Code Style

- Use Result<T> with anyhow for error handling
- Keep command functions async (use tokio runtime)
- Prefer explicit error messages with context
- Use const for string literals and configuration values
- Module-level comments explain purpose and design
- Function-level comments only where logic is non-obvious

# Rust Coding Standards 

## 1. File Headers
- Every file MUST start with its full path relative to the project root as a comment.
- Example: `// src/main.rs` or `// src/auth/mod.rs`.

## 2. File Size & Token Management
To prevent "lazy coding" and context drift, maintain a "Goldilocks Zone" for file length:
- **Optimal Total Length:** 300–500 lines.
- **Hard Limit:** 600 lines. 
- **Component Budget:**
    - Logic: 150–250 lines.
    - Unit Tests (in-file): 100–200 lines.

## 3. Test Coverage & Quality
- **Target Coverage:** Aim for **80% code coverage**.
- **Priority:** Focus on the "Happy Path," complex edge cases, and error handling.
- **Efficiency:** Do not test trivial boilerplate. If testing pushes the file over 500 lines, prioritize core logic and move extra tests to a separate file.

## 4. File & Folder Naming Conventions
- **Singular Only:** Always use singular names (e.g., `user`, `model`).
- **Short Over Long:** Choose the shortest descriptive name possible (e.g., `auth`).
- **All Lowercase:** Never use uppercase letters in paths or filenames.
- **Single word:** Avoid 2 words like app_log or applog unless is needed and short. Log or logger is better. Minimize underscores (`_`).

## 5. Refactoring Triggers
- If a request would push a file beyond 500 lines, you MUST propose a refactor plan to split the code into sub-modules (`mod`) before implementing the new feature.
- Use `cargo check` frequently. If compilation exceeds 3 seconds, simplify module complexity.

## 6. Interaction Style
- Never use `// ... rest of code stays the same` comments. Rewrite the full file.
- If you hit a "Loop of Shame" (fixing the same compiler error 3+ times), stop and propose a simplification immediately.
