// src/cli.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gitar",
    version,
    about = "AI-powered Git assistant\n\nAll commands accept an optional [REF] argument (tag, commit, branch) to specify the starting point.",
    after_help = "EXAMPLES:
    gitar changelog v1.0.0          # Release notes since tag
    gitar changelog HEAD~10         # Release notes for last 10 commits
    gitar changelog --since '1 week ago'

    gitar history v1.0.0            # Generate messages since tag
    gitar history -n 5              # Generate for last 5 commits

    gitar explain v1.0.0            # Explain changes since tag
    gitar explain --staged          # Explain staged changes

    gitar pr develop                # PR description against develop
    gitar pr --staged               # PR from staged changes

    gitar hook install              # Install git hook for auto-commit messages
    gitar hook uninstall            # Remove gitar git hook

    gitar version v1.0.0            # Version bump since tag

    gitar diff --compare            # Compare smart diff algorithms
    gitar commit --alg 3            # Use hunk-level analysis for large refactors

DIFF ALGORITHMS:
    --alg 1    Full: complete git diff (ignores --max-chars)
    --alg 2    Files: selective files, ranked by priority (default)
    --alg 3    Hunks: selective hunks, ranked by importance
    --alg 4    Semantic: JSON IR with scored hunks (token-efficient)"
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub api_key: Option<String>,
    #[arg(long, global = true)]
    pub model: Option<String>,
    #[arg(long, global = true)]
    pub max_tokens: Option<u32>,
    #[arg(long, global = true)]
    pub temperature: Option<f32>,
    #[arg(long, env = "OPENAI_BASE_URL", global = true)]
    pub base_url: Option<String>,
    #[arg(long, global = true)]
    pub base_branch: Option<String>,
    #[arg(
        long,
        global = true,
        value_parser = ["openai", "claude", "gemini", "google", "groq", "ollama", "local"]
    )]
    pub provider: Option<String>,

    /// Stream responses to stdout (when supported by the provider).
    #[arg(long, global = true, default_value_t = false)]
    pub stream: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a commit with an AI-generated message
    ///
    /// By default this will generate a message from staged changes, then run `git commit`.
    /// Use `-a` to stage all changes first, and `-p` to push after committing.
    Commit {
        /// Push after committing
        #[arg(short = 'p', long)]
        push: bool,

        /// Stage all changes before committing (`git add -A`)
        #[arg(short = 'a', long)]
        all: bool,

        /// Add AI model/provider tag to the commit message (default: true)
        #[arg(long, default_value = "true")]
        tag: bool,

        /// Do not add AI model/provider tag to the commit message
        #[arg(long = "no-tag")]
        no_tag: bool,

        /// Write commit message to file instead of committing (used by git hooks)
        #[arg(long, hide = true)]
        write_to: Option<String>,

        /// Suppress interactive prompts (used by git hooks)
        #[arg(long, hide = true)]
        silent: bool,

        /// Stream (per-command override). If set, enables streaming for this command.
        /// Global --stream also enables streaming.
        #[arg(long, default_value = "false")]
        stream: bool,

        /// Diff algorithm: 1=naive, 2=standard, 3=think, 4=ir
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=4))]
        alg: u8,
    },

    /// Generate an AI commit message for currently staged changes
    ///
    /// Prints the message to stdout (does not create a commit).
    Staged {
        /// Diff algorithm: 1=naive, 2=standard, 3=think, 4=ir
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=4))]
        alg: u8,
    },

    /// Generate an AI commit message for unstaged working tree changes
    ///
    /// Prints the message to stdout (does not create a commit).
    Unstaged {
        /// Diff algorithm: 1=naive, 2=standard, 3=think, 4=ir
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=4))]
        alg: u8,
    },

    /// Describe a range of commits in plain English (does not modify history)
    ///
    /// Useful for understanding what happened between two refs or within a time window.
    /// Note: this command does NOT rewrite commits or create new commits.
    History {
        /// Starting ref (tag, commit, branch). If omitted, defaults to recent commits.
        #[arg(value_name = "REF")]
        from: Option<String>,

        /// Ending ref (default: HEAD)
        #[arg(long)]
        to: Option<String>,

        /// Only include commits after this date (git date formats supported)
        #[arg(long)]
        since: Option<String>,

        /// Only include commits before this date (git date formats supported)
        #[arg(long)]
        until: Option<String>,

        /// Maximum number of commits to process (default: 50 when no FROM ref is given)
        #[arg(short = 'n', long)]
        limit: Option<usize>,

        /// Delay between API calls in milliseconds (useful to avoid rate limits)
        #[arg(long, default_value = "500")]
        delay: u64,

        /// Diff algorithm: 1=naive, 2=standard, 3=think, 4=ir
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=4))]
        alg: u8,
    },

    /// Generate a pull request description from branch changes
    ///
    /// Compares your current HEAD against BASE (or configured base branch).
    /// Use `--staged` to generate from staged changes only.
    Pr {
        /// Base ref to compare against (default: configured base branch, e.g. main)
        #[arg(value_name = "REF")]
        base: Option<String>,

        /// Ending ref (default: HEAD)
        #[arg(long)]
        to: Option<String>,

        /// Use staged changes only instead of comparing refs
        #[arg(long)]
        staged: bool,

        /// Diff algorithm: 1=naive, 2=standard, 3=think, 4=ir
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=4))]
        alg: u8,
    },

    /// Generate release notes (changelog) from a commit range
    ///
    /// Useful for GitHub Releases. Outputs markdown-ready text.
    Changelog {
        /// Starting ref (tag, commit, branch)
        #[arg(value_name = "REF")]
        from: Option<String>,

        /// Ending ref (default: HEAD)
        #[arg(long)]
        to: Option<String>,

        /// Only include commits after this date (git date formats supported)
        #[arg(long)]
        since: Option<String>,

        /// Only include commits before this date (git date formats supported)
        #[arg(long)]
        until: Option<String>,

        /// Maximum number of commits to include
        #[arg(short = 'n', long)]
        limit: Option<usize>,

        /// Diff algorithm: 1=naive, 2=standard, 3=think, 4=ir
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=4))]
        alg: u8,
    },

    /// Explain changes in plain English for non-technical stakeholders
    ///
    /// Can explain a commit range or staged changes (`--staged`).
    Explain {
        /// Starting ref (tag, commit, branch)
        #[arg(value_name = "REF")]
        from: Option<String>,

        /// Ending ref (default: HEAD)
        #[arg(long)]
        to: Option<String>,

        /// Only include commits after this date (git date formats supported)
        #[arg(long)]
        since: Option<String>,

        /// Only include commits before this date (git date formats supported)
        #[arg(long)]
        until: Option<String>,

        /// Explain staged changes only (ignores ref range)
        #[arg(long)]
        staged: bool,

        /// Diff algorithm: 1=naive, 2=standard, 3=think, 4=ir
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=4))]
        alg: u8,
    },

    /// Suggest a semantic version bump (major/minor/patch) from changes
    ///
    /// Optionally provide the current version to influence the recommendation.
    Version {
        /// Base ref to compare against (tag, commit, branch)
        #[arg(value_name = "REF")]
        base: Option<String>,

        /// Ending ref (default: HEAD)
        #[arg(long)]
        to: Option<String>,

        /// Current version (e.g. 1.2.3) used to contextualize the bump suggestion
        #[arg(long)]
        current: Option<String>,

        /// Diff algorithm: 1=naive, 2=standard, 3=think, 4=ir
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=4))]
        alg: u8,
    },

    /// Manage git hooks for automatic commit message generation
    Hook {
        #[command(subcommand)]
        command: HookCommands,
    },

    /// Create or update `~/.gitar.toml` with provider/model defaults
    Init,

    /// Show the resolved configuration and where each value comes from
    Config,

    /// List available models (when the provider exposes a models endpoint)
    Models,

    /// Debug: Preview what would be sent to the LLM
    Diff {
        /// Git diff target (branch, commit, etc.)
        target: Option<String>,

        /// Show staged changes only
        #[arg(long)]
        staged: bool,

        /// Maximum characters to send
        #[arg(long, default_value = "15000")]
        max_chars: usize,

        /// Diff algorithm: 1=naive, 2=standard, 3=think, 4=ir
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
        alg: Option<u8>,

        /// Include git diff --stat header
        #[arg(long)]
        stats: bool,

        /// Show stats only (no diff output)
        #[arg(long)]
        stats_only: bool,

        /// Compare all algorithms side-by-side
        #[arg(long)]
        compare: bool,
    },
}

