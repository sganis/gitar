## [3.0.0] - 2026-01-28

# Release Notes
## Improvements
*   Enhanced system template consistency by adding strict output formatting rules to the `WEEKLY_SYSTEM` template.
*   Improved configuration management by separating user-writable paths from system configuration within the config module.
*   Streamlined project context by simplifying `gitar` project context and removing extraneous template documentation.

## Infrastructure
*   Updated Rust dependencies to ensure stability and incorporate the latest security patches.

## [2.6.0] - 2026-01-27

# Release Notes
## Features
*   Implemented large file detection and handling in plan execution

## [2.5.0] - 2026-01-27

# Release Notes

## Features

- Added configurable prompt overrides using TOML files.
- Introduced a new weekly highlights command.
- Added initial Gitar project context file with commit and build conventions.
- Refined weekly report templates to include highlight ranking and word count guidance.

## Fixes

- Added CLI tests for tell --weekly flag parsing and selector conflict validation.

## Improvements

- Enhanced context display and template personalization.
- Documented config directory layout and weekly tell flag.
- Switched changelog module to use get_changelog_user accessor.

## Breaking Changes

- Removed legacy gitar.md support.

## Infrastructure

- Updated Rust dependencies.

## [2.5.0] - 2026-01-27

# Release Notes

## Features
- Configurable prompt overrides via TOML configuration.
- Weekly highlights command added, enabling weekly summaries.

## Improvements
- Added CLI tests for tell --weekly flag parsing and selector conflict validation.
- Documentation expanded to cover configuration directory layout and the weekly tell flag.
- Internal refactor to use the get_changelog_user accessor in the changelog module.

## Infrastructure
- Updated Rust dependencies to current versions.

## [2.4.0] - 2026-01-26

# Release Notes
## Features
- Added per-commit approval mode to the plan editor, supporting AcceptAll and AcceptOneByOne actions.
- Introduced a staging helper and improved file status handling in execute_plan for more accurate git operations.

## Improvements
- Added retry logic for git commands in tests to handle index.lock contention.
- Updated Rust dependencies to latest compatible versions.

## [2.3.1] - 2026-01-25

# Release Notes
## Features
- Add FileWithStatus usage in plan editor and execute tests to validate file status handling
- Add architecture overview for agent workflow and kernel execution flow

## Improvements
- Update Rust dependencies

## [2.3.0] - 2026-01-25

# Release Notes
## Features
- Added file status tracking to commit groups for better change analysis and improved JSON repair handling in the plan prompt parser.
- Replaced the Python-based LLM planner with a new Rust implementation, introducing context, model, prompt, and scoring modules.
- Added comprehensive technical documentation and removed outdated planning notes.
- Added planning document to improve commit grouping quality in the gitar plan command.

## Fixes
- Fixed Windows CMD installer to use a temporary script path.
- Fixed CMD installer to create the temp directory before version resolution and corrected errorlevel checks.

## Improvements
- Improved Windows installer reliability by using PowerShell JSON parsing and Expand-Archive with a curl User-Agent header.
- Updated Rust dependencies for compatibility and performance.
- Updated Cargo manifest with revised description, keywords, and reordered dependencies.
- Updated path handling to use the repository root for absolute path resolution across fix, plan, and release commands.

## Infrastructure
- Internal release tags updated (v1.3.2, v1.3.3).

## [2.2.0] - 2026-01-25

# Release Notes

## Fixes
- Fix release command version file detection to use the current directory, improving support for monorepos.

## Improvements
- Add test to verify release command correctly detects Cargo.toml when run from a subdirectory.

## Infrastructure
- Update Rust dependencies.

## [2.1.0] - 2026-01-25

# Release Notes

## Improvements
- Added repository root caching to the git module, improving performance and reliability when operating from subdirectories.
- Ensured the plan command correctly detects changes when run from a subdirectory by adding targeted test coverage.

## Fixes
- Routed fix and git_helper commands through run_git to ensure correct behavior when executed from subdirectories.

## Infrastructure
- Updated Rust dependencies to their latest compatible versions.

## [2.0.0] - 2026-01-25

# Release Notes

## Features
- Enterprise configuration support with environment variables for proxy, custom CA certificates, and cascading config precedence. Adds GITAR_CA_FILE for custom CA trust and GITAR_SYSTEM_CONFIG for system-wide configuration merged with user config.
- Centralized HTTP client configuration to ensure consistent proxy and TLS behavior across commands.
- Improved path handling so fix, plan, and release commands consistently operate from the repository root.

## Fixes
- Windows CMD installation now uses a temporary script path to avoid failures on install.
- Absolute path resolution issues fixed across multiple commands by standardizing repository root usage.

## Improvements
- Documentation expanded and clarified for enterprise deployments, including environment variables, proxy and CA configuration, and config precedence.
- Core command documentation updated along with code style guidelines and testing practices.
- Cargo manifest updated with a revised description, improved keywords, and reordered dependencies.

## Breaking Changes
- Renamed configuration environment variables:
  - System config is now specified via GITAR_SYSTEM_CONFIG.
  - User config environment variable renamed to GITAR_CONFIG_FILE.
  - Proxy configuration standardized to GITAR_PROXY.
  These changes may require updating existing scripts or deployment environments.

## Infrastructure
- HTTP client setup consolidated to simplify maintenance and improve consistency.
- Project metadata refreshed to better reflect current functionality and ecosystem usage.

## [1.3.2] - 2026-01-24

# Release Notes
## Features
- Define gitar binary target in Cargo manifest ([3b2dd37])

