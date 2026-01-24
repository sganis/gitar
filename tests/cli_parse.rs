// tests/cli_parse.rs
//! CLI argument parsing tests
//!
//! Tests for command-line argument parsing, algorithm flags, provider validation,
//! preset validation, and global options.

use clap::Parser;
use gitar::cli::{Cli, Commands, HOOK_SCRIPT};

// ==========================================================================
// Core command parsing tests
// ==========================================================================

#[test]
fn cli_parses_all_commands() {
    let commands = [
        vec!["gitar", "commit"],
        vec!["gitar", "staged"],
        vec!["gitar", "unstaged"],
        vec!["gitar", "pr", "main"],
        vec!["gitar", "changelog"],
        vec!["gitar", "explain"],
        vec!["gitar", "history"],
        vec!["gitar", "init"],
        vec!["gitar", "config"],
        vec!["gitar", "models"],
        vec!["gitar", "diff"],
        vec!["gitar", "resolve"],
        vec!["gitar", "run"],
        vec!["gitar", "release"],
        vec!["gitar", "squash", "3"],
        vec!["gitar", "rewrite", "5"],
        vec!["gitar", "hook", "install"],
        vec!["gitar", "hook", "uninstall"],
    ];

    for args in commands {
        let result = Cli::try_parse_from(&args);
        assert!(result.is_ok(), "Failed to parse: {:?}", args);
    }
}

// ==========================================================================
// Algorithm flag tests
// ==========================================================================

#[test]
fn cli_parses_algo_flag() {
    for algo_val in 1..=4 {
        let cli =
            Cli::try_parse_from(["gitar", "commit", "--algo", &algo_val.to_string()]).unwrap();
        if let Commands::Commit { algo, .. } = cli.command {
            assert_eq!(algo, algo_val);
        }
    }
}

#[test]
fn cli_algo_defaults_to_4() {
    let cli = Cli::try_parse_from(["gitar", "commit"]).unwrap();
    if let Commands::Commit { algo, .. } = cli.command {
        assert_eq!(algo, 4);
    }
}

#[test]
fn cli_rejects_invalid_algo() {
    assert!(Cli::try_parse_from(["gitar", "commit", "--algo", "0"]).is_err());
    assert!(Cli::try_parse_from(["gitar", "commit", "--algo", "5"]).is_err());
}

// ==========================================================================
// Provider tests
// ==========================================================================

#[test]
fn cli_parses_valid_providers() {
    let providers = [
        "openai", "claude", "gemini", "google", "groq", "ollama", "local",
    ];
    for provider in providers {
        let cli = Cli::try_parse_from(["gitar", "--provider", provider, "staged"]).unwrap();
        assert_eq!(cli.provider, Some(provider.into()));
    }
}

#[test]
fn cli_rejects_invalid_provider() {
    assert!(Cli::try_parse_from(["gitar", "--provider", "invalid", "staged"]).is_err());
}

// ==========================================================================
// Preset tests
// ==========================================================================

#[test]
fn cli_parses_valid_presets() {
    let presets = [
        "rust",
        "rs",
        "javascript",
        "js",
        "python",
        "py",
        "auto",
        "default",
    ];
    for preset in presets {
        let cli = Cli::try_parse_from(["gitar", "--preset", preset, "staged"]).unwrap();
        assert_eq!(cli.preset, Some(preset.into()));
    }
}

#[test]
fn cli_rejects_invalid_preset() {
    assert!(Cli::try_parse_from(["gitar", "--preset", "invalid", "staged"]).is_err());
}

// ==========================================================================
// Global options tests
// ==========================================================================

#[test]
fn cli_parses_global_options() {
    let cli = Cli::try_parse_from([
        "gitar",
        "--model",
        "gpt-4",
        "--max-tokens",
        "2048",
        "--temperature",
        "0.5",
        "--stream",
        "staged",
    ])
    .unwrap();
    assert_eq!(cli.model, Some("gpt-4".into()));
    assert_eq!(cli.max_tokens, Some(2048));
    assert_eq!(cli.temperature, Some(0.5));
    assert!(cli.stream);
}

// ==========================================================================
// Commit-specific flags tests
// ==========================================================================

#[test]
fn cli_parses_commit_flags() {
    let cli = Cli::try_parse_from(["gitar", "commit", "-p", "-a", "--no-tag"]).unwrap();
    if let Commands::Commit {
        push, all, no_tag, ..
    } = cli.command
    {
        assert!(push);
        assert!(all);
        assert!(no_tag);
    } else {
        panic!("Expected Commit command");
    }
}

#[test]
fn cli_parses_commit_amend() {
    let cli = Cli::try_parse_from(["gitar", "commit", "--amend"]).unwrap();
    if let Commands::Commit { amend, .. } = cli.command {
        assert!(amend);
    } else {
        panic!("Expected Commit command");
    }
}

// ==========================================================================
// Diff command tests
// ==========================================================================

#[test]
fn cli_parses_diff_options() {
    let cli = Cli::try_parse_from(["gitar", "diff", "--compare", "--stats"]).unwrap();
    if let Commands::Diff { compare, stats, .. } = cli.command {
        assert!(compare);
        assert!(stats);
    } else {
        panic!("Expected Diff command");
    }
}

