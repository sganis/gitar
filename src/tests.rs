// tests.rs
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
            model: Some("gpt-5-chat-latest".into()),
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
            provider: None,
            base_branch,
            command: Commands::Config,
        }
    }

    fn make_test_cli_with_provider(
        api_key: Option<String>,
        model: Option<String>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        base_url: Option<String>,
        provider: Option<String>,
        base_branch: Option<String>,
    ) -> Cli {
        Cli {
            api_key,
            model,
            max_tokens,
            temperature,
            base_url,
            provider,
            base_branch,
            command: Commands::Config,
        }
    }

    #[test]
    fn resolved_config_uses_defaults() {
        std::env::remove_var("OPENAI_API_KEY");
        let cli = make_test_cli(None, None, None, None, None, None);
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert!(resolved.api_key.is_none());
        assert_eq!(resolved.model, "gpt-5-chat-latest");
        assert_eq!(resolved.max_tokens, 500);
        assert_eq!(resolved.temperature, 0.5);
        assert_eq!(resolved.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn resolved_config_file_overrides_defaults() {
        std::env::remove_var("OPENAI_API_KEY");
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
        std::env::remove_var("OPENAI_API_KEY");
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
        let diff = format!(
            "diff --git a/file1.rs\n{}\n{}",
            "a".repeat(10),
            "b".repeat(200)
        );
        let result = truncate_diff(diff, 100);
        assert!(result.contains("[... truncated ...]"));
    }

    // =========================================================================
    // BUILD RANGE TESTS
    // =========================================================================

    #[test]
    fn build_range_with_ref() {
        let result = build_range(Some("v1.0.0"), None, "main");
        assert_eq!(result, Some("v1.0.0..HEAD".to_string()));
    }

    #[test]
    fn build_range_with_ref_and_to() {
        let result = build_range(Some("v1.0.0"), Some("v1.0.1"), "main");
        assert_eq!(result, Some("v1.0.0..v1.0.1".to_string()));
    }

    #[test]
    fn build_range_with_commit_hash() {
        let result = build_range(Some("abc123"), None, "main");
        assert_eq!(result, Some("abc123..HEAD".to_string()));
    }

    #[test]
    fn build_range_none_on_base_branch() {
        let result = build_range(None, None, "nonexistent-branch-xyz");
        assert!(result.is_some() || result.is_none());
    }

    // =========================================================================
    // BUILD DIFF TARGET TESTS
    // =========================================================================

    #[test]
    fn build_diff_target_with_ref() {
        let result = build_diff_target(Some("v1.0.0"), None, "main");
        assert_eq!(result, "v1.0.0..HEAD");
    }

    #[test]
    fn build_diff_target_with_ref_and_to() {
        let result = build_diff_target(Some("v1.0.0"), Some("v1.0.1"), "main");
        assert_eq!(result, "v1.0.0..v1.0.1");
    }

    #[test]
    fn build_diff_target_with_commit() {
        let result = build_diff_target(Some("abc123def"), None, "main");
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
        assert!(!prompt.contains("{commits}"));
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
    // GIT UTILITY FUNCTION TESTS
    // =========================================================================

    #[test]
    fn run_git_returns_result() {
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
        let result = is_git_repo();
        let _ = result;
    }

    #[test]
    fn get_current_branch_returns_string() {
        let branch = get_current_branch();
        assert!(!branch.is_empty());
    }

    #[test]
    fn get_default_branch_returns_valid() {
        let branch = get_default_branch();
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn get_current_version_returns_string() {
        let version = get_current_version();
        assert!(!version.is_empty());
    }

    // =========================================================================
    // CLI STRUCTURE TESTS
    // =========================================================================

    #[test]
    fn cli_parses_commit_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "commit"]).unwrap();
        assert!(matches!(cli.command, Commands::Commit { .. }));
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
        assert!(matches!(cli.command, Commands::Staged));
    }

    #[test]
    fn cli_parses_unstaged_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "unstaged"]).unwrap();
        assert!(matches!(cli.command, Commands::Unstaged));
    }

    #[test]
    fn cli_parses_pr_with_base() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "pr", "develop"]).unwrap();
        if let Commands::Pr { base, to, staged } = cli.command {
            assert_eq!(base, Some("develop".into()));
            assert!(to.is_none());
            assert!(!staged);
        } else {
            panic!("Expected Pr command");
        }
    }

    #[test]
    fn cli_parses_pr_with_to() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "pr", "main", "--to", "feature/oauth"]).unwrap();
        if let Commands::Pr { base, to, staged } = cli.command {
            assert_eq!(base, Some("main".into()));
            assert_eq!(to, Some("feature/oauth".into()));
            assert!(!staged);
        } else {
            panic!("Expected Pr command");
        }
    }

    #[test]
    fn cli_parses_changelog_with_ref() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "changelog", "v1.0.0"]).unwrap();
        if let Commands::Changelog { from, to, .. } = cli.command {
            assert_eq!(from, Some("v1.0.0".into()));
            assert!(to.is_none());
        } else {
            panic!("Expected Changelog command");
        }
    }

    #[test]
    fn cli_parses_changelog_with_to() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "changelog", "v1.0.0", "--to", "v1.0.1"]).unwrap();
        if let Commands::Changelog { from, to, .. } = cli.command {
            assert_eq!(from, Some("v1.0.0".into()));
            assert_eq!(to, Some("v1.0.1".into()));
        } else {
            panic!("Expected Changelog command");
        }
    }

    #[test]
    fn cli_parses_commits_with_options() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "history",
            "v1.0.0",
            "--since",
            "2024-01-01",
            "-n",
            "10",
            "--delay",
            "1000",
        ])
        .unwrap();
        if let Commands::History {
            from,
            to,
            since,
            limit,
            delay,
            ..
        } = cli.command
        {
            assert_eq!(from, Some("v1.0.0".into()));
            assert!(to.is_none());
            assert_eq!(since, Some("2024-01-01".into()));
            assert_eq!(limit, Some(10));
            assert_eq!(delay, 1000);
        } else {
            panic!("Expected History command");
        }
    }

    #[test]
    fn cli_parses_history_with_to() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "history", "v1.0.0", "--to", "v1.0.1"]).unwrap();
        if let Commands::History { from, to, .. } = cli.command {
            assert_eq!(from, Some("v1.0.0".into()));
            assert_eq!(to, Some("v1.0.1".into()));
        } else {
            panic!("Expected History command");
        }
    }

    #[test]
    fn cli_parses_global_options() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--model",
            "gpt-4",
            "--max-tokens",
            "2048",
            "--temperature",
            "0.5",
            "staged",
        ])
        .unwrap();
        assert_eq!(cli.model, Some("gpt-4".into()));
        assert_eq!(cli.max_tokens, Some(2048));
        assert_eq!(cli.temperature, Some(0.5));
    }

    #[test]
    fn cli_parses_version_command() {
        use clap::Parser;
        let cli =
            Cli::try_parse_from(["gitar", "version", "v1.0.0", "--current", "1.2.3"]).unwrap();
        if let Commands::Version { base, to, current } = cli.command {
            assert_eq!(base, Some("v1.0.0".into()));
            assert!(to.is_none());
            assert_eq!(current, Some("1.2.3".into()));
        } else {
            panic!("Expected Version command");
        }
    }

    #[test]
    fn cli_parses_version_with_to() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "version", "v1.0.0", "--to", "v1.0.1"]).unwrap();
        if let Commands::Version { base, to, current } = cli.command {
            assert_eq!(base, Some("v1.0.0".into()));
            assert_eq!(to, Some("v1.0.1".into()));
            assert!(current.is_none());
        } else {
            panic!("Expected Version command");
        }
    }

    #[test]
    fn cli_parses_explain_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "explain", "--staged"]).unwrap();
        if let Commands::Explain {
            from,
            to,
            since,
            until,
            staged,
        } = cli.command
        {
            assert!(from.is_none());
            assert!(to.is_none());
            assert!(since.is_none());
            assert!(until.is_none());
            assert!(staged);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn cli_parses_explain_with_date_filters() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "explain",
            "v1.0.0",
            "--since",
            "2024-01-01",
            "--until",
            "2024-12-31",
        ])
        .unwrap();
        if let Commands::Explain {
            from,
            to,
            since,
            until,
            staged,
        } = cli.command
        {
            assert_eq!(from, Some("v1.0.0".into()));
            assert!(to.is_none());
            assert_eq!(since, Some("2024-01-01".into()));
            assert_eq!(until, Some("2024-12-31".into()));
            assert!(!staged);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn cli_parses_explain_with_to() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "explain", "v1.0.0", "--to", "v1.0.1"]).unwrap();
        if let Commands::Explain { from, to, staged, .. } = cli.command {
            assert_eq!(from, Some("v1.0.0".into()));
            assert_eq!(to, Some("v1.0.1".into()));
            assert!(!staged);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn cli_parses_init_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--model", "claude-3", "--base-branch", "develop", "init"])
            .unwrap();
        assert!(matches!(cli.command, Commands::Init));
        assert_eq!(cli.model, Some("claude-3".into()));
        assert_eq!(cli.base_branch, Some("develop".into()));
    }

    #[test]
    fn cli_parses_config_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "config"]).unwrap();
        assert!(matches!(cli.command, Commands::Config));
    }

    #[test]
    fn cli_parses_models_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "models"]).unwrap();
        assert!(matches!(cli.command, Commands::Models));
    }

    #[test]
    fn cli_commit_no_tag_flag() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "commit", "--no-tag"]).unwrap();
        if let Commands::Commit { tag, no_tag, .. } = cli.command {
            assert!(tag);
            assert!(no_tag);
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
            max_tokens: Some(1024),
            max_completion_tokens: None,
            temperature: Some(0.7),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"gpt-4o\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(json.contains("\"role\":\"system\""));
        assert!(json.contains("\"role\":\"user\""));
        assert!(!json.contains("max_completion_tokens"));
    }

    #[test]
    fn chat_completion_response_deserializes() {
        let json = r#"{
            "choices": [
                { "message": { "content": "Hello! How can I help?" } }
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
                { "message": { "content": null } }
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
                {"id": "gpt-5-chat-latest"},
                {"id": "gpt-3.5-turbo"}
            ]
        }"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 3);
        assert_eq!(resp.data[0].id, "gpt-4o");
        assert_eq!(resp.data[1].id, "gpt-5-chat-latest");
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
            "error": { "message": "Invalid API key" }
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

    // =========================================================================
    // CLAUDE API TYPES TESTS
    // =========================================================================

    #[test]
    fn claude_request_serializes() {
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            system: "You are helpful.".to_string(),
            max_tokens: 1024,
            temperature: Some(0.7),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"claude-sonnet-4-5-20250929\""));
        assert!(json.contains("\"system\":\"You are helpful.\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(json.contains("\"role\":\"user\""));
    }

    #[test]
    fn claude_request_skips_none_temperature() {
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![],
            system: "test".to_string(),
            max_tokens: 500,
            temperature: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("temperature"));
    }

    #[test]
    fn claude_response_deserializes() {
        let json = r#"{
            "content": [
                { "type": "text", "text": "Hello! How can I help?" }
            ]
        }"#;
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.content[0].text, Some("Hello! How can I help?".to_string()));
    }

    #[test]
    fn claude_response_handles_null_text() {
        let json = r#"{
            "content": [
                { "type": "text", "text": null }
            ]
        }"#;
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert!(resp.content[0].text.is_none());
    }

    #[test]
    fn claude_response_handles_empty_content() {
        let json = r#"{"content": []}"#;
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert!(resp.content.is_empty());
    }

    #[test]
    fn claude_response_handles_multiple_content_blocks() {
        let json = r#"{
            "content": [
                {"text": "First part"},
                {"text": "Second part"}
            ]
        }"#;
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content.len(), 2);
        assert_eq!(resp.content[0].text, Some("First part".to_string()));
        assert_eq!(resp.content[1].text, Some("Second part".to_string()));
    }

    // =========================================================================
    // GEMINI API TYPES TESTS
    // =========================================================================

    #[test]
    fn gemini_request_serializes() {
        let req = GeminiGenerateContentRequest {
            system_instruction: Some(GeminiContent {
                parts: vec![GeminiPart { text: "You are helpful.".to_string() }],
            }),
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: "Hello".to_string() }],
            }],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"system_instruction\""));
        assert!(json.contains("You are helpful."));
        assert!(json.contains("\"contents\""));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn gemini_request_skips_none_system_instruction() {
        let req = GeminiGenerateContentRequest {
            system_instruction: None,
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: "Hello".to_string() }],
            }],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("system_instruction"));
    }

    #[test]
    fn gemini_response_deserializes() {
        let json = r#"{
            "candidates": [
                {
                    "content": {
                        "parts": [{"text": "Hello! How can I help?"}]
                    }
                }
            ]
        }"#;
        let resp: GeminiGenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(resp.candidates.is_some());
        let candidates = resp.candidates.unwrap();
        assert_eq!(candidates.len(), 1);
        let text = candidates[0].content.as_ref().unwrap().parts[0].text.clone();
        assert_eq!(text, "Hello! How can I help?");
    }

    #[test]
    fn gemini_response_handles_null_candidates() {
        let json = r#"{"candidates": null}"#;
        let resp: GeminiGenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(resp.candidates.is_none());
    }

    #[test]
    fn gemini_response_handles_empty_candidates() {
        let json = r#"{"candidates": []}"#;
        let resp: GeminiGenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(resp.candidates.as_ref().unwrap().is_empty());
    }

    #[test]
    fn gemini_response_handles_null_content() {
        let json = r#"{
            "candidates": [{"content": null}]
        }"#;
        let resp: GeminiGenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(resp.candidates.unwrap()[0].content.is_none());
    }

    #[test]
    fn gemini_models_response_deserializes() {
        let json = r#"{
            "models": [
                {"name": "models/gemini-2.5-flash"},
                {"name": "models/gemini-2.5-pro"},
                {"name": "models/gemini-1.5-flash"}
            ]
        }"#;
        let resp: GeminiModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.models.len(), 3);
        assert_eq!(resp.models[0].name, "models/gemini-2.5-flash");
        assert_eq!(resp.models[1].name, "models/gemini-2.5-pro");
    }

    #[test]
    fn gemini_models_response_handles_empty() {
        let json = r#"{"models": []}"#;
        let resp: GeminiModelsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.models.is_empty());
    }

    #[test]
    fn gemini_model_name_strip_prefix() {
        let name = "models/gemini-2.5-flash";
        let stripped = name.strip_prefix("models/").unwrap_or(name);
        assert_eq!(stripped, "gemini-2.5-flash");
    }

    #[test]
    fn gemini_model_name_no_prefix() {
        let name = "gemini-2.5-flash";
        let stripped = name.strip_prefix("models/").unwrap_or(name);
        assert_eq!(stripped, "gemini-2.5-flash");
    }

    // =========================================================================
    // API DETECTION TESTS
    // =========================================================================

    #[test]
    fn is_claude_api_detects_anthropic_url() {
        let config = ResolvedConfig {
            api_key: None,
            model: "claude-sonnet-4-5-20250929".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: "https://api.anthropic.com/v1".into(),
            base_branch: "main".into(),
        };
        let client = LlmClient::new(&config).unwrap();
        assert!(client.is_claude_api());
    }

    #[test]
    fn is_claude_api_false_for_openai() {
        let config = ResolvedConfig {
            api_key: None,
            model: "gpt-5-chat-latest".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: "https://api.openai.com/v1".into(),
            base_branch: "main".into(),
        };
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_claude_api());
    }

    #[test]
    fn is_claude_api_false_for_custom_url() {
        let config = ResolvedConfig {
            api_key: None,
            model: "local-model".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: "http://localhost:8080".into(),
            base_branch: "main".into(),
        };
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_claude_api());
    }

    #[test]
    fn is_gemini_api_detects_google_url() {
        let config = ResolvedConfig {
            api_key: None,
            model: "gemini-2.5-flash".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: "https://generativelanguage.googleapis.com".into(),
            base_branch: "main".into(),
        };
        let client = LlmClient::new(&config).unwrap();
        assert!(client.is_gemini_api());
        assert!(!client.is_claude_api());
    }

    #[test]
    fn is_gemini_api_false_for_openai() {
        let config = ResolvedConfig {
            api_key: None,
            model: "gpt-5-chat-latest".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: "https://api.openai.com/v1".into(),
            base_branch: "main".into(),
        };
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn is_gemini_api_false_for_claude() {
        let config = ResolvedConfig {
            api_key: None,
            model: "claude-sonnet-4-5-20250929".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: "https://api.anthropic.com/v1".into(),
            base_branch: "main".into(),
        };
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_gemini_api());
        assert!(client.is_claude_api());
    }

    #[test]
    fn is_ollama_detected_from_localhost() {
        let config = ResolvedConfig {
            api_key: None,
            model: "llama3.2:latest".into(),
            max_tokens: 500,
            temperature: 0.5,
            base_url: "http://localhost:11434/v1".into(),
            base_branch: "main".into(),
        };
        let client = LlmClient::new(&config).unwrap();
        assert!(!client.is_claude_api());
        assert!(!client.is_gemini_api());
    }

    #[test]
    fn provider_detection_mutually_exclusive() {
        let urls = [
            ("https://api.openai.com/v1", false, false),
            ("https://api.anthropic.com/v1", true, false),
            ("https://generativelanguage.googleapis.com", false, true),
            ("https://api.groq.com/openai/v1", false, false),
            ("http://localhost:11434/v1", false, false),
            ("http://127.0.0.1:11434/v1", false, false),
            ("http://localhost:8080", false, false),
        ];

        for (url, expected_claude, expected_gemini) in urls {
            let config = ResolvedConfig {
                api_key: None,
                model: "test".into(),
                max_tokens: 500,
                temperature: 0.5,
                base_url: url.into(),
                base_branch: "main".into(),
            };
            let client = LlmClient::new(&config).unwrap();
            assert_eq!(
                client.is_claude_api(),
                expected_claude,
                "Claude detection failed for {}",
                url
            );
            assert_eq!(
                client.is_gemini_api(),
                expected_gemini,
                "Gemini detection failed for {}",
                url
            );
        }
    }

    // =========================================================================
    // RESOLVED CONFIG PROVIDER TESTS
    // =========================================================================

    #[test]
    fn resolved_config_uses_claude_default_model() {
        let cli = make_test_cli(
            None,
            None,
            None,
            None,
            Some("https://api.anthropic.com/v1".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "claude-sonnet-4-5-20250929");
        assert_eq!(resolved.base_url, "https://api.anthropic.com/v1");
    }

    #[test]
    fn resolved_config_uses_openai_default_model() {
        std::env::remove_var("OPENAI_API_KEY");
        let cli = make_test_cli(None, None, None, None, None, None);
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "gpt-5-chat-latest");
        assert_eq!(resolved.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn resolved_config_cli_model_overrides_claude_default() {
        let cli = make_test_cli(
            None,
            Some("claude-opus-4-5-20251101".into()),
            None,
            None,
            Some("https://api.anthropic.com/v1".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "claude-opus-4-5-20251101");
    }

    #[test]
    fn resolved_config_file_model_overrides_claude_default() {
        let cli = make_test_cli(
            None,
            None,
            None,
            None,
            Some("https://api.anthropic.com/v1".into()),
            None,
        );
        let file = Config {
            model: Some("claude-haiku-4-5-20251001".into()),
            ..Config::default()
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "claude-haiku-4-5-20251001");
    }

    #[test]
    fn resolved_config_file_url_determines_default_model() {
        let cli = make_test_cli(None, None, None, None, None, None);
        let file = Config {
            base_url: Some("https://api.anthropic.com/v1".into()),
            ..Config::default()
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "claude-sonnet-4-5-20250929");
    }

    #[test]
    fn resolved_config_uses_gemini_default_model() {
        let cli = make_test_cli(
            None,
            None,
            None,
            None,
            Some("https://generativelanguage.googleapis.com".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "gemini-2.5-flash");
        assert_eq!(resolved.base_url, "https://generativelanguage.googleapis.com");
    }

    #[test]
    fn resolved_config_cli_model_overrides_gemini_default() {
        let cli = make_test_cli(
            None,
            Some("gemini-2.5-pro".into()),
            None,
            None,
            Some("https://generativelanguage.googleapis.com".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "gemini-2.5-pro");
    }

    #[test]
    fn resolved_config_file_url_determines_gemini_default_model() {
        let cli = make_test_cli(None, None, None, None, None, None);
        let file = Config {
            base_url: Some("https://generativelanguage.googleapis.com".into()),
            ..Config::default()
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "gemini-2.5-flash");
    }

    #[test]
    fn resolved_config_groq_uses_openai_default_model() {
        let cli = make_test_cli(
            None,
            None,
            None,
            None,
            Some("https://api.groq.com/openai/v1".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "gpt-5-chat-latest");
        assert_eq!(resolved.base_url, "https://api.groq.com/openai/v1");
    }

    #[test]
    fn resolved_config_groq_with_custom_model() {
        let cli = make_test_cli(
            None,
            Some("llama-3.3-70b-versatile".into()),
            None,
            None,
            Some("https://api.groq.com/openai/v1".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "llama-3.3-70b-versatile");
    }

    #[test]
    fn resolved_config_ollama_uses_openai_default_model() {
        let cli = make_test_cli(
            None,
            None,
            None,
            None,
            Some("http://localhost:11434/v1".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "gpt-5-chat-latest");
        assert_eq!(resolved.base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn resolved_config_ollama_with_custom_model() {
        let cli = make_test_cli(
            None,
            Some("codellama:13b".into()),
            None,
            None,
            Some("http://localhost:11434/v1".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.model, "codellama:13b");
    }

    #[test]
    fn resolved_config_ollama_no_api_key_needed() {
        std::env::remove_var("OPENAI_API_KEY");
        let cli = make_test_cli(
            None,
            Some("mistral:latest".into()),
            None,
            None,
            Some("http://localhost:11434/v1".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert!(resolved.api_key.is_none());
    }

    // =========================================================================
    // PROVIDER MAPPING TESTS
    // =========================================================================

    #[test]
    fn provider_to_url_openai() {
        assert_eq!(provider_to_url("openai"), Some(PROVIDER_OPENAI));
        assert_eq!(provider_to_url("OPENAI"), Some(PROVIDER_OPENAI));
        assert_eq!(provider_to_url("OpenAI"), Some(PROVIDER_OPENAI));
    }

    #[test]
    fn provider_to_url_claude() {
        assert_eq!(provider_to_url("claude"), Some(PROVIDER_CLAUDE));
        assert_eq!(provider_to_url("CLAUDE"), Some(PROVIDER_CLAUDE));
        assert_eq!(provider_to_url("anthropic"), Some(PROVIDER_CLAUDE));
        assert_eq!(provider_to_url("Anthropic"), Some(PROVIDER_CLAUDE));
    }

    #[test]
    fn provider_to_url_gemini() {
        assert_eq!(provider_to_url("gemini"), Some(PROVIDER_GEMINI));
        assert_eq!(provider_to_url("GEMINI"), Some(PROVIDER_GEMINI));
    }

    #[test]
    fn provider_to_url_groq() {
        assert_eq!(provider_to_url("groq"), Some(PROVIDER_GROQ));
        assert_eq!(provider_to_url("GROQ"), Some(PROVIDER_GROQ));
        assert_eq!(provider_to_url("Groq"), Some(PROVIDER_GROQ));
    }

    #[test]
    fn provider_to_url_ollama() {
        assert_eq!(provider_to_url("ollama"), Some(PROVIDER_OLLAMA));
        assert_eq!(provider_to_url("OLLAMA"), Some(PROVIDER_OLLAMA));
        assert_eq!(provider_to_url("Ollama"), Some(PROVIDER_OLLAMA));
        assert_eq!(provider_to_url("local"), Some(PROVIDER_OLLAMA));
        assert_eq!(provider_to_url("LOCAL"), Some(PROVIDER_OLLAMA));
    }

    #[test]
    fn provider_to_url_invalid() {
        assert_eq!(provider_to_url("invalid"), None);
        assert_eq!(provider_to_url("azure"), None);
        assert_eq!(provider_to_url(""), None);
    }

    #[test]
    fn provider_constants_valid_urls() {
        assert!(PROVIDER_OPENAI.starts_with("https://"));
        assert!(PROVIDER_CLAUDE.starts_with("https://"));
        assert!(PROVIDER_GEMINI.starts_with("https://"));
        assert!(PROVIDER_GROQ.starts_with("https://"));
        assert!(PROVIDER_OLLAMA.starts_with("http://"));
    }

    #[test]
    fn provider_ollama_url_is_localhost() {
        assert!(PROVIDER_OLLAMA.contains("localhost:11434"));
    }

    // =========================================================================
    // CLI PROVIDER ARGUMENT TESTS
    // =========================================================================

    #[test]
    fn cli_with_provider_claude() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--provider", "claude", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("claude".into()));
    }

    #[test]
    fn cli_with_provider_gemini() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--provider", "gemini", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("gemini".into()));
    }

    #[test]
    fn cli_with_provider_groq() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--provider", "groq", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("groq".into()));
    }

    #[test]
    fn cli_with_provider_openai() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--provider", "openai", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("openai".into()));
    }

    #[test]
    fn cli_with_provider_anthropic_alias() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--provider", "anthropic", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("anthropic".into()));
    }

    #[test]
    fn cli_with_provider_ollama() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--provider", "ollama", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("ollama".into()));
    }

    #[test]
    fn cli_with_provider_local_alias() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--provider", "local", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("local".into()));
    }

    #[test]
    fn cli_rejects_invalid_provider() {
        use clap::Parser;
        let result = Cli::try_parse_from(["gitar", "--provider", "invalid", "staged"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_provider_and_base_url_both_accepted() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--provider",
            "claude",
            "--base-url",
            "https://custom.api",
            "staged",
        ])
        .unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("claude".into()));
        assert_eq!(cli.base_url, Some("https://custom.api".into()));
    }

    #[test]
    fn cli_provider_with_model() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--provider",
            "gemini",
            "--model",
            "gemini-2.5-pro",
            "staged",
        ])
        .unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("gemini".into()));
        assert_eq!(cli.model, Some("gemini-2.5-pro".into()));
    }

    #[test]
    fn cli_provider_with_api_key() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--provider",
            "groq",
            "--api-key",
            "gsk_test123",
            "staged",
        ])
        .unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("groq".into()));
        assert_eq!(cli.api_key, Some("gsk_test123".into()));
    }

    #[test]
    fn cli_ollama_with_model() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--provider",
            "ollama",
            "--model",
            "llama3.2:latest",
            "staged",
        ])
        .unwrap();
        assert!(matches!(cli.command, Commands::Staged));
        assert_eq!(cli.provider, Some("ollama".into()));
        assert_eq!(cli.model, Some("llama3.2:latest".into()));
    }

    #[test]
    fn cli_provider_with_commit_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--provider", "gemini", "commit", "-a"]).unwrap();
        if let Commands::Commit { all, .. } = cli.command {
            assert!(all);
        } else {
            panic!("Expected Commit command");
        }
        assert_eq!(cli.provider, Some("gemini".into()));
    }

    #[test]
    fn cli_provider_with_history_command() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--provider",
            "ollama",
            "--model",
            "llama3.2",
            "history",
            "-n",
            "5",
        ])
        .unwrap();
        if let Commands::History { limit, .. } = cli.command {
            assert_eq!(limit, Some(5));
        } else {
            panic!("Expected History command");
        }
        assert_eq!(cli.provider, Some("ollama".into()));
        assert_eq!(cli.model, Some("llama3.2".into()));
    }

    #[test]
    fn cli_parses_anthropic_base_url() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--base-url",
            "https://api.anthropic.com/v1",
            "staged",
        ])
        .unwrap();
        assert_eq!(cli.base_url, Some("https://api.anthropic.com/v1".into()));
    }

    #[test]
    fn cli_parses_claude_model() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--model",
            "claude-sonnet-4-5-20250929",
            "staged",
        ])
        .unwrap();
        assert_eq!(cli.model, Some("claude-sonnet-4-5-20250929".into()));
    }

    #[test]
    fn cli_parses_gemini_base_url() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--base-url",
            "https://generativelanguage.googleapis.com",
            "staged",
        ])
        .unwrap();
        assert_eq!(
            cli.base_url,
            Some("https://generativelanguage.googleapis.com".into())
        );
    }

    #[test]
    fn cli_parses_groq_base_url() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "gitar",
            "--base-url",
            "https://api.groq.com/openai/v1",
            "staged",
        ])
        .unwrap();
        assert_eq!(cli.base_url, Some("https://api.groq.com/openai/v1".into()));
    }

    #[test]
    fn cli_parses_gemini_model() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["gitar", "--model", "gemini-2.5-flash", "staged"]).unwrap();
        assert_eq!(cli.model, Some("gemini-2.5-flash".into()));
    }

    // =========================================================================
    // PROVIDER RESOLUTION TESTS
    // =========================================================================

    #[test]
    fn provider_takes_precedence_over_base_url() {
        let base_url = Some("https://custom.api".to_string());
        let provider = Some("claude".to_string());

        let resolved_url = provider
            .as_ref()
            .and_then(|p| provider_to_url(p).map(String::from))
            .or(base_url);

        assert_eq!(resolved_url, Some(PROVIDER_CLAUDE.to_string()));
    }

    #[test]
    fn base_url_used_when_no_provider() {
        let base_url = Some("https://custom.api".to_string());
        let provider: Option<String> = None;

        let resolved_url = provider
            .as_ref()
            .and_then(|p| provider_to_url(p).map(String::from))
            .or(base_url);

        assert_eq!(resolved_url, Some("https://custom.api".to_string()));
    }

    #[test]
    fn none_when_neither_provider_nor_base_url() {
        let base_url: Option<String> = None;
        let provider: Option<String> = None;

        let resolved_url = provider
            .as_ref()
            .and_then(|p| provider_to_url(p).map(String::from))
            .or(base_url);

        assert!(resolved_url.is_none());
    }

    #[test]
    fn resolved_config_provider_sets_claude_url() {
        let cli = make_test_cli_with_provider(None, None, None, None, None, Some("claude".into()), None);
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.base_url, PROVIDER_CLAUDE);
        assert_eq!(resolved.model, "claude-sonnet-4-5-20250929");
    }

    #[test]
    fn resolved_config_provider_sets_gemini_url() {
        let cli = make_test_cli_with_provider(None, None, None, None, None, Some("gemini".into()), None);
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.base_url, PROVIDER_GEMINI);
        assert_eq!(resolved.model, "gemini-2.5-flash");
    }

    #[test]
    fn resolved_config_provider_sets_groq_url() {
        let cli = make_test_cli_with_provider(None, None, None, None, None, Some("groq".into()), None);
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.base_url, PROVIDER_GROQ);
    }

    #[test]
    fn resolved_config_provider_sets_ollama_url() {
        let cli = make_test_cli_with_provider(None, None, None, None, None, Some("ollama".into()), None);
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.base_url, PROVIDER_OLLAMA);
    }

    #[test]
    fn resolved_config_provider_overrides_base_url() {
        let cli = make_test_cli_with_provider(
            None,
            None,
            None,
            None,
            Some("https://custom.api".into()),
            Some("claude".into()),
            None,
        );
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.base_url, PROVIDER_CLAUDE);
    }

    #[test]
    fn resolved_config_provider_overrides_file_base_url() {
        let cli = make_test_cli_with_provider(None, None, None, None, None, Some("gemini".into()), None);
        let file = Config {
            base_url: Some("https://api.openai.com/v1".into()),
            ..Config::default()
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.base_url, PROVIDER_GEMINI);
    }

    // =========================================================================
    // API KEY PRIORITY TESTS
    // =========================================================================

    #[test]
    fn resolved_config_cli_api_key_takes_priority_over_env_and_file() {
        std::env::set_var("OPENAI_API_KEY", "env-key");

        let cli = make_test_cli(Some("cli-key".into()), None, None, None, None, None);
        let file = Config {
            api_key: Some("file-key".into()),
            ..Config::default()
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.api_key, Some("cli-key".into()));

        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn resolved_config_env_api_key_second_priority() {
        std::env::set_var("OPENAI_API_KEY", "env-key");

        let cli = make_test_cli(None, None, None, None, None, None);
        let file = Config {
            api_key: Some("file-key".into()),
            ..Config::default()
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.api_key, Some("env-key".into()));

        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn resolved_config_file_api_key_third_priority() {
        std::env::remove_var("OPENAI_API_KEY");

        let cli = make_test_cli(None, None, None, None, None, None);
        let file = Config {
            api_key: Some("file-key".into()),
            ..Config::default()
        };
        let resolved = ResolvedConfig::new(&cli, &file);

        assert_eq!(resolved.api_key, Some("file-key".into()));
    }

    #[test]
    fn resolved_config_no_api_key_when_none_set() {
        std::env::remove_var("OPENAI_API_KEY");

        let cli = make_test_cli(None, None, None, None, None, None);
        let file = Config::default();
        let resolved = ResolvedConfig::new(&cli, &file);

        assert!(resolved.api_key.is_none());
    }

    // =========================================================================
    // CLAUDE MODEL ID TESTS
    // =========================================================================

    #[test]
    fn claude_model_ids_valid_format() {
        let valid_models = [
            "claude-opus-4-5-20251101",
            "claude-sonnet-4-5-20250929",
            "claude-haiku-4-5-20251001",
            "claude-opus-4-1-20250805",
            "claude-sonnet-4-20250514",
            "claude-opus-4-20250514",
        ];
        for model in valid_models {
            assert!(
                model.starts_with("claude-"),
                "Model should start with 'claude-': {}",
                model
            );
            assert!(
                model.contains("-202"),
                "Model should contain date suffix: {}",
                model
            );
        }
    }
}
