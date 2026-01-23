# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Gitar is an AI-powered Git assistant written in Rust that generates commit messages, PR descriptions, changelogs, and explanations from Git diffs and history. It supports multiple LLM providers (OpenAI, Claude, Gemini, Groq, Ollama) and includes advanced features like secret detection, smart diff shaping algorithms, and style presets for different programming languages.

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
- Linux: x86_64-unknown-linux-gnu, x86_64-unknown-linux-musl, aarch64-unknown-linux-gnu
- macOS: aarch64-apple-darwin
- Windows: x86_64-pc-windows-msvc, aarch64-pc-windows-msvc

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
├── types.rs          # Shared data structures
├── plan.rs           # Plan data structures (Plan, Action)
├── executor.rs       # Execute git commands from plans
├── command/          # Command implementations
│   ├── mod.rs        # Module exports and shared utilities
│   ├── commit/mod.rs # Interactive commit with AI message
│   ├── changelog/mod.rs # Generate release notes
│   ├── explain/mod.rs   # Explain changes in plain English
│   ├── history/mod.rs   # Regenerate commit messages for history
│   ├── pr/mod.rs        # Generate PR descriptions
│   ├── version/mod.rs   # Suggest version bumps
│   ├── diff/mod.rs      # Debug diff algorithms
│   ├── hook/mod.rs      # Git hook installation
│   ├── config/mod.rs    # Show resolved configuration
│   ├── init/mod.rs      # Create/update ~/.gitar.toml
│   ├── models/mod.rs    # List available models
│   ├── split/mod.rs     # Split large diffs into logical commits
│   ├── run/             # Interactive commit execution (LLM-powered)
│   │   ├── mod.rs       # Main run logic & command entry
│   │   ├── analyze.rs   # Repository state analysis
│   │   ├── group.rs     # LLM-based commit grouping
│   │   ├── editor.rs    # Interactive strategy editing
│   │   └── execute.rs   # Strategy execution (git operations)
│   └── resolve/         # Merge conflict resolution
│       ├── mod.rs       # Main resolve logic
│       ├── parser.rs    # Parse conflict markers
│       ├── heuristic.rs # Heuristic conflict resolution
│       ├── llm.rs       # LLM-based resolution
│       ├── diff_preview.rs  # Preview resolution diffs
│       └── git_helper.rs    # Git operations for conflicts
├── util/             # Shared utilities
│   ├── mod.rs        # Module exports
│   └── diff.rs       # Smart diff utilities (used by commands)
├── context/          # Context management (LLM & repository)
│   ├── mod.rs        # Module exports
│   ├── algo.rs       # Diff shaping algorithms (1-4)
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

**4. Diff Shaping Algorithms** (context/algo.rs)
- Algorithm 1: Full diff (truncate only)
- Algorithm 2: Selective files by priority
- Algorithm 3: Selective hunks by importance score
- Algorithm 4: Semantic JSON (default, most token-efficient)
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

## Testing

Tests are in `tests/` directory:
- `cli.rs`: Integration tests using assert_cmd
- `diff.rs`: Unit tests for diff shaping algorithms

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

## Important Recent Additions

**New Commands (2024-2025)**
- `run` — LLM-powered interactive commit execution (groups changes into logical commits)
- `resolve` — AI-assisted merge conflict resolution with heuristics + LLM fallback
- `split` — Split large working tree diffs into logical commits (may be superseded by `run`)
- `init` — Initialize ~/.gitar.toml configuration file

**Run Command** (command/run/)
- **Multi-mode analysis**: Auto-detect changes, or target staged/unstaged/history
- **LLM-powered grouping**: Groups changes by semantic intent into logical commits
- **Interactive editing**: Review, reorder, merge, split, or regenerate commit strategies
- **Safe execution**: Dry-run by default, use `--apply` to execute commits
- **Legacy --suggest mode**: Simple state inspection without LLM (shows next actions)

Architecture:
- `analyze.rs`: Detects repository state (AnalysisMode enum: Auto, Staged, WorkingTree, History)
- `group.rs`: Calls LLM to generate logical commit groups from analyzed files
- `editor.rs`: Interactive TUI for reviewing and editing the generated strategy
- `execute.rs`: Executes git operations (add, commit) from approved strategy
- `mod.rs`: Main command orchestration and entry point

**Resolve Command** (command/resolve/)
- Detects and parses merge/rebase/cherry-pick conflicts
- Three-tier resolution strategy: heuristics → per-region LLM → full-file LLM
- Safety checks: no markers remain, no unmerged files remain
- Use `--apply` to write + stage, `--yes` to skip confirmation

**Split Command** (command/split/)
- Analyzes unstaged changes and guides creating a series of focused commits
- Groups changes by type (docs, tests, config) and semantic intent
- Requires interactive confirmation before each commit
- Note: May be superseded by enhanced `run` command with interactive mode

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
- Complex commands: Multiple submodules (see `run/` and `resolve/` as examples)
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
- All algorithm logic in context/algo.rs
- Keep algorithms pure (no I/O, no git calls)
- Update shape_diff() function for new algorithm
- Add comparison logic to compare_algorithms() for --compare flag

## Key Dependencies

- **clap**: CLI argument parsing with derive macros
- **reqwest**: HTTP client with JSON, SOCKS, and streaming support
- **tokio**: Async runtime (full features)
- **serde/serde_json**: Serialization for API requests/responses
- **anyhow**: Error handling with context
- **regex**: Secret detection patterns
- **toml**: Config file parsing

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
