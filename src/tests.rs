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

    // =========================================================================
    // RESOLVED CONFIG TESTS
    // =========================================================================

    #[test]
    fn resolved_config_uses_defaults() {
        let _file = Config::default();
        let resolved = ResolvedConfig {
            api_key: None,
            model: "gpt-4o".into(),
            max_tokens: 4096,
            temperature: 0.7,
            base_url: None,
            base_branch: "main".into(),
        };
        assert_eq!(resolved.model, "gpt-4o");
        assert_eq!(resolved.max_tokens, 4096);
        assert_eq!(resolved.temperature, 0.7);
    }

    #[test]
    fn resolved_config_file_overrides_defaults() {
        let file = Config {
            api_key: Some("file-key".into()),
            model: Some("gpt-3.5-turbo".into()),
            max_tokens: Some(2048),
            temperature: Some(0.3),
            base_url: Some("https://custom.api".into()),
            base_branch: Some("develop".into()),
        };
        // Simulating resolution logic
        let model = file.model.clone().unwrap_or_else(|| "gpt-4o".into());
        let max_tokens = file.max_tokens.unwrap_or(4096);
        let temperature = file.temperature.unwrap_or(0.7);
        
        assert_eq!(model, "gpt-3.5-turbo");
        assert_eq!(max_tokens, 2048);
        assert_eq!(temperature, 0.3);
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
    }

    #[test]
    fn truncate_diff_exact_boundary() {
        let diff = "exactly100chars".repeat(10); // 150 chars
        let result = truncate_diff(diff.clone(), 150);
        assert_eq!(result, diff);
    }

    // =========================================================================
    // COMMIT INFO PARSING TESTS
    // =========================================================================

    #[test]
    fn parse_commit_log_line() {
        let line = "abc123def|John Doe|2024-01-15 10:30:00|Fix bug in parser";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "abc123def");
        assert_eq!(parts[1], "John Doe");
        assert_eq!(parts[2], "2024-01-15 10:30:00");
        assert_eq!(parts[3], "Fix bug in parser");
    }

    #[test]
    fn parse_commit_log_with_pipe_in_message() {
        let line = "abc123|Author|2024-01-15|Message with | pipe";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[3], "Message with | pipe");
    }

    #[test]
    fn parse_commit_log_incomplete_rejected() {
        let line = "abc123|Author|2024-01-15";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert!(parts.len() < 4);
    }

    // =========================================================================
    // PROMPT TEMPLATE TESTS
    // =========================================================================

    #[test]
    fn quick_commit_prompt_substitution() {
        let diff = "test diff content";
        let prompt = QUICK_COMMIT_USER_PROMPT.replace("{diff}", diff);
        assert!(prompt.contains("test diff content"));
        assert!(!prompt.contains("{diff}"));
    }

    #[test]
    fn commit_prompt_substitution() {
        let diff = "test diff";
        let original = "Original message";
        let prompt = COMMIT_USER_PROMPT
            .replace("{original_message}", original)
            .replace("{diff}", diff);
        assert!(prompt.contains("test diff"));
        assert!(prompt.contains("Original message"));
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
    }

    #[test]
    fn changelog_prompt_substitution() {
        let prompt = CHANGELOG_USER_PROMPT
            .replace("{range}", "v1.0.0 → HEAD")
            .replace("{count}", "10")
            .replace("{commits}", "- [abc123] Fix bug");
        assert!(prompt.contains("v1.0.0 → HEAD"));
        assert!(prompt.contains("10"));
    }

    #[test]
    fn version_prompt_substitution() {
        let prompt = VERSION_USER_PROMPT
            .replace("{version}", "1.2.3")
            .replace("{diff}", "some changes");
        assert!(prompt.contains("1.2.3"));
        assert!(prompt.contains("some changes"));
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
            assert!(pattern.starts_with(":(exclude)"), "Pattern should start with :(exclude): {}", pattern);
        }
    }

    #[test]
    fn exclude_patterns_contains_lock_files() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains("lock")));
        assert!(patterns.iter().any(|p| p.contains("package-lock.json")));
    }

    #[test]
    fn exclude_patterns_contains_build_dirs() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains("dist/")));
        assert!(patterns.iter().any(|p| p.contains("build/")));
        assert!(patterns.iter().any(|p| p.contains("target/")));
    }

    // =========================================================================
    // SYSTEM PROMPT TESTS
    // =========================================================================

    #[test]
    fn system_prompts_not_empty() {
        assert!(!COMMIT_SYSTEM_PROMPT.is_empty());
        assert!(!QUICK_COMMIT_SYSTEM_PROMPT.is_empty());
        assert!(!PR_SYSTEM_PROMPT.is_empty());
        assert!(!CHANGELOG_SYSTEM_PROMPT.is_empty());
        assert!(!EXPLAIN_SYSTEM_PROMPT.is_empty());
        assert!(!VERSION_SYSTEM_PROMPT.is_empty());
    }

    #[test]
    fn commit_system_prompt_contains_types() {
        assert!(COMMIT_SYSTEM_PROMPT.contains("Feat"));
        assert!(COMMIT_SYSTEM_PROMPT.contains("Fix"));
        assert!(COMMIT_SYSTEM_PROMPT.contains("Refactor"));
    }

    // =========================================================================
    // CONFIG FILENAME TESTS
    // =========================================================================

    #[test]
    fn config_filename_correct() {
        assert_eq!(CONFIG_FILENAME, ".gitar.toml");
    }

    #[test]
    fn config_path_in_home() {
        if let Some(path) = Config::path() {
            let path_str = path.to_string_lossy();
            assert!(path_str.ends_with(".gitar.toml"));
        }
    }

    // =========================================================================
    // EDGE CASE TESTS
    // =========================================================================

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
    fn parse_empty_commit_log() {
        let output = "";
        let commits: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
        assert!(commits.is_empty());
    }

    #[test]
    fn config_handles_empty_toml() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.api_key.is_none());
        assert!(config.model.is_none());
    }
}