#[test]
fn cli_parses_diff_staged() {
    let cli = Cli::try_parse_from(["gitar", "diff", "--staged"]).unwrap();
    if let Commands::Diff { staged, .. } = cli.command {
        assert!(staged);
    } else {
        panic!("Expected Diff command");
    }
}

// ==========================================================================
// Resolve command tests
// ==========================================================================

#[test]
fn cli_parses_resolve_flags() {
    let cli = Cli::try_parse_from(["gitar", "resolve", "--apply", "--yes", "--stream"]).unwrap();
    if let Commands::Resolve { apply, yes, stream } = cli.command {
        assert!(apply);
        assert!(yes);
        assert!(stream);
    } else {
        panic!("Expected Resolve command");
    }
}

// ==========================================================================
// Run command tests
// ==========================================================================

#[test]
fn cli_parses_run_flags() {
    let cli = Cli::try_parse_from(["gitar", "run", "--apply", "--suggest"]).unwrap();
    if let Commands::Run { apply, suggest, .. } = cli.command {
        assert!(apply);
        assert!(suggest);
    } else {
        panic!("Expected Run command");
    }
}

#[test]
fn cli_parses_run_mode() {
    let cli = Cli::try_parse_from(["gitar", "run", "--mode", "working"]).unwrap();
    if let Commands::Run { mode, .. } = cli.command {
        assert_eq!(mode, Some("working".to_string()));
    } else {
        panic!("Expected Run command");
    }
}

#[test]
fn cli_parses_run_algo() {
    let cli = Cli::try_parse_from(["gitar", "run", "--algo", "3"]).unwrap();
    if let Commands::Run { algo, .. } = cli.command {
        assert_eq!(algo, 3);
    } else {
        panic!("Expected Run command");
    }
}

#[test]
fn cli_parses_run_resolve() {
    let cli = Cli::try_parse_from(["gitar", "run", "--resolve"]).unwrap();
    if let Commands::Run { resolve, .. } = cli.command {
        assert!(resolve);
    } else {
        panic!("Expected Run command");
    }
}

// ==========================================================================
// Release command tests
// ==========================================================================

#[test]
fn cli_parses_release_flags() {
    let cli = Cli::try_parse_from(["gitar", "release", "--apply", "--bump", "minor"]).unwrap();
    if let Commands::Release { apply, bump, .. } = cli.command {
        assert!(apply);
        assert_eq!(bump, "minor");
    } else {
        panic!("Expected Release command");
    }
}

#[test]
fn cli_parses_release_changelog_options() {
    let cli = Cli::try_parse_from([
        "gitar",
        "release",
        "--skip-changelog",
        "--changelog-file",
        "HISTORY.md",
    ])
    .unwrap();
    if let Commands::Release {
        skip_changelog,
        changelog_file,
        ..
    } = cli.command
    {
        assert!(skip_changelog);
        assert_eq!(changelog_file, "HISTORY.md");
    } else {
        panic!("Expected Release command");
    }
}

// ==========================================================================
// Squash and Rewrite command tests
// ==========================================================================

#[test]
fn cli_parses_squash_target() {
    let cli = Cli::try_parse_from(["gitar", "squash", "5"]).unwrap();
    if let Commands::Squash { target, .. } = cli.command {
        assert_eq!(target, "5");
    } else {
        panic!("Expected Squash command");
    }
}

#[test]
fn cli_parses_rewrite_target() {
    let cli = Cli::try_parse_from(["gitar", "rewrite", "HEAD~10"]).unwrap();
    if let Commands::Rewrite { target, .. } = cli.command {
        assert_eq!(target, "HEAD~10");
    } else {
        panic!("Expected Rewrite command");
    }
}

// ==========================================================================
// History command tests
// ==========================================================================

#[test]
fn cli_parses_history_options() {
    let cli = Cli::try_parse_from([
        "gitar", "history", "v1.0.0", "--to", "HEAD", "--limit", "10", "--delay", "1000",
    ])
    .unwrap();
    if let Commands::History {
        from,
        to,
        limit,
        delay,
        ..
    } = cli.command
    {
        assert_eq!(from, Some("v1.0.0".to_string()));
        assert_eq!(to, Some("HEAD".to_string()));
        assert_eq!(limit, Some(10));
        assert_eq!(delay, 1000);
    } else {
        panic!("Expected History command");
    }
}

// ==========================================================================
// Hook script tests
// ==========================================================================

#[test]
fn hook_script_contains_required_elements() {
    assert!(HOOK_SCRIPT.contains("gitar-hook"), "Should contain marker");
    assert!(
        HOOK_SCRIPT.contains("COMMIT_SOURCE"),
        "Should check commit source"
    );
    assert!(
        HOOK_SCRIPT.contains("command -v gitar"),
        "Should check gitar installed"
    );
    assert!(
        HOOK_SCRIPT.contains("--write-to"),
        "Should use write-to flag"
    );
}

#[test]
fn hook_script_is_valid_shell() {
    assert!(HOOK_SCRIPT.starts_with("#!/bin/sh"), "Should have shebang");
    assert!(HOOK_SCRIPT.contains("exit 0"), "Should have exit");
}
