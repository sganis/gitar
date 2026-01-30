# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Gitar is an AI-powered Git assistant written in Rust that generates commit messages, PR descriptions, changelogs, and explanations from Git diffs and history. It supports multiple LLM providers (OpenAI, Claude, Gemini, Groq, Ollama) and includes advanced features like secret detection, smart diff shaping algorithms, and style presets for different programming languages.

## Product Philosophy

Gitar is an **AI-native interface to Git history** with four core commands:

| Command | Alias | Purpose | Mutates? |
|---------|-------|---------|----------|
| `gitar plan` | `p` | Create/reshape history (default) | Only with `--apply` |
| `gitar tell` | `t` | Understand/communicate history | No (read-only) |
| `gitar fix` | `f` | Repair conflicts | Only with `--apply` |
| `gitar release` | `r` | Ship releases | Only with `--apply` |

**Core Principles:**
- Dry-run by default — nothing mutates without `--apply`
- Unified scope flags: `--working`, `--staged`, `--history <REF>`

## Build & Test Commands

```bash
cargo build --release     # Build
cargo test                # Run all tests
cargo test test_name      # Run specific test
cargo test -- --nocapture # Tests with output
cargo check               # Fast syntax check
cargo fmt                 # Format code
cargo clippy              # Lint
cargo run -- <command>    # Run without building
```

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
│   │   ├── scoring.rs   # Deterministic plan quality scoring
│   │   ├── model.rs     # Data structures (PlanCandidate, PlanScore, etc.)
│   │   ├── prompt.rs    # LLM prompt construction & response parsing
│   │   ├── context.rs   # Build planning context from analysis
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
    ├── anthropic.rs  # Anthropic Claude API
    ├── claudecode.rs # Claude via Max subscription (CLI subprocess)
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
- Provider-specific sections: [openai], [anthropic], [gemini], [claudecode], etc.
- Global defaults: default_provider, base_branch, max_diff_chars, preset, secret_action
- API keys read from environment variables (OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.)
- claudecode provider uses browser OAuth (no API key needed)
- Release config: `[release]` section for custom version file locations

**Custom Version Files (Release Command)**
- Configure non-standard version files in `[release]` section
- Single file: `version_file = "path"` with `version_json_path` for JSON
- Multiple files: `version_files = [{ file, json_path }]` array
- Custom patterns: `pattern` (regex) + `template` (replacement) for non-JSON

**Proxy Support**
- Respects GITAR_PROXY environment variable
- Supports HTTP and SOCKS5 proxies (socks5h:// for DNS through proxy)
- Useful for corporate/air-gapped environments via SSH tunnel

**Windows Path Handling**
- Git on Windows returns paths with forward slashes (/), not backslashes (\)
- Always normalize paths when comparing or matching file patterns
- Use Path::new() and to_string_lossy() for cross-platform compatibility

## Testing

Tests are in `tests/` directory using `assert_cmd` for integration testing:
- `cli.rs`: Core integration tests
- `cli_parse.rs`: CLI argument parsing tests
- `commands.rs`: Command-specific integration tests
- `diff.rs`: Unit tests for diff shaping algorithms
- `resolve.rs`: Integration tests for conflict resolution

Use `serial_test` for tests that require isolation (git state, file system).

When adding new commands, add tests in tests/cli.rs:
```rust
#[test]
fn test_command_name() {
    Command::new(env!("CARGO_BIN_EXE_gitar"))
        .args(&["command", "args"])
        .assert()
        .success();
}
```

## Command Details

**Plan Command** (command/plan/) — Default when running `gitar` with no args
- Scope flags: `--working`, `--staged`, `--history <REF>`
- `--apply` to execute, `-i` for interactive editing
- Flow: analyze.rs → group.rs → scoring.rs → editor.rs → execute.rs

### Plan Scoring System (command/plan/scoring.rs)

The plan command uses a **deterministic scoring algorithm** to evaluate LLM-proposed commit groupings. This acts as a guardrail to catch when the AI makes poor grouping decisions.

**How It Works:**
1. LLM analyzes files and proposes commit groups
2. Deterministic scorer validates the proposal against commit hygiene rules
3. Score is displayed to user with detailed breakdown
4. User can accept, regenerate, or manually edit

**Score Calculation:**
- Starts at **100** (baseline)
- Bonuses add points for good practices
- Penalties subtract points for violations
- Score ≥ 50 with no violations = "acceptable"
- Otherwise = "needs improvement"

**Penalties (negative points):**

| Points | Constant | Rule |
|--------|----------|------|
| -30 | `PENALTY_MISSING_FILES` | Files not assigned to any group |
| -25 | `PENALTY_EXCEEDS_SIZE` | Group exceeds max files (15) or total groups exceeds max (8) |
| -20 | `PENALTY_MIX_FORMAT_FUNCTIONAL` | Mixing formatting-only changes with functional changes |
| -15 | `PENALTY_MIX_DOCS_SRC` | Mixing documentation with source code in same commit |
| -10 | `PENALTY_MIX_RENAME_FEATURE` | Mixing file renames with feature changes |
| -8 | `PENALTY_MANY_ROOTS` | Group touches >3 unrelated top-level directories |
| -5 | (inline) | More than 6 groups in the plan |

**Bonuses (positive points):**

| Points | Constant | Rule |
|--------|----------|------|
| +10 | `BONUS_SINGLE_MODULE` | All files in group are in the same directory (cohesive) |
| +8 | `BONUS_TESTS_WITH_SRC` | Tests grouped with their related source files |
| +5 | `BONUS_CLEAR_RATIONALE` | LLM provided explanation >20 chars |
| +5 | (inline) | High confidence (>0.8) from LLM |
| +3 | (inline) | Risk properly escalated (high-risk groups flagged) |

**Example Output:**
```
Score: 151 (needs improvement)

Reasons:
  + 8: Group 1 pairs tests with related source
  + 10: Group 2 has good single-module cohesion
  + 5: Clear rationale provided
  - 5: Many groups (8)
  - 25: Group 5 exceeds max files (17 > 15)

Violations:
  - Group 5 has 17 files (max 15)
```

**Constraints (configurable in PlanConstraints):**
- `max_files_per_group`: 15 (default)
- `max_groups`: 8 (default)
- `separate_formatting`: true — formatting changes should be in own commit
- `separate_docs`: true — docs should be separate from code
- `separate_renames`: true — renames should be separate from features
- `keep_tests_with_src`: true — tests should stay with related source

**Design Rationale:**
The scoring system enforces commit hygiene best practices:
- **Atomic commits**: Each commit should do one thing
- **Reviewability**: Commits shouldn't be too large to review
- **Bisectability**: Clean history helps `git bisect`
- **Clarity**: Separating concerns makes history readable

**Tell Subcommands** (gitar tell --<selector>)
- `--commit` — Generate AI commit message and commit
- `--pr [BASE]` — Generate PR description
- `--changelog [REF]` — Generate release notes
- `--history [REF]` — Describe commit range
- `--explain` — Plain English explanation (default)

**Fix Command** (command/fix/)
- Three-tier resolution: heuristics → per-region LLM → full-file LLM
- Safety checks: no markers remain, no unmerged files remain

**Utility Commands**
- `squash <N|REF>` — Squash commits with AI message
- `rewrite <N|REF>` — Rewrite commit messages
- `release` — Version bump + changelog + tag
- `init --show` — Display resolved config

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

## Code Style

- Use `Result<T>` with `anyhow` for error handling
- Keep command functions async (tokio runtime)
- Prefer explicit error messages with `.context()`
- Use `const` for string literals and configuration values
- Module-level comments explain purpose; function-level only where non-obvious