## Fixes
- Fix Windows CMD install command to use temporary script path ([eeb4412])
- Fix CMD installer to create temporary directory before version resolution and correct errorlevel checks ([d736123])

## Improvements
- Use serial_test crate for test isolation in git module ([ea33579])
- Remove unused success color import from plan module ([3b66efa])
- Update Rust dependencies ([3e82f39])
- Remove redundant Rust coding standards section from CLAUDE.md ([ce1bea2])

## [1.3.1] - 2026-01-24

# Release Notes
## Features
- Added success message after plan execution in `execute_plan` function.
- Added manual installation instructions to README, covering both binary and source installation options.

## Fixes
- Removed redundant success message after commit execution in `plan` command.

## Improvements
- Adjusted final output formatting in `execute_plan` for dry-run mode.
- Improved version bump parsing and clarified semantic versioning criteria in the release module.
- Normalized version handling in install scripts by stripping the 'v' prefix from tags for artifact URLs.

## Infrastructure
- Updated Rust dependencies.

## [1.3.0] - 2026-01-24

# Release Notes
## Features
- Added auto-apply configuration and dry-run flag to CLI, including lock file group message helpers. ([aed6e58])
- Added CLI dry-run flag parsing tests for plan and fix commands. ([1f7f551])

## Improvements
- Documented auto-apply mode and configuration examples in README. ([94b0ef2])
- Revised README introduction to clarify Gitar purpose and usage overview. ([277d9d0])

## Infrastructure
- Updated Rust dependencies. ([9c76554])

## [1.2.0] - 2026-01-24

# Release Notes
## Features
- Added `--show` flag to `init` command to display resolved configuration.  
- Introduced `HookAction` enum and CLI flag handling for hook install/uninstall.  
- Added Cargo-style CLI color styling for improved terminal output.  

## Fixes
- Updated CLI tests to reflect new subcommands (`plan`, `fix`, `tell`) and flag changes.  
- Corrected documentation and examples for renamed and removed commands.  

## Improvements
- Rewrote README and planning documentation for final CLI naming and syntax.  
- Updated architecture notes and documentation for new command structure.  
- Simplified changelog module call formatting for clarity.  

## Breaking Changes
- Removed `config` command; use `init --show` instead.  
- Renamed `explain` command to `tell` (alias supported).  
- Replaced `run` command with `plan` and `resolve` with `fix`.  
- Replaced `tag`/`no-tag` flags with `ai_author`/`no_ai_author`.  
- Renamed `insecure_tls` flag to `insecure`.  

## Infrastructure
- Removed deprecated CLI redesign plan document.  
- Internal cleanup of unused or redundant test and documentation references.

## [1.1.3] - 2026-01-24

# Release Notes
## Features
- Integrated AnalysisContext into command modules and replaced apply_smart_diff with a context-aware variant.
- Added logging and auto-staging to the release command.

## Fixes
- Added integration and CLI parsing tests for commands and resolve module, improving reliability.

## Improvements
- Reformatted command modules and cleaned up CLI tests for consistent module organization.
- Updated dependencies.
- Expanded CLAUDE.md with new release, squash, and rewrite command specifications.

## Infrastructure
- Removed outdated PLAN.md file.
- Bumped version to 1.1.2.

## [1.1.2] - 2026-01-23

# Release Notes
## Features
- Added logging for commit and tag steps in the release command for better visibility during release operations.

## Improvements
- Updated release command to use `git commit -a` to automatically stage all tracked file changes before committing.

## [1.1.1] - 2026-01-2git s3

# Release Notes
## Fixes
- Corrected inverted apply flag behavior in the release command, ensuring proper flag logic during release execution.
- Updated changelog to include details for the 1.1.0 release.

## [1.1.0] - 2026-01-23

# Release Notes
## Features
- Added `init` command with provider selection, client setup, and context loading.
- Added `plan` command for repo state inspection and CLI integration.
- Added `resolve` command for conflict resolution with per-region LLM diff previews and unmerged index detection.
- Added `commit` planning with LLM integration and execution support.
- Added `release` command with changelog generation, streaming, and diff options.
- Added modular plan layer with analysis and grouping infrastructure.
- Added prompt module for interactive TTY input and unified context management.
- Added `GitarTheme` with cyan highlighting for improved prompt visuals.
- Added Windows CMD, PowerShell, and curl-based installers with platform detection and version resolution.
- Added MIT license and crates.io metadata to Cargo.toml.

## Fixes
- Reverted crate version in Cargo manifest to 1.0.7 to correct version mismatch.
- Marked resolve tests as serial to prevent environment conflicts.
- Removed Unicode output from prints for full ASCII compliance.

## Improvements
- Renamed `plan` command to `run` and enhanced release module with changelog file handling.
- Replaced `version` command with enhanced release flow and added amend support to `commit` command.
- Enhanced `cmd_init` to support interactive and non-interactive modes with API key validation.
- Refactored prompt module into `context` and `util` namespaces for cleaner imports.
- Completed modular plan layer implementation and command restructuring.
- Clarified README with detailed architecture, setup flow, and provider configuration.
- Improved install instructions with platform-specific guidance.
- Revised CLI roadmap and reorganized documentation (README, PLAN.md).
- Simplified split command output and refactored change grouping logic.
- Formatted codebase with `cargo fmt`.

## Breaking Changes
- Deprecated `split` command replaced by new `plan` and `commit` planning workflow.
- Replaced `version` command with `release` command.

## Infrastructure
- Bumped `dialoguer` dependency to version 0.12.
- Enabled macOS Intel build job in CI workflow.
- Added serial_test dependency.
- Moved install scripts into `install/` directory.
- Renamed artifact files from `gitar-v` to `gitar-`.

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

