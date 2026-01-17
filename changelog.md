# v1.0.6

## Features
- Added intelligent diff selection with file-aware and hunk-level semantic algorithms for improved LLM context optimization.
- Introduced new diff command and smart diff algorithms, documented in README.
- Refactored CLI into a modular command structure with subcommands for changelog, commit, diff, explain, history, and admin hook management.

## Fixes
- Removed deprecated admin command module and migrated hook/config handling to modern CLI structure.

## Improvements
- CLI now defaults to the semantic diff algorithm for more meaningful comparisons.
- Added per-file hunk limit of 3 in diff algorithm for better output readability.
- Clarified and reorganized README:
  - Updated hook installation and uninstall instructions.
  - Enhanced semantic JSON mode description.
  - Moved Git Hook section to follow proxy configuration instructions.
  - Replaced `GITAR_PROXY` with `ALL_PROXY` and added SSH tunnel usage examples.
  - Capitalized project name references for consistency.
  - Removed Rust justification section for brevity.

## Breaking Changes
- Deprecated admin command module removed; CLI subcommands updated accordingly.

## Infrastructure
- Adjusted related tests to align with new CLI structure and default diff behavior.


# v1.0.5

## Features

- Add streaming support for chat responses with --stream CLI flag for OpenAI, Claude, and Gemini providers
- Add git hook command for automatic commit message generation during git commit operations
- Add cross-platform git hook installation with improved error handling and security guidance

## Improvements

- Refactor configuration to support explicit LLM provider sections with improved resolution logic
- Reorganize LLM provider modules into dedicated providers directory for better code organization
- Refactor main.rs into modular architecture by extracting API clients, config, Git utilities, and prompts into separate modules
- Preserve existing default provider in init command
- Disable streaming for commit messages to ensure clean git commit output
- Unify git hook script to use single cross-platform shell version
- Update documentation with improved installation instructions and git hook usage

## Infrastructure

- Bump version to 1.0.5
- Remove unused provider getter method
- Rename unused stream variable to follow Rust naming conventions


# v1.0.4

## Features
- Added Google Gemini provider support and provider recommendations.
- Added provider selection via CLI flag with URL resolution for OpenAI, Claude, Gemini, Groq, and Ollama.

## Improvements
- Expanded and revamped README with detailed setup, quick start, CLI usage, configuration examples, and model recommendations.
- Enhanced changelog generation and documentation for new provider support.
- Improved configuration resolution and added comprehensive provider option tests.
- Updated default Gemini model to 2.5-flash.


# v1.0.3

## Features

- Add support for Groq API integration with dedicated GROQ_API_KEY environment variable
- Add fallback logic for detached HEAD states in git branch detection
- Environment variable fallback: GROQ_API_KEY now falls back to OPENAI_API_KEY if not set

## Improvements

- Make get_current_branch function public for external use
- Update README branding with guitar emoji in title
- Add SSH tunnel proxy documentation for Groq API usage


# v1.0.2

## Features

- Add Claude API support with multi-provider architecture, including automatic provider detection based on model names and environment variables (ANTHROPIC_API_KEY, OPENAI_API_KEY)
- Add --to flag for range-based operations across all commands supporting commit history and diff analysis, enabling flexible commit range specifications
- Add date filtering capabilities with --since and --until flags to the Explain command for time-based commit analysis
- Add SOCKS proxy support through reqwest dependency upgrade
- Add reasoning model handling with automatic detection and appropriate parameter selection (max_completion_tokens vs max_tokens)
- Add unified LLM interface supporting multiple providers (OpenAI, Anthropic, Gemini)
- Add comprehensive test coverage for Claude API integration including serialization, detection, configuration, and model validation

## Improvements

- Switch default model from gpt-4o-mini to gpt-5-chat-latest for improved performance
- Improve diff range selection when working on base branch
- Make token and temperature fields optional in API requests to support different model requirements
- Add retry logic with model marking for specific API errors
- Add default values for CLI options to improve user experience
- Update commit message format prompts to specify single-line responses for better consistency
- Enhance user prompts with clearer instructions throughout the application
- Extract send_chat_request helper function for better code organization
- Derive Clone trait for ChatMessage to enable better message handling

## Infrastructure

- Add comprehensive documentation for Anthropic Claude support, Rust rationale, and improved configuration management
- Update dependencies to latest versions including dirs (6.0.0), dirs-sys (0.5.0), redox_users (0.5.2), and windows-sys
- Remove outdated windows-targets and related packages
- Add build status badge to README
- Update pricing category labels for clarity in code comments
- Remove unused requirements.txt file
- Remove prompt debug output for cleaner execution

## Fixes

- Fix Cargo.toml configuration issues
- Fix tests to use gpt-5-chat-latest as default model
- Verify optional request fields are properly omitted during serialization
- Update tests to handle new model defaults and API behavior

# v1.0.1
## Features

- Add interactive AI-generated commit message feature with options to edit, regenerate, and push changes
- Add AI-assisted Git utilities with command-line interface for enhanced Git workflows
- Add GitHub release step in CI workflow with automatic artifact publishing
- Add dynamic versioning to artifact naming in CI workflow
- Add unit testing module to main.rs for improved code quality

## Improvements

- Enhance CLI and refactor code for improved functionality and clarity
- Expand README with detailed usage instructions and examples
- Update toml dependency to version 0.9.8
- Refactor code for improved AI integration with new dependencies
- Remove outdated comments from tests module
- Remove author section from README.md for cleaner documentation

## Infrastructure

- Add CI/CD workflow for building and testing Rust project on multiple platforms
- Update CI workflow to trigger on main branch
- Fix CI workflow by removing redundant working-directory paths
- Improve artifact naming with version information

## Breaking Changes

- Rename project from "gitai" to "gitan"
- Bump version to 1.0.0

