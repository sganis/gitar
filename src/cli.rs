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
    gitar commit --algo 3            # Use hunk-level analysis for large refactors

DIFF ALGORITHMS:
    --algo 1    Full: complete git diff (ignores --max-chars)
    --algo 2    Files: selective files, ranked by priority
    --algo 3    Hunks: selective hunks, ranked by importance
    --algo 4    Semantic: JSON IR with scored hunks (default)

STYLE PRESETS:
    --preset rust       Rust conventions (crate/module focused)
    --preset js         JavaScript conventions (component/hook focused)
    --preset python     Python conventions (module/endpoint focused)
    --preset auto       Auto-detect from project files (default)"
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

    /// Commit message style preset (rust, js, python, auto)
    #[arg(
        long,
        global = true,
        value_parser = ["rust", "rs", "javascript", "js", "python", "py", "auto", "default"]
    )]
    pub preset: Option<String>,

    /// Stream responses to stdout (when supported by the provider).
    #[arg(long, global = true, default_value_t = false)]
    pub stream: bool,

    /// Disable TLS certificate verification (INSECURE - use only for debugging)
    #[arg(long, global = true, default_value_t = false)]
    pub insecure_tls: bool,

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

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },

    /// Generate an AI commit message for currently staged changes
    ///
    /// Prints the message to stdout (does not create a commit).
    Staged {
        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },

    /// Generate an AI commit message for unstaged working tree changes
    ///
    /// Prints the message to stdout (does not create a commit).
    Unstaged {
        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
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

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
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

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
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

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
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

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
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

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
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
        algo: Option<u8>,

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

    /// [DEPRECATED] Split working tree into commits (use `gitar plan` instead)
    ///
    /// This command is deprecated and will be removed in v2.0.0.
    /// Use `gitar plan --mode working --apply` for the same functionality
    /// with improved features (interactive editing, better grouping, etc).
    #[deprecated(since = "1.1.0", note = "use `gitar plan --mode working` instead")]
    Split {
        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },

    /// Resolve merge/rebase/cherry-pick conflicts (semantic synthesis).
    ///
    /// Default: inspect and propose. Use --apply to write + stage.
    Resolve {
        /// Apply suggested resolutions (writes files + stages them)
        #[arg(long, default_value_t = false)]
        apply: bool,

        /// Assume "yes" for prompts (required with --apply for now)
        #[arg(long, default_value_t = false)]
        yes: bool,

        /// Stream output (per-command override). Global --stream also enables streaming.
        #[arg(long, default_value_t = false)]
        stream: bool,
    },

    /// Analyze repo state and create a multi-commit plan
    ///
    /// The plan command analyzes your changes and proposes an optimal commit structure.
    /// You can review and refine the plan interactively before execution.
    Plan {
        /// Execute the plan after approval
        #[arg(long, default_value_t = false)]
        apply: bool,

        /// Legacy mode: print next-command suggestions based on repo state
        #[arg(long, default_value_t = false)]
        suggest: bool,

        /// Analysis mode: working, staged, or history
        #[arg(long, value_parser = ["working", "staged", "history", "auto"])]
        mode: Option<String>,

        /// For history mode: starting ref (tag, commit, branch)
        #[arg(long, requires = "mode")]
        from: Option<String>,

        /// For history mode: ending ref (default: HEAD)
        #[arg(long, requires = "from")]
        to: Option<String>,

        /// Enable interactive mode (move files, edit messages, exclude files)
        #[arg(long, short = 'i', default_value_t = true)]
        interactive: bool,

        /// Non-interactive mode (auto-approve plan)
        #[arg(long, conflicts_with = "interactive")]
        yes: bool,

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
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

    // ==========================================================================
    // Core command parsing tests
    // ==========================================================================

    #[test]
    fn cli_parses_all_commands() {
        // Test that all command variants parse correctly
        let commands = [
            vec!["gitar", "commit"],
            vec!["gitar", "staged"],
            vec!["gitar", "unstaged"],
            vec!["gitar", "pr", "main"],
            vec!["gitar", "changelog"],
            vec!["gitar", "explain"],
            vec!["gitar", "version"],
            vec!["gitar", "history"],
            vec!["gitar", "init"],
            vec!["gitar", "config"],
            vec!["gitar", "models"],
            vec!["gitar", "diff"],
            vec!["gitar", "split"],
            vec!["gitar", "resolve"],
            vec!["gitar", "plan"],
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

    // ==========================================================================
    // Resolve + Plan command tests
    // ==========================================================================

    #[test]
    fn cli_parses_resolve_flags() {
        let cli =
            Cli::try_parse_from(["gitar", "resolve", "--apply", "--yes", "--stream"]).unwrap();
        if let Commands::Resolve { apply, yes, stream } = cli.command {
            assert!(apply);
            assert!(yes);
            assert!(stream);
        } else {
            panic!("Expected Resolve command");
        }
    }

    #[test]
    fn cli_parses_plan_flags() {
        let cli = Cli::try_parse_from(["gitar", "plan", "--apply", "--suggest"]).unwrap();
        if let Commands::Plan { apply, suggest, .. } = cli.command {
            assert!(apply);
            assert!(suggest);
        } else {
            panic!("Expected Plan command");
        }
    }

    #[test]
    fn cli_parses_plan_mode() {
        let cli = Cli::try_parse_from(["gitar", "plan", "--mode", "working"]).unwrap();
        if let Commands::Plan { mode, .. } = cli.command {
            assert_eq!(mode, Some("working".to_string()));
        } else {
            panic!("Expected Plan command");
        }
    }

    #[test]
    fn cli_parses_plan_algo() {
        let cli = Cli::try_parse_from(["gitar", "plan", "--algo", "3"]).unwrap();
        if let Commands::Plan { algo, .. } = cli.command {
            assert_eq!(algo, 3);
        } else {
            panic!("Expected Plan command");
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
        assert!(HOOK_SCRIPT.contains("--write-to"), "Should use write-to flag");
    }
}
