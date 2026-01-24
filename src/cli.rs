// src/cli.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gitar",
    version,
    about = "AI-powered Git assistant\n\nGitar is an AI-native interface to plan, explain, fix, and release your Git history.",
    after_help = "EXAMPLES:
    gitar                           # Plan commits (default command)
    gitar plan                      # Same as above
    gitar plan --apply              # Execute the plan
    gitar plan --history v1.0.0     # Plan from history

    gitar explain commit            # Create commit with AI message
    gitar explain pr                # Generate PR description
    gitar explain changelog v1.0.0  # Release notes since tag
    gitar explain history v1.0.0    # Describe commits since tag
    gitar explain report            # Plain English explanation

    gitar fix                       # Preview conflict resolution
    gitar fix --apply               # Apply conflict fixes

    gitar release                   # Preview release (version, changelog, tag)
    gitar release --apply           # Execute release

    gitar hook install              # Install git hook for auto-commit messages
    gitar hook uninstall            # Remove gitar git hook

SCOPE FLAGS (available on plan command):
    --working                       # Analyze working tree changes
    --staged                        # Analyze staged changes only
    --history <REF>                 # Analyze history from REF to HEAD

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
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze repo state and create a multi-commit execution plan
    ///
    /// The plan command analyzes your changes and proposes an optimal commit structure.
    /// You can review and refine the plan interactively before execution.
    /// This is the default command when running `gitar` without arguments.
    Plan {
        /// Execute the plan after approval
        #[arg(long, default_value_t = false)]
        apply: bool,

        /// Auto-fix conflicts before analysis
        #[arg(long, default_value_t = false)]
        fix: bool,

        /// Legacy mode: print next-command suggestions based on repo state
        #[arg(long, default_value_t = false)]
        suggest: bool,

        /// Analyze working tree changes (unstaged + untracked)
        #[arg(long)]
        working: bool,

        /// Analyze staged changes only
        #[arg(long)]
        staged: bool,

        /// Analyze history from REF to HEAD
        #[arg(long, value_name = "REF")]
        history: Option<String>,

        /// For history mode: ending ref (default: HEAD)
        #[arg(long)]
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

    /// Explain, describe, and communicate Git changes (read-only)
    ///
    /// All read-only narration commands: commit messages, PR descriptions,
    /// changelogs, history descriptions, and plain English reports.
    Explain {
        #[command(subcommand)]
        command: ExplainCommands,
    },

    /// Fix merge/rebase/cherry-pick conflicts (semantic synthesis)
    ///
    /// Default: inspect and propose. Use --apply to write + stage.
    Fix {
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

    /// Create a new release (version bump, changelog, tag)
    ///
    /// Analyzes commits since the last tag, suggests a version bump,
    /// updates version files, generates a changelog, and creates a git tag.
    Release {
        /// Execute the release (default: dry-run)
        #[arg(long, default_value_t = false)]
        apply: bool,

        /// Skip changelog generation
        #[arg(long, default_value_t = false)]
        skip_changelog: bool,

        /// Skip writing changelog to CHANGELOG.md file
        #[arg(long, default_value_t = false)]
        skip_changelog_file: bool,

        /// Custom changelog file path (default: CHANGELOG.md)
        #[arg(long, default_value = "CHANGELOG.md")]
        changelog_file: String,

        /// Base reference (tag/commit/branch) - defaults to latest tag
        #[arg(long)]
        from: Option<String>,

        /// Version bump strategy: auto (LLM analysis), major, minor, or patch
        #[arg(long, default_value = "auto")]
        bump: String,

        /// Ending ref (default: HEAD) - used for LLM version analysis
        #[arg(long)]
        to: Option<String>,

        /// Diff algorithm for LLM analysis: 1=full, 2=files, 3=hunks, 4=semantic (default)
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

    /// Squash recent commits into one with an AI-generated message
    ///
    /// Combines multiple commits into a single commit with a unified message.
    Squash {
        /// Number of commits to squash (e.g., 3) or a ref (e.g., HEAD~5, v1.0.0)
        #[arg(value_name = "COUNT_OR_REF")]
        target: String,

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },

    /// Rewrite commit history with AI-generated messages
    ///
    /// Interactively regenerate commit messages for a range of commits.
    Rewrite {
        /// Number of commits to rewrite (e.g., 5) or a ref (e.g., HEAD~5, v1.0.0)
        #[arg(value_name = "COUNT_OR_REF")]
        target: String,

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },
}

#[derive(Subcommand, Clone)]
pub enum ExplainCommands {
    /// Create a commit with an AI-generated message
    ///
    /// By default this will generate a message from staged changes, then run `git commit`.
    /// Use `-a` to stage all changes first, and `-p` to push after committing.
    /// Use `--amend` to regenerate the message for the last commit.
    Commit {
        /// Push after committing
        #[arg(short = 'p', long)]
        push: bool,

        /// Stage all changes before committing (`git add -A`)
        #[arg(short = 'a', long)]
        all: bool,

        /// Amend the last commit with a new AI-generated message
        #[arg(long)]
        amend: bool,

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

    /// Generate an AI commit message for currently staged changes (no commit)
    Staged {
        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },

    /// Generate an AI commit message for unstaged working tree changes (no commit)
    Unstaged {
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

    /// Explain changes in plain English for non-technical stakeholders
    ///
    /// Can explain a commit range or staged changes (`--staged`).
    Report {
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
gitar explain commit --write-to "$COMMIT_MSG_FILE" --silent
"#;