#[derive(Subcommand, Clone)]
pub enum HookCommands {
    /// Install the prepare-commit-msg hook
    Install,
    /// Uninstall the prepare-commit-msg hook
    Uninstall,
}

pub const HOOK_SCRIPT: &str = r#"#!/bin/sh
# gitar-hook: Auto-generated by gitar
# This script runs on Linux, macOS, and Windows (via Git Bash)

# Skip if gitar is not in PATH
if ! command -v gitar >/dev/null 2>&1; then
    exit 0
fi

COMMIT_MSG_FILE=$1
COMMIT_SOURCE=$2

# Skip if the user provided a message via -m, -F, or if it's a merge/squash
if [ -n "$COMMIT_SOURCE" ]; then
    exit 0
fi

# Run gitar to generate the message into the git commit file
gitar commit --write-to "$COMMIT_MSG_FILE" --silent
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_parses_commit_command() {
        let cli = Cli::try_parse_from(["gitar", "commit"]).unwrap();
        assert!(matches!(cli.command, Commands::Commit { .. }));
    }

    #[test]
    fn cli_parses_commit_with_alg() {
        let cli = Cli::try_parse_from(["gitar", "commit", "--alg", "3"]).unwrap();
        if let Commands::Commit { alg, .. } = cli.command {
            assert_eq!(alg, 3);
        } else {
            panic!("Expected Commit command");
        }
    }

    #[test]
    fn cli_parses_commit_default_alg() {
        let cli = Cli::try_parse_from(["gitar", "commit"]).unwrap();
        if let Commands::Commit { alg, .. } = cli.command {
            assert_eq!(alg, 2);
        } else {
            panic!("Expected Commit command");
        }
    }

    #[test]
    fn cli_parses_staged_with_alg() {
        let cli = Cli::try_parse_from(["gitar", "staged", "--alg", "4"]).unwrap();
        if let Commands::Staged { alg } = cli.command {
            assert_eq!(alg, 4);
        } else {
            panic!("Expected Staged command");
        }
    }

    #[test]
    fn cli_parses_pr_with_alg() {
        let cli = Cli::try_parse_from(["gitar", "pr", "main", "--alg", "3"]).unwrap();
        if let Commands::Pr { base, alg, .. } = cli.command {
            assert_eq!(base, Some("main".into()));
            assert_eq!(alg, 3);
        } else {
            panic!("Expected Pr command");
        }
    }

    #[test]
    fn cli_parses_diff_compare() {
        let cli = Cli::try_parse_from(["gitar", "diff", "--compare"]).unwrap();
        if let Commands::Diff { compare, .. } = cli.command {
            assert!(compare);
        } else {
            panic!("Expected Diff command");
        }
    }

    #[test]
    fn cli_parses_diff_with_alg() {
        let cli = Cli::try_parse_from(["gitar", "diff", "--alg", "1"]).unwrap();
        if let Commands::Diff { alg, .. } = cli.command {
            assert_eq!(alg, Some(1));
        } else {
            panic!("Expected Diff command");
        }
    }

    #[test]
    fn cli_rejects_invalid_alg() {
        let result = Cli::try_parse_from(["gitar", "commit", "--alg", "5"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_rejects_alg_zero() {
        let result = Cli::try_parse_from(["gitar", "commit", "--alg", "0"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_parses_commit_with_flags() {
        let cli = Cli::try_parse_from(["gitar", "commit", "-p", "-a"]).unwrap();
        if let Commands::Commit { push, all, .. } = cli.command {
            assert!(push);
            assert!(all);
        } else {
            panic!("Expected Commit command");
        }
    }

    #[test]
    fn cli_parses_global_stream_flag() {
        let cli = Cli::try_parse_from(["gitar", "--stream", "staged"]).unwrap();
        assert!(cli.stream);
        assert!(matches!(cli.command, Commands::Staged { .. }));
    }

    #[test]
    fn cli_parses_staged_command() {
        let cli = Cli::try_parse_from(["gitar", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged { .. }));
    }

    #[test]
    fn cli_parses_unstaged_command() {
        let cli = Cli::try_parse_from(["gitar", "unstaged"]).unwrap();
        assert!(matches!(cli.command, Commands::Unstaged { .. }));
    }

    #[test]
    fn cli_parses_pr_with_base() {
        let cli = Cli::try_parse_from(["gitar", "pr", "develop"]).unwrap();
        if let Commands::Pr { base, to, staged, .. } = cli.command {
            assert_eq!(base, Some("develop".into()));
            assert!(to.is_none());
            assert!(!staged);
        } else {
            panic!("Expected Pr command");
        }
    }

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
            "staged",
        ])
        .unwrap();
        assert_eq!(cli.model, Some("gpt-4".into()));
        assert_eq!(cli.max_tokens, Some(2048));
        assert_eq!(cli.temperature, Some(0.5));
    }

    #[test]
    fn cli_parses_init_command() {
        let cli = Cli::try_parse_from([
            "gitar",
            "--model",
            "claude-3",
            "--base-branch",
            "develop",
            "init",
        ])
        .unwrap();
        assert!(matches!(cli.command, Commands::Init));
        assert_eq!(cli.model, Some("claude-3".into()));
        assert_eq!(cli.base_branch, Some("develop".into()));
    }

    #[test]
    fn cli_parses_config_command() {
        let cli = Cli::try_parse_from(["gitar", "config"]).unwrap();
        assert!(matches!(cli.command, Commands::Config));
    }

    #[test]
    fn cli_parses_models_command() {
        let cli = Cli::try_parse_from(["gitar", "models"]).unwrap();
        assert!(matches!(cli.command, Commands::Models));
    }

    #[test]
    fn cli_with_provider_claude() {
        let cli = Cli::try_parse_from(["gitar", "--provider", "claude", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged { .. }));
        assert_eq!(cli.provider, Some("claude".into()));
    }

    #[test]
    fn cli_with_provider_gemini() {
        let cli = Cli::try_parse_from(["gitar", "--provider", "gemini", "staged"]).unwrap();
        assert_eq!(cli.provider, Some("gemini".into()));
    }

    #[test]
    fn cli_with_provider_ollama() {
        let cli = Cli::try_parse_from(["gitar", "--provider", "ollama", "staged"]).unwrap();
        assert_eq!(cli.provider, Some("ollama".into()));
    }

    #[test]
    fn cli_rejects_invalid_provider() {
        let result = Cli::try_parse_from(["gitar", "--provider", "invalid", "staged"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_parses_hook_install() {
        let cli = Cli::try_parse_from(["gitar", "hook", "install"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Hook {
                command: HookCommands::Install
            }
        ));
    }

    #[test]
    fn cli_parses_hook_uninstall() {
        let cli = Cli::try_parse_from(["gitar", "hook", "uninstall"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Hook {
                command: HookCommands::Uninstall
            }
        ));
    }

    #[test]
    fn hook_script_unix_contains_marker() {
        assert!(HOOK_SCRIPT.contains("gitar-hook"));
    }

    #[test]
    fn hook_scripts_skip_when_message_provided() {
        assert!(HOOK_SCRIPT.contains("COMMIT_SOURCE"));
    }

    #[test]
    fn hook_scripts_check_gitar_installed() {
        assert!(HOOK_SCRIPT.contains("command -v gitar"));
    }

    #[test]
    fn cli_parses_all_alg_values() {
        for alg_val in 1..=4 {
            let cli =
                Cli::try_parse_from(["gitar", "commit", "--alg", &alg_val.to_string()]).unwrap();
            if let Commands::Commit { alg, .. } = cli.command {
                assert_eq!(alg, alg_val);
            }
        }
    }
}