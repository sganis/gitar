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
        // Core verbs
        vec!["gitar", "plan"],
        vec!["gitar", "fix"],
        vec!["gitar", "release"],
        // Tell with flags
        vec!["gitar", "tell", "--commit"],
        vec!["gitar", "tell", "--pr", "main"],
        vec!["gitar", "tell", "--changelog"],
        vec!["gitar", "tell", "--history"],
        vec!["gitar", "tell", "--explain"],
        vec!["gitar", "tell"], // defaults to explain
        // Utilities
        vec!["gitar", "init"],
        vec!["gitar", "init", "--show"],
        vec!["gitar", "models"],
        vec!["gitar", "diff"],
        vec!["gitar", "squash", "3"],
        vec!["gitar", "rewrite", "5"],
        vec!["gitar", "hook", "--install"],
        vec!["gitar", "hook", "--uninstall"],
        // Compatibility aliases
        vec!["gitar", "run"],
        vec!["gitar", "resolve"],
        vec!["gitar", "commit"],
        vec!["gitar", "pr"],
        vec!["gitar", "changelog"],
        vec!["gitar", "history"],
    ];

    for args in commands {
        let result = Cli::try_parse_from(&args);
        assert!(result.is_ok(), "Failed to parse: {:?}", args);
    }
}

#[test]
fn cli_default_command_is_plan() {
    // Running just "gitar" should default to plan command
    let cli = Cli::try_parse_from(["gitar"]).unwrap();
    assert!(cli.command.is_none(), "No command means default to plan");
}

// ==========================================================================
// Algorithm flag tests
// ==========================================================================

#[test]
fn cli_parses_algo_flag() {
    for algo_val in 1..=4 {
        let cli = Cli::try_parse_from([
            "gitar",
            "tell",
            "--commit",
            "--algo",
            &algo_val.to_string(),
        ])
        .unwrap();
        if let Some(Commands::Tell { algo, commit, .. }) = cli.command {
            assert!(commit);
            assert_eq!(algo, algo_val);
        }
    }
}

#[test]
fn cli_algo_defaults_to_4() {
    let cli = Cli::try_parse_from(["gitar", "tell", "--commit"]).unwrap();
    if let Some(Commands::Tell { algo, .. }) = cli.command {
        assert_eq!(algo, 4);
    }
}

#[test]
fn cli_rejects_invalid_algo() {
    assert!(Cli::try_parse_from(["gitar", "tell", "--commit", "--algo", "0"]).is_err());
    assert!(Cli::try_parse_from(["gitar", "tell", "--commit", "--algo", "5"]).is_err());
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
        let cli =
            Cli::try_parse_from(["gitar", "--provider", provider, "tell", "--staged"]).unwrap();
        assert_eq!(cli.provider, Some(provider.into()));
    }
}

#[test]
fn cli_rejects_invalid_provider() {
    assert!(
        Cli::try_parse_from(["gitar", "--provider", "invalid", "tell", "--staged"]).is_err()
    );
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
        let cli =
            Cli::try_parse_from(["gitar", "--preset", preset, "tell", "--staged"]).unwrap();
        assert_eq!(cli.preset, Some(preset.into()));
    }
}

#[test]
fn cli_rejects_invalid_preset() {
    assert!(Cli::try_parse_from(["gitar", "--preset", "invalid", "tell", "--staged"]).is_err());
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
        "tell",
        "--staged",
    ])
    .unwrap();
    assert_eq!(cli.model, Some("gpt-4".into()));
    assert_eq!(cli.max_tokens, Some(2048));
    assert_eq!(cli.temperature, Some(0.5));
    assert!(cli.stream);
}

// ==========================================================================
// Commit-specific flags tests (via tell --commit)
// ==========================================================================

#[test]
fn cli_parses_commit_flags() {
    let cli =
        Cli::try_parse_from(["gitar", "tell", "--commit", "-p", "-a", "--no-ai-author"]).unwrap();
    if let Some(Commands::Tell {
        commit,
        push,
        all,
        no_ai_author,
        ..
    }) = cli.command
    {
        assert!(commit);
        assert!(push);
        assert!(all);
        assert!(no_ai_author);
    } else {
        panic!("Expected Tell command with --commit flag");
    }
}

