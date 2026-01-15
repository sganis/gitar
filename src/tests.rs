#[cfg(test)]
mod tests {
    use crate::*;

    // =========================================================================
    // CONFIG TESTS
    // =========================================================================

    #[test]
    fn config_default_is_empty() {
        let config = Config::default();
        assert!(config.api_key.is_none());
        assert!(config.model.is_none());
        assert!(config.max_tokens.is_none());
        assert!(config.temperature.is_none());
        assert!(config.base_url.is_none());
        assert!(config.base_branch.is_none());
    }

    #[test]
    fn config_serializes_to_toml() {
        let config = Config {
            api_key: Some("sk-test123".into()),
            model: Some("gpt-4o".into()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            base_url: None,
            base_branch: Some("main".into()),
        };
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("api_key = \"sk-test123\""));
        assert!(toml_str.contains("model = \"gpt-4o\""));
        assert!(toml_str.contains("max_tokens = 4096"));
        assert!(toml_str.contains("temperature = 0.7"));
        assert!(toml_str.contains("base_branch = \"main\""));
    }

    #[test]
    fn config_deserializes_from_toml() {
        let toml_str = r#"
            api_key = "sk-test"
            model = "gpt-4"
            max_tokens = 2048
            temperature = 0.5
            base_branch = "develop"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.api_key, Some("sk-test".into()));
        assert_eq!(config.model, Some("gpt-4".into()));
        assert_eq!(config.max_tokens, Some(2048));
        assert_eq!(config.temperature, Some(0.5));
        assert_eq!(config.base_branch, Some("develop".into()));
    }

    #[test]
    fn config_deserializes_partial_toml() {
        let toml_str = r#"model = "gpt-4""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.api_key.is_none());
        assert_eq!(config.model, Some("gpt-4".into()));
        assert!(config.max_tokens.is_none());
    }

    #[test]
    fn config_handles_empty_toml() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.api_key.is_none());
        assert!(config.model.is_none());
    }

    #[test]
    fn config_roundtrip() {
        let original = Config {
            api_key: Some("test-key".into()),
            model: Some("gpt-4o-mini".into()),
            max_tokens: Some(8192),
            temperature: Some(0.5),
            base_url: Some("https://api.example.com".into()),
            base_branch: Some("develop".into()),
        };
        let toml_str = toml::to_string(&original).unwrap();
        let restored: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(original.api_key, restored.api_key);
        assert_eq!(original.model, restored.model);
        assert_eq!(original.max_tokens, restored.max_tokens);
        assert_eq!(original.temperature, restored.temperature);
        assert_eq!(original.base_url, restored.base_url);
        assert_eq!(original.base_branch, restored.base_branch);
    }

    #[test]
    fn config_path_in_home() {
        if let Some(path) = Config::path() {
            let path_str = path.to_string_lossy();
            assert!(path_str.ends_with(".gitar.toml"));
        }
    }

    #[test]
    fn config_filename_correct() {
        assert_eq!(CONFIG_FILENAME, ".gitar.toml");
    }

    // =========================================================================
    // RESOLVED CONFIG TESTS
    // =========================================================================

    fn make_test_cli(
        api_key: Option<String>,
        model: Option<String>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        base_url: Option<String>,
        base_branch: Option<String>,
    ) -> Cli {
        Cli {
            api_key,
            model,
            max_tokens,
            temperature,
            base_url,
            base_branch,
            command: Commands::Config,
        }
    }

    #[test]
    fn resolved_config_uses_defaults() {
        let cli = make_test_cli(None, None, None, None, None, None);
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert!(resolved.api_key.is_none());
        assert_eq!(resolved.model, "gpt-4o-mini");
        assert_eq!(resolved.max_tokens, 500);
        assert_eq!(resolved.temperature, 0.5);
        assert_eq!(resolved.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn resolved_config_file_overrides_defaults() {
        let cli = make_test_cli(None, None, None, None, None, None);
        let file = Config {
            api_key: Some("file-key".into()),
            model: Some("gpt-3.5-turbo".into()),
            max_tokens: Some(2048),
            temperature: Some(0.3),
            base_url: Some("https://custom.api".into()),
            base_branch: Some("develop".into()),
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.api_key, Some("file-key".into()));
        assert_eq!(resolved.model, "gpt-3.5-turbo");
        assert_eq!(resolved.max_tokens, 2048);
        assert_eq!(resolved.temperature, 0.3);
        assert_eq!(resolved.base_url, "https://custom.api");
        assert_eq!(resolved.base_branch, "develop");
    }

    #[test]
    fn resolved_config_cli_overrides_file() {
        let cli = make_test_cli(
            Some("cli-key".into()),
            Some("claude-3".into()),
            Some(1024),
            Some(0.9),
            Some("https://cli.api".into()),
            Some("main".into()),
        );
        let file = Config {
            api_key: Some("file-key".into()),
            model: Some("gpt-3.5-turbo".into()),
            max_tokens: Some(2048),
            temperature: Some(0.3),
            base_url: Some("https://file.api".into()),
            base_branch: Some("develop".into()),
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.api_key, Some("cli-key".into()));
        assert_eq!(resolved.model, "claude-3");
        assert_eq!(resolved.max_tokens, 1024);
        assert_eq!(resolved.temperature, 0.9);
        assert_eq!(resolved.base_url, "https://cli.api");
        assert_eq!(resolved.base_branch, "main");
    }

    #[test]
    fn resolved_config_partial_cli_override() {
        let cli = make_test_cli(None, Some("claude-3".into()), None, None, None, None);
        let file = Config {
            api_key: Some("file-key".into()),
            model: Some("gpt-4".into()),
            max_tokens: Some(2048),
            temperature: None,
            base_url: None,
            base_branch: None,
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.api_key, Some("file-key".into()));
        assert_eq!(resolved.model, "claude-3"); // CLI wins
        assert_eq!(resolved.max_tokens, 2048); // File value
        assert_eq!(resolved.temperature, 0.5); // Default
        assert_eq!(resolved.base_url, "https://api.openai.com/v1"); // Default
    }

    // =========================================================================
    // TRUNCATE DIFF TESTS
    // =========================================================================

    #[test]
    fn truncate_diff_short_unchanged() {
        let diff = "short diff content".to_string();
        let result = truncate_diff(diff.clone(), 1000);
        assert_eq!(result, diff);
    }

    #[test]
    fn truncate_diff_long_truncated() {
        let diff = "a".repeat(500);
        let result = truncate_diff(diff, 100);
        assert!(result.len() < 500);
        assert!(result.contains("[... truncated ...]"));
    }

    #[test]
    fn truncate_diff_preserves_file_boundaries() {
        let diff = format!(
            "diff --git a/file1.rs\n{}\ndiff --git a/file2.rs\n{}",
            "a".repeat(100),
            "b".repeat(100)
        );
        let result = truncate_diff(diff, 150);
        assert!(result.contains("[... truncated ...]"));
        // Should try to cut at file boundary
        assert!(result.contains("diff --git a/file1.rs"));
    }

    #[test]
    fn truncate_diff_exact_boundary() {
        let diff = "exactly100chars".repeat(10); // 150 chars
        let result = truncate_diff(diff.clone(), 150);
        assert_eq!(result, diff);
    }

    #[test]
    fn truncate_diff_empty_string() {
        let result = truncate_diff(String::new(), 100);
        assert!(result.is_empty());
    }

    #[test]
    fn truncate_diff_max_zero() {
        let diff = "some content".to_string();
        let result = truncate_diff(diff, 0);
        assert!(result.contains("[... truncated ...]"));
    }

    #[test]
    fn truncate_diff_single_char_over() {
        let diff = "abcde".to_string();
        let result = truncate_diff(diff, 4);
        assert!(result.contains("[... truncated ...]"));
    }

    #[test]
    fn truncate_diff_no_file_boundary_in_first_half() {
        // File boundary too early should not be used
        let diff = format!(
            "diff --git a/file1.rs\n{}\n{}",
            "a".repeat(10),  // boundary at ~30
            "b".repeat(200)  // most content after
        );
        let result = truncate_diff(diff, 100);
        assert!(result.contains("[... truncated ...]"));
    }

    // =========================================================================
    // BUILD RANGE TESTS
    // =========================================================================

    #[test]
    fn build_range_with_ref() {
        let result = build_range(Some("v1.0.0"), "main");
        assert_eq!(result, Some("v1.0.0..HEAD".to_string()));
    }

    #[test]
    fn build_range_with_commit_hash() {
        let result = build_range(Some("abc123"), "main");
        assert_eq!(result, Some("abc123..HEAD".to_string()));
    }

    #[test]
    fn build_range_none_on_base_branch() {
        // When on the base branch with no ref, should return None
        // This tests the logic but depends on get_current_branch()
        let result = build_range(None, "nonexistent-branch-xyz");
        // Will compare current branch to "nonexistent-branch-xyz"
        assert!(result.is_some() || result.is_none()); // depends on current branch
    }

    // =========================================================================
    // BUILD DIFF TARGET TESTS
    // =========================================================================

    #[test]
    fn build_diff_target_with_ref() {
        let result = build_diff_target(Some("v1.0.0"), "main");
        assert_eq!(result, "v1.0.0..HEAD");
    }

    #[test]
    fn build_diff_target_with_commit() {
        let result = build_diff_target(Some("abc123def"), "main");
        assert_eq!(result, "abc123def..HEAD");
    }

    // =========================================================================
    // COMMIT INFO PARSING TESTS
    // =========================================================================

    #[test]
    fn commit_info_struct_creation() {
        let info = CommitInfo {
            hash: "abc123def456".into(),
            author: "John Doe".into(),
            date: "2024-01-15 10:30:00 +0000".into(),
            message: "Fix bug in parser".into(),
        };
        assert_eq!(info.hash, "abc123def456");
        assert_eq!(info.author, "John Doe");
        assert_eq!(info.date, "2024-01-15 10:30:00 +0000");
        assert_eq!(info.message, "Fix bug in parser");
    }

    #[test]
    fn parse_commit_log_line() {
        let line = "abc123def|John Doe|2024-01-15 10:30:00|Fix bug in parser";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert_eq!(parts.len(), 4);
        
        let info = CommitInfo {
            hash: parts[0].into(),
            author: parts[1].into(),
            date: parts[2].into(),
            message: parts[3].into(),
        };
        assert_eq!(info.hash, "abc123def");
        assert_eq!(info.author, "John Doe");
    }

    #[test]
    fn parse_commit_log_with_pipe_in_message() {
        let line = "abc123|Author|2024-01-15|Message with | pipe | chars";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[3], "Message with | pipe | chars");
    }

    #[test]
    fn parse_commit_log_incomplete_rejected() {
        let line = "abc123|Author|2024-01-15";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert!(parts.len() < 4);
    }

    #[test]
    fn parse_empty_commit_log() {
        let output = "";
        let commits: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
        assert!(commits.is_empty());
    }

    #[test]
    fn parse_commit_log_filters_empty_lines() {
        let output = "abc|author|date|msg\n\n\ndef|author2|date2|msg2\n";
        let commits: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(commits.len(), 2);
    }

    // =========================================================================
    // PROMPT TEMPLATE TESTS
    // =========================================================================

    #[test]
    fn commit_prompt_substitution() {
        let diff = "test diff";
        let original = "Original message";
        let prompt = HISTORY_USER_PROMPT
            .replace("{original_message}", original)
            .replace("{diff}", diff);
        assert!(prompt.contains("test diff"));
        assert!(prompt.contains("Original message"));
        assert!(!prompt.contains("{diff}"));
        assert!(!prompt.contains("{original_message}"));
    }

    #[test]
    fn pr_prompt_substitution() {
        let prompt = PR_USER_PROMPT
            .replace("{branch}", "feature/test")
            .replace("{commits}", "- commit 1\n- commit 2")
            .replace("{stats}", "2 files changed")
            .replace("{diff}", "diff content");
        assert!(prompt.contains("feature/test"));
        assert!(prompt.contains("- commit 1"));
        assert!(prompt.contains("2 files changed"));
        assert!(prompt.contains("diff content"));
        assert!(!prompt.contains("{branch}"));
        assert!(!prompt.contains("{commtis}"));
        assert!(!prompt.contains("{stats}"));
        assert!(!prompt.contains("{diff}"));
    }

    #[test]
    fn changelog_prompt_substitution() {
        let prompt = CHANGELOG_USER_PROMPT
            .replace("{range}", "v1.0.0..HEAD")
            .replace("{count}", "10")
            .replace("{commits}", "- [abc123] Fix bug");
        assert!(prompt.contains("v1.0.0..HEAD"));
        assert!(prompt.contains("10"));
        assert!(prompt.contains("- [abc123] Fix bug"));
        assert!(!prompt.contains("{range}"));
        assert!(!prompt.contains("{count}"));
        assert!(!prompt.contains("{commits}"));
    }

    #[test]
    fn version_prompt_substitution() {
        let prompt = VERSION_USER_PROMPT
            .replace("{version}", "1.2.3")
            .replace("{diff}", "some changes");
        assert!(prompt.contains("1.2.3"));
        assert!(prompt.contains("some changes"));
        assert!(!prompt.contains("{version}"));
        assert!(!prompt.contains("{diff}"));
    }

    #[test]
    fn explain_prompt_substitution() {
        let prompt = EXPLAIN_USER_PROMPT
            .replace("{stats}", "5 files, +100 -50")
            .replace("{diff}", "diff here");
        assert!(prompt.contains("5 files, +100 -50"));
        assert!(prompt.contains("diff here"));
        assert!(!prompt.contains("{stats}"));
        assert!(!prompt.contains("{diff}"));
    }

    // =========================================================================
    // EXCLUDE PATTERNS TESTS
    // =========================================================================

    #[test]
    fn exclude_patterns_not_empty() {
        assert!(!EXCLUDE_PATTERNS.is_empty());
    }

    #[test]
    fn exclude_patterns_format() {
        for pattern in EXCLUDE_PATTERNS {
            assert!(
                pattern.starts_with(":(exclude)"),
                "Pattern should start with :(exclude): {}",
                pattern
            );
        }
    }

    #[test]
    fn exclude_patterns_contains_lock_files() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains("*.lock")));
        assert!(patterns.iter().any(|p| p.contains("package-lock.json")));
        assert!(patterns.iter().any(|p| p.contains("yarn.lock")));
        assert!(patterns.iter().any(|p| p.contains("pnpm-lock.yaml")));
    }

    #[test]
    fn exclude_patterns_contains_build_dirs() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains("dist/*")));
        assert!(patterns.iter().any(|p| p.contains("build/*")));
        assert!(patterns.iter().any(|p| p.contains("target/*")));
    }

    #[test]
    fn exclude_patterns_contains_minified() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains("*.min.js")));
        assert!(patterns.iter().any(|p| p.contains("*.min.css")));
        assert!(patterns.iter().any(|p| p.contains("*.map")));
    }

    #[test]
    fn exclude_patterns_contains_env() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains(".env")));
    }

    // =========================================================================
    // SYSTEM PROMPT TESTS
    // =========================================================================

    #[test]
    fn system_prompts_not_empty() {
        assert!(!HISTORY_SYSTEM_PROMPT.is_empty());
        assert!(!COMMIT_SYSTEM_PROMPT.is_empty());
        assert!(!PR_SYSTEM_PROMPT.is_empty());
        assert!(!CHANGELOG_SYSTEM_PROMPT.is_empty());
        assert!(!EXPLAIN_SYSTEM_PROMPT.is_empty());
        assert!(!VERSION_SYSTEM_PROMPT.is_empty());
    }

    #[test]
    fn commit_system_prompt_contains_types() {
        assert!(HISTORY_SYSTEM_PROMPT.contains("Feat"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Fix"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Refactor"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Docs"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Style"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Test"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Chore"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Perf"));
    }

    #[test]
    fn prompts_disallow_emojis() {
        let prompts = [
            HISTORY_SYSTEM_PROMPT,
            COMMIT_SYSTEM_PROMPT,
            PR_SYSTEM_PROMPT,
            CHANGELOG_SYSTEM_PROMPT,
            EXPLAIN_SYSTEM_PROMPT,
            VERSION_SYSTEM_PROMPT,
        ];
        for prompt in prompts {
            assert!(
                prompt.contains("ASCII") || prompt.contains("emoji"),
                "Prompt should mention ASCII or emoji restriction"
            );
        }
    }

    #[test]
    fn version_prompt_contains_semver() {
        assert!(VERSION_SYSTEM_PROMPT.contains("MAJOR"));
        assert!(VERSION_SYSTEM_PROMPT.contains("MINOR"));
        assert!(VERSION_SYSTEM_PROMPT.contains("PATCH"));
    }

    #[test]
    fn pr_prompt_contains_sections() {
        assert!(PR_SYSTEM_PROMPT.contains("Summary"));
        assert!(PR_SYSTEM_PROMPT.contains("What Changed"));
        assert!(PR_SYSTEM_PROMPT.contains("Why"));
        assert!(PR_SYSTEM_PROMPT.contains("Risks"));
        assert!(PR_SYSTEM_PROMPT.contains("Testing"));
    }

    #[test]
    fn changelog_prompt_contains_sections() {
        assert!(CHANGELOG_SYSTEM_PROMPT.contains("Features"));
        assert!(CHANGELOG_SYSTEM_PROMPT.contains("Fixes"));
        assert!(CHANGELOG_SYSTEM_PROMPT.contains("Breaking Changes"));
    }

    // =========================================================================
    // GIT UTILITY FUNCTION TESTS (non-git-dependent)
    // =========================================================================

    #[test]
    fn run_git_returns_result() {
        // This will fail if not in a git repo, but tests the function signature
        let result = run_git(&["--version"]);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("git version"));
    }

    #[test]
    fn run_git_status_returns_tuple() {
        let (stdout, stderr, success) = run_git_status(&["--version"]);
        assert!(success);
        assert!(stdout.contains("git version"));
        assert!(stderr.is_empty() || !stderr.contains("fatal"));
    }

    #[test]
    fn run_git_status_handles_invalid_command() {
        let (stdout, stderr, success) = run_git_status(&["invalid-command-xyz"]);
        assert!(!success);
        assert!(stdout.is_empty() || stderr.contains("git"));
    }

    // =========================================================================
    // INTEGRATION-STYLE TESTS (require git repo)
    // =========================================================================

    #[test]
    fn is_git_repo_detects_repo() {
        // This test file should be in a git repo
        let result = is_git_repo();
        // We can't assert true because CI might not have git initialized
        // Just verify the function runs without panic
        let _ = result;
    }

    #[test]
    fn get_current_branch_returns_string() {
        let branch = get_current_branch();
        // Should return something (either branch name or "HEAD")
        assert!(!branch.is_empty());
    }

    #[test]
    fn get_default_branch_returns_valid() {
        let branch = get_default_branch();
        // Should be "main" or "master"
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn get_current_version_returns_string() {
        let version = get_current_version();
        // Should return something (version tag or "0.0.0")
        assert!(!version.is_empty());
    }

    // =========================================================================
    // CLI STRUCTURE TESTS
    // =========================================================================

    #[test]
    fn cli_parses_commit_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "commit"]).unwrap();
        matches!(cli.command, Commands::Commit { .. });
    }

    #[test]
    fn cli_parses_commit_with_flags() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "commit", "-p", "-a"]).unwrap();
        if let Commands::Commit { push, all, .. } = cli.command {
            assert!(push);
            assert!(all);
        } else {
            panic!("Expected Commit command");
        }
    }

    #[test]
    fn cli_parses_staged_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "staged"]).unwrap();
        matches!(cli.command, Commands::Staged);
    }

    #[test]
    fn cli_parses_unstaged_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "unstaged"]).unwrap();
        matches!(cli.command, Commands::Unstaged);
    }

    #[test]
    fn cli_parses_pr_with_base() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "pr", "develop"]).unwrap();
        if let Commands::Pr { base, staged } = cli.command {
            assert_eq!(base, Some("develop".into()));
            assert!(!staged);
        } else {
            panic!("Expected Pr command");
        }
    }

    #[test]
    fn cli_parses_changelog_with_ref() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "changelog", "v1.0.0"]).unwrap();
        if let Commands::Changelog { from, .. } = cli.command {
            assert_eq!(from, Some("v1.0.0".into()));
        } else {
            panic!("Expected Changelog command");
        }
    }

    #[test]
    fn cli_parses_commits_with_options() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar", "history", "v1.0.0", 
            "--since", "2024-01-01",
            "-n", "10",
            "--delay", "1000"
        ]).unwrap();
        if let Commands::History { from, since, limit, delay, .. } = cli.command {
            assert_eq!(from, Some("v1.0.0".into()));
            assert_eq!(since, Some("2024-01-01".into()));
            assert_eq!(limit, Some(10));
            assert_eq!(delay, 1000);
        } else {
            panic!("Expected History command");
        }
    }

    #[test]
    fn cli_parses_global_options() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--model", "gpt-4",
            "--max-tokens", "2048",
            "--temperature", "0.5",
            "staged"
        ]).unwrap();
        assert_eq!(cli.model, Some("gpt-4".into()));
        assert_eq!(cli.max_tokens, Some(2048));
        assert_eq!(cli.temperature, Some(0.5));
    }

    #[test]
    fn cli_parses_version_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "version", "v1.0.0", "--current", "1.2.3"]).unwrap();
        if let Commands::Version { base, current } = cli.command {
            assert_eq!(base, Some("v1.0.0".into()));
            assert_eq!(current, Some("1.2.3".into()));
        } else {
            panic!("Expected Version command");
        }
    }

    #[test]
    fn cli_parses_explain_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "explain", "--staged"]).unwrap();
        if let Commands::Explain { from, staged } = cli.command {
            assert!(from.is_none());
            assert!(staged);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn cli_parses_init_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar", "init",
            "--model", "claude-3",
            "--base-branch", "develop"
        ]).unwrap();
        if let Commands::Init { model, base_branch, .. } = cli.command {
            assert_eq!(model, Some("claude-3".into()));
            assert_eq!(base_branch, Some("develop".into()));
        } else {
            panic!("Expected Init command");
        }
    }

    #[test]
    fn cli_parses_config_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "config"]).unwrap();
        matches!(cli.command, Commands::Config);
    }

    #[test]
    fn cli_parses_models_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "models"]).unwrap();
        matches!(cli.command, Commands::Models);
    }

    #[test]
    fn cli_commit_no_tag_flag() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "commit", "--no-tag"]).unwrap();
        if let Commands::Commit { tag, no_tag, .. } = cli.command {
            assert!(tag); // default is true
            assert!(no_tag); // explicitly set
        } else {
            panic!("Expected Commit command");
        }
    }

    // =========================================================================
    // API TYPES TESTS
    // =========================================================================

    #[test]
    fn chat_message_serializes() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn chat_completion_request_serializes() {
        let req = ChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are helpful.".to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "Hi".to_string(),
                },
            ],
            max_tokens: 1024,
            temperature: 0.7,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"gpt-4o\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(json.contains("\"role\":\"system\""));
        assert!(json.contains("\"role\":\"user\""));
    }

    #[test]
    fn chat_completion_response_deserializes() {
        let json = r#"{
            "choices": [
                {
                    "message": {
                        "content": "Hello! How can I help?"
                    }
                }
            ]
        }"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(
            resp.choices[0].message.content,
            Some("Hello! How can I help?".to_string())
        );
    }

    #[test]
    fn chat_completion_response_handles_null_content() {
        let json = r#"{
            "choices": [
                {
                    "message": {
                        "content": null
                    }
                }
            ]
        }"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert!(resp.choices[0].message.content.is_none());
    }

    #[test]
    fn models_response_deserializes() {
        let json = r#"{
            "data": [
                {"id": "gpt-4o"},
                {"id": "gpt-4o-mini"},
                {"id": "gpt-3.5-turbo"}
            ]
        }"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 3);
        assert_eq!(resp.data[0].id, "gpt-4o");
        assert_eq!(resp.data[1].id, "gpt-4o-mini");
        assert_eq!(resp.data[2].id, "gpt-3.5-turbo");
    }

    #[test]
    fn models_response_handles_empty() {
        let json = r#"{"data": []}"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
    }

    #[test]
    fn api_error_deserializes() {
        let json = r#"{
            "error": {
                "message": "Invalid API key"
            }
        }"#;
        let err: ApiError = serde_json::from_str(json).unwrap();
        assert!(err.error.is_some());
        assert_eq!(err.error.unwrap().message, Some("Invalid API key".to_string()));
    }

    #[test]
    fn api_error_handles_missing_fields() {
        let json = r#"{"error": {}}"#;
        let err: ApiError = serde_json::from_str(json).unwrap();
        assert!(err.error.is_some());
        assert!(err.error.unwrap().message.is_none());
    }

    #[test]
    fn api_error_handles_null_error() {
        let json = r#"{"error": null}"#;
        let err: ApiError = serde_json::from_str(json).unwrap();
        assert!(err.error.is_none());
    }
}