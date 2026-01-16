# Release Notes

## Features

- Add comprehensive support for Anthropic Claude API with automatic provider detection and environment variable selection (ANTHROPIC_API_KEY)
- Add Claude model validation and configuration management with support for multiple Claude model families (claude-3-5-sonnet, claude-3-5-haiku, claude-3-opus)
- Add support for Anthropic and Gemini providers with unified LLM interface
- Add date filtering capabilities to Explain CLI command with --since and --until options
- Add reasoning-model handling with automatic selection between max_tokens and max_completion_tokens based on model requirements
- Add retry mechanism for models that require max_completion_tokens with automatic model marking on specific errors
- Add SOCKS proxy support through updated reqwest dependency

## Improvements

- Switch default model from gpt-4o-mini to gpt-5-chat-latest for improved performance
- Improve diff range selection logic when operating on base branch
- Update commit message format to specify single-line responses for better consistency
- Make token and temperature fields optional in API requests to reduce payload size
- Enhance user prompts with clearer instructions for commit message generation
- Update pricing category labels in comments for improved clarity
- Add default values for CLI options to improve user experience
- Remove prompt debug output for cleaner console output

## Infrastructure

- Add comprehensive test coverage for Claude API integration including serialization, detection, configuration, and model validation
- Update tests to use gpt-5-chat-latest as default model and verify optional request fields are omitted during serialization
- Add comprehensive documentation for Anthropic Claude support, Rust rationale, and improved configuration management
- Update dependencies to latest versions: dirs 6.0.0, dirs-sys 0.5.0, redox_users 0.5.2
- Remove outdated windows-targets and related packages for improved compatibility
- Add build status badge to README
- Extract send_chat_request helper function for better code organization
- Derive Clone trait for ChatMessage struct
- Add script to interact with various LLM providers and test model completions
- Remove unused requirements.txt file

v1.0.1
# Release Notes

## Features

- Add Claude API support with multi-provider architecture enabling seamless integration with Anthropic's Claude models alongside existing OpenAI support
- Add automatic provider detection that selects the appropriate API based on model name and available environment variables (ANTHROPIC_API_KEY or OPENAI_API_KEY)
- Add support for reasoning models (o1, o3) with automatic handling of max_completion_tokens parameter and retry logic for cache-incompatible models
- Add date filtering capabilities with --since and --until options for the Explain command to analyze commits within specific time ranges
- Add SOCKS proxy support through updated reqwest dependency configuration
- Add unified LLM interface supporting multiple providers including OpenAI, Anthropic, and Gemini
- Add interactive AI-generated commit message feature with options to edit, regenerate, and push changes
- Add comprehensive test coverage for Claude API integration including serialization, detection, configuration, and model validation
- Add CI/CD workflow for building and testing on multiple platforms (Linux, macOS, Windows)
- Add GitHub release automation in CI pipeline with dynamic artifact versioning

## Improvements

- Switch default model from gpt-4o-mini to gpt-5-chat-latest for improved performance
- Improve diff range selection on base branch for more accurate commit analysis
- Update commit message format to specify single-line responses for cleaner output
- Enhance user prompts with clearer instructions and improved formatting
- Update pricing category labels for better clarity in code comments
- Refactor code structure for improved functionality, clarity, and maintainability
- Expand README with detailed usage instructions, examples, and comprehensive documentation
- Remove prompt debug output for cleaner console experience
- Make token and temperature fields optional in API requests for greater flexibility
- Automatically choose between max_tokens and max_completion_tokens based on model requirements

## Infrastructure

- Update dependencies to latest versions including dirs 6.0.0, dirs-sys 0.5.0, redox_users 0.5.2, and toml 0.9.8
- Add SOCKS support in reqwest dependency for proxy capabilities
- Remove outdated windows-targets and related packages for improved compatibility
- Add build status badge to README for visibility into CI/CD status