#[test]
fn cli_parses_commit_amend() {
    let cli = Cli::try_parse_from(["gitar", "tell", "--commit", "--amend"]).unwrap();
    if let Some(Commands::Tell { commit, amend, .. }) = cli.command {
        assert!(commit);
        assert!(amend);
    } else {
        panic!("Expected Tell command with --commit flag");
    }
}

// ==========================================================================
// Diff command tests
// ==========================================================================

#[test]
fn cli_parses_diff_options() {
    let cli = Cli::try_parse_from(["gitar", "diff", "--compare", "--stats"]).unwrap();
    if let Some(Commands::Diff { compare, stats, .. }) = cli.command {
        assert!(compare);
        assert!(stats);
    } else {
        panic!("Expected Diff command");
    }
}

#[test]
fn cli_parses_diff_staged() {
    let cli = Cli::try_parse_from(["gitar", "diff", "--staged"]).unwrap();
    if let Some(Commands::Diff { staged, .. }) = cli.command {
        assert!(staged);
    } else {
        panic!("Expected Diff command");
    }
}

// ==========================================================================
// Fix command tests
// ==========================================================================

#[test]
fn cli_parses_fix_flags() {
    let cli = Cli::try_parse_from(["gitar", "fix", "--apply", "--yes", "--stream"]).unwrap();
    if let Some(Commands::Fix { apply, yes, stream, .. }) = cli.command {
        assert!(apply);
        assert!(yes);
        assert!(stream);
    } else {
        panic!("Expected Fix command");
    }
}

// ==========================================================================
// Plan command tests
// ==========================================================================

#[test]
fn cli_parses_plan_flags() {
    let cli = Cli::try_parse_from(["gitar", "plan", "--apply", "--suggest"]).unwrap();
    if let Some(Commands::Plan { apply, suggest, .. }) = cli.command {
        assert!(apply);
        assert!(suggest);
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn cli_parses_plan_scope_flags() {
    // --working
    let cli = Cli::try_parse_from(["gitar", "plan", "--working"]).unwrap();
    if let Some(Commands::Plan { working, .. }) = cli.command {
        assert!(working);
    } else {
        panic!("Expected Plan command");
    }

    // --staged
    let cli = Cli::try_parse_from(["gitar", "plan", "--staged"]).unwrap();
    if let Some(Commands::Plan { staged, .. }) = cli.command {
        assert!(staged);
    } else {
        panic!("Expected Plan command");
    }

    // --history
    let cli = Cli::try_parse_from(["gitar", "plan", "--history", "v1.0.0"]).unwrap();
    if let Some(Commands::Plan { history, .. }) = cli.command {
        assert_eq!(history, Some("v1.0.0".to_string()));
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn cli_parses_plan_algo() {
    let cli = Cli::try_parse_from(["gitar", "plan", "--algo", "3"]).unwrap();
    if let Some(Commands::Plan { algo, .. }) = cli.command {
        assert_eq!(algo, 3);
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn cli_parses_plan_fix() {
    let cli = Cli::try_parse_from(["gitar", "plan", "--fix"]).unwrap();
    if let Some(Commands::Plan { fix, .. }) = cli.command {
        assert!(fix);
    } else {
        panic!("Expected Plan command");
    }
}

// ==========================================================================
// Release command tests
// ==========================================================================

#[test]
fn cli_parses_release_flags() {
    let cli = Cli::try_parse_from(["gitar", "release", "--apply", "--bump", "minor"]).unwrap();
    if let Some(Commands::Release { apply, bump, .. }) = cli.command {
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
    if let Some(Commands::Release {
        skip_changelog,
        changelog_file,
        ..
    }) = cli.command
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
    if let Some(Commands::Squash { target, .. }) = cli.command {
        assert_eq!(target, "5");
    } else {
        panic!("Expected Squash command");
    }
}

#[test]
fn cli_parses_rewrite_target() {
    let cli = Cli::try_parse_from(["gitar", "rewrite", "HEAD~10"]).unwrap();
    if let Some(Commands::Rewrite { target, .. }) = cli.command {
        assert_eq!(target, "HEAD~10");
    } else {
        panic!("Expected Rewrite command");
    }
}

// ==========================================================================
// History flag tests (via tell --history)
// ==========================================================================

#[test]
fn cli_parses_history_options() {
    let cli = Cli::try_parse_from([
        "gitar",
        "tell",
        "--history",
        "v1.0.0",
        "--to",
        "HEAD",
        "--limit",
        "10",
        "--delay",
        "1000",
    ])
    .unwrap();
    if let Some(Commands::Tell {
        history,
        reference,
        to,
        limit,
        delay,
        ..
    }) = cli.command
    {
        assert!(history);
        assert_eq!(reference, Some("v1.0.0".to_string()));
        assert_eq!(to, Some("HEAD".to_string()));
        assert_eq!(limit, Some(10));
        assert_eq!(delay, 1000);
    } else {
        panic!("Expected Tell command with --history flag");
    }
}

// ==========================================================================
// Tell explain tests
// ==========================================================================

#[test]
fn cli_parses_tell_explain() {
    let cli = Cli::try_parse_from(["gitar", "tell", "--explain", "--staged"]).unwrap();
    if let Some(Commands::Tell {
        explain, staged, ..
    }) = cli.command
    {
        assert!(explain);
        assert!(staged);
    } else {
        panic!("Expected Tell command with --explain flag");
    }
}

#[test]
fn cli_tell_defaults_to_explain() {
    // Just "gitar tell" should work (defaults to explain behavior)
    let cli = Cli::try_parse_from(["gitar", "tell"]).unwrap();
    if let Some(Commands::Tell { .. }) = cli.command {
        // Success - no selector flag required
    } else {
        panic!("Expected Tell command");
    }
}

// ==========================================================================
// Selector flags are mutually exclusive
// ==========================================================================

#[test]
fn cli_rejects_multiple_selectors() {
    // Cannot use --commit and --pr together
    assert!(Cli::try_parse_from(["gitar", "tell", "--commit", "--pr"]).is_err());
    assert!(Cli::try_parse_from(["gitar", "tell", "--changelog", "--history"]).is_err());
}

// ==========================================================================
// Compatibility alias tests
// ==========================================================================

#[test]
fn cli_parses_run_alias() {
    let cli = Cli::try_parse_from(["gitar", "run", "--apply"]).unwrap();
    if let Some(Commands::Run { apply, .. }) = cli.command {
        assert!(apply);
    } else {
        panic!("Expected Run alias");
    }
}

#[test]
fn cli_parses_resolve_alias() {
    let cli = Cli::try_parse_from(["gitar", "resolve", "--apply"]).unwrap();
    if let Some(Commands::Resolve { apply, .. }) = cli.command {
        assert!(apply);
    } else {
        panic!("Expected Resolve alias");
    }
}

#[test]
fn cli_parses_commit_alias() {
    let cli = Cli::try_parse_from(["gitar", "commit", "-p", "-a"]).unwrap();
    if let Some(Commands::Commit { push, all, .. }) = cli.command {
        assert!(push);
        assert!(all);
    } else {
        panic!("Expected Commit alias");
    }
}

// ==========================================================================
// Hook command tests (flag-based)
// ==========================================================================

#[test]
fn cli_parses_hook_install() {
    let cli = Cli::try_parse_from(["gitar", "hook", "--install"]).unwrap();
    if let Some(Commands::Hook { install, uninstall }) = cli.command {
        assert!(install);
        assert!(!uninstall);
    } else {
        panic!("Expected Hook command");
    }
}

#[test]
fn cli_parses_hook_uninstall() {
    let cli = Cli::try_parse_from(["gitar", "hook", "--uninstall"]).unwrap();
    if let Some(Commands::Hook { install, uninstall }) = cli.command {
        assert!(!install);
        assert!(uninstall);
    } else {
        panic!("Expected Hook command");
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

#[test]
fn hook_script_uses_tell_command() {
    // Hook script should use "gitar tell --commit"
    assert!(
        HOOK_SCRIPT.contains("gitar tell --commit"),
        "Hook should use tell --commit command"
    );
}

// ==========================================================================
// Single-letter alias tests
// ==========================================================================

#[test]
fn cli_parses_plan_alias_p() {
    let cli = Cli::try_parse_from(["gitar", "p", "--apply"]).unwrap();
    if let Some(Commands::Plan { apply, .. }) = cli.command {
        assert!(apply);
    } else {
        panic!("Expected Plan command via 'p' alias");
    }
}

#[test]
fn cli_parses_tell_alias_t() {
    let cli = Cli::try_parse_from(["gitar", "t", "--commit"]).unwrap();
    if let Some(Commands::Tell { commit, .. }) = cli.command {
        assert!(commit);
    } else {
        panic!("Expected Tell command via 't' alias");
    }
}

#[test]
fn cli_parses_fix_alias_f() {
    let cli = Cli::try_parse_from(["gitar", "f", "--apply"]).unwrap();
    if let Some(Commands::Fix { apply, .. }) = cli.command {
        assert!(apply);
    } else {
        panic!("Expected Fix command via 'f' alias");
    }
}

#[test]
fn cli_parses_release_alias_r() {
    let cli = Cli::try_parse_from(["gitar", "r", "--apply"]).unwrap();
    if let Some(Commands::Release { apply, .. }) = cli.command {
        assert!(apply);
    } else {
        panic!("Expected Release command via 'r' alias");
    }
}

// ==========================================================================
// Dry-run flag tests
// ==========================================================================

#[test]
fn cli_parses_plan_dry_run() {
    let cli = Cli::try_parse_from(["gitar", "plan", "--dry-run"]).unwrap();
    if let Some(Commands::Plan { dry_run, apply, .. }) = cli.command {
        assert!(dry_run);
        assert!(!apply);
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn cli_parses_fix_dry_run() {
    let cli = Cli::try_parse_from(["gitar", "fix", "--dry-run"]).unwrap();
    if let Some(Commands::Fix { dry_run, apply, .. }) = cli.command {
        assert!(dry_run);
        assert!(!apply);
    } else {
        panic!("Expected Fix command");
    }
}

#[test]
fn cli_parses_release_dry_run() {
    let cli = Cli::try_parse_from(["gitar", "release", "--dry-run"]).unwrap();
    if let Some(Commands::Release { dry_run, apply, .. }) = cli.command {
        assert!(dry_run);
        assert!(!apply);
    } else {
        panic!("Expected Release command");
    }
}

// ==========================================================================
// Init command tests
// ==========================================================================

#[test]
fn cli_parses_init_auto_apply_true() {
    let cli = Cli::try_parse_from(["gitar", "init", "--auto-apply", "true"]).unwrap();
    if let Some(Commands::Init { auto_apply, show }) = cli.command {
        assert_eq!(auto_apply, Some(true));
        assert!(!show);
    } else {
        panic!("Expected Init command");
    }
}

#[test]
fn cli_parses_init_auto_apply_false() {
    let cli = Cli::try_parse_from(["gitar", "init", "--auto-apply", "false"]).unwrap();
    if let Some(Commands::Init { auto_apply, show }) = cli.command {
        assert_eq!(auto_apply, Some(false));
        assert!(!show);
    } else {
        panic!("Expected Init command");
    }
}

#[test]
fn cli_parses_init_show() {
    let cli = Cli::try_parse_from(["gitar", "init", "--show"]).unwrap();
    if let Some(Commands::Init { auto_apply, show }) = cli.command {
        assert!(show);
        assert!(auto_apply.is_none());
    } else {
        panic!("Expected Init command");
    }
}
