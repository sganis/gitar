// src/cli.rs
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand};

/// Build Cargo-style CLI colors for clap.
/// Respects NO_COLOR env and works cross-platform.
pub fn cargo_styles() -> Styles {
    Styles::styled()
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Blue.on_default() | Effects::BOLD)
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Green.on_default())
        .invalid(AnsiColor::Yellow.on_default())
}

/// Build styled after_help text with Cargo-like colors.
/// - Headers: Green + Bold
/// - Commands/flags: Cyan + Bold
/// - Placeholders: Blue + Bold
/// - Comments: Gray (BrightBlack)
/// - Descriptions: Default
pub fn styled_after_help() -> String {
    use anstyle::{AnsiColor, Effects, Style};

    let header = Style::new()
        .fg_color(Some(AnsiColor::Green.into()))
        .effects(Effects::BOLD);
    let cmd = Style::new()
        .fg_color(Some(AnsiColor::Cyan.into()))
        .effects(Effects::BOLD);
    let placeholder = Style::new()
        .fg_color(Some(AnsiColor::Blue.into()))
        .effects(Effects::BOLD);
    //let comment = Style::new().fg_color(Some(AnsiColor::BrightBlack.into()));
    let desc = Style::new(); // default

    let h = |s: &str| format!("{header}{s}{header:#}");
    let c = |s: &str| format!("{cmd}{s}{cmd:#}");
    let p = |s: &str| format!("{placeholder}{s}{placeholder:#}");
    let g = |s: &str| format!("{s}");
    let d = |s: &str| format!("{desc}{s}{desc:#}");

    format!(
        r#"{header}
    {cmd1}                          {cmt1}
    {cmd2}                     {cmt2}
    {cmd3}             {cmt3}
    {cmd4}    {cmt4}

    {cmd5}                     {cmt5}
    {cmd6}            {cmt6}
    {cmd7}                {cmt7}
    {cmd8}           {cmt8}
    {cmd9}    {cmt9}
    {cmd10}      {cmt10}

    {cmd11}                      {cmt11}
    {cmd12}              {cmt12}

    {cmd13}                  {cmt13}
    {cmd14}          {cmt14}

    {cmd15}           {cmt15}
    {cmd16}         {cmt16}

{scope_header}
    {flag1}                      {cmt17}
    {flag2}                       {cmt18}
    {flag3} {ph1}                {cmt19}

{algo_header}
    {algo1}                       {desc1}
    {algo2}                       {desc2}
    {algo3}                       {desc3}
    {algo4}                       {desc4}

{preset_header}
    {preset1}                  {desc5}
    {preset2}                    {desc6}
    {preset3}                {desc7}
    {preset4}                  {desc8}"#,
        header = h("EXAMPLES:"),
        cmd1 = c("gitar"),
        cmt1 = g("# Plan commits (default command)"),
        cmd2 = c("gitar plan"),
        cmt2 = g("# Same as above"),
        cmd3 = c("gitar plan --apply"),
        cmt3 = g("# Execute the plan"),
        cmd4 = c("gitar plan --history v1.0.0"),
        cmt4 = g("# Plan from history"),
        cmd5 = c("gitar tell"),
        cmt5 = g("# Plain English business value explanation (default)"),
        cmd6 = c("gitar tell --commit"),
        cmt6 = g("# Generate commit message"),
        cmd7 = c("gitar tell --pr"),
        cmt7 = g("# Generate PR description"),
        cmd8 = c("gitar tell --pr main"),
        cmt8 = g("# PR description against main"),
        cmd9 = c("gitar tell --changelog v1.0"),
        cmt9 = g("# Release notes since tag"),
        cmd10 = c("gitar tell --history v1.0"),
        cmt10 = g("# Describe commits since tag"),
        cmd11 = c("gitar fix"),
        cmt11 = g("# Preview conflict resolution"),
        cmd12 = c("gitar fix --apply"),
        cmt12 = g("# Apply conflict fixes"),
        cmd13 = c("gitar release"),
        cmt13 = g("# Preview release (version, changelog, tag)"),
        cmd14 = c("gitar release --apply"),
        cmt14 = g("# Execute release"),
        cmd15 = c("gitar hook --install"),
        cmt15 = g("# Install git hook for auto-commit messages"),
        cmd16 = c("gitar hook --uninstall"),
        cmt16 = g("# Remove gitar git hook"),
        scope_header = h("SCOPE FLAGS (available on plan command):"),
        flag1 = c("--working"),
        cmt17 = g("# Analyze working tree changes"),
        flag2 = c("--staged"),
        cmt18 = g("# Analyze staged changes only"),
        flag3 = c("--history"),
        ph1 = p("<REF>"),
        cmt19 = g("# Analyze history from REF to HEAD"),
        algo_header = h("DIFF ALGORITHMS:"),
        algo1 = c("--algo 1"),
        desc1 = d("# Full: complete git diff (ignores --max-chars)"),
        algo2 = c("--algo 2"),
        desc2 = d("# Files: selective files, ranked by priority"),
        algo3 = c("--algo 3"),
        desc3 = d("# Hunks: selective hunks, ranked by importance"),
        algo4 = c("--algo 4"),
        desc4 = d("# Semantic: JSON IR with scored hunks (default)"),
        preset_header = h("STYLE PRESETS:"),
        preset1 = c("--preset rust"),
        desc5 = d("# Rust conventions (crate/module focused)"),
        preset2 = c("--preset js"),
        desc6 = d("# JavaScript conventions (component/hook focused)"),
        preset3 = c("--preset python"),
        desc7 = d("# Python conventions (module/endpoint focused)"),
        preset4 = c("--preset auto"),
        desc8 = d("# Auto-detect from project files (default)"),
    )
}

#[derive(Parser)]
#[command(
    name = "gitar",
    version,
    styles = cargo_styles(),
    about = "AI-powered Git assistant\n\nGitar is an AI-native interface to plan, explain, fix, and release your Git history.",
    after_help = styled_after_help()
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
    pub insecure: bool,

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
    #[command(visible_alias = "p")]
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

    /// Tell, describe, and communicate Git changes (read-only)
    ///
    /// Use selector flags to choose output type:
    ///   --commit     Generate AI commit message
    ///   --pr         Generate PR description
    ///   --changelog  Generate release notes
    ///   --history    Describe commit range
    ///   --explain    Plain English business value explanation  (default)
    #[command(visible_alias = "t")]
    Tell {
        /// Generate a commit message (and optionally commit)
        #[arg(long, group = "selector")]
        commit: bool,

        /// Generate a PR description
        #[arg(long, group = "selector")]
        pr: bool,

        /// Generate release notes (changelog)
        #[arg(long, group = "selector")]
        changelog: bool,

        /// Describe a range of commits
        #[arg(long, group = "selector")]
        history: bool,

        /// Plain English business value explanation (default)
        #[arg(long, group = "selector")]
        explain: bool,

        /// Reference (tag, commit, branch) - meaning depends on selector
        #[arg(value_name = "REF")]
        reference: Option<String>,

        /// Ending ref (default: HEAD)
        #[arg(long)]
        to: Option<String>,

        /// Use staged changes only
        #[arg(long)]
        staged: bool,

        /// Only include commits after this date
        #[arg(long)]
        since: Option<String>,

        /// Only include commits before this date
        #[arg(long)]
        until: Option<String>,

        /// Maximum number of commits to include
        #[arg(short = 'n', long)]
        limit: Option<usize>,

        /// Delay between API calls in ms (for --history)
        #[arg(long, default_value = "500")]
        delay: u64,

        /// Diff algorithm: 1=full, 2=files, 3=hunks, 4=semantic (default)
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,

        /// Stream output
        #[arg(long, default_value_t = false)]
        stream: bool,

        // Commit-specific flags
        /// Push after committing (--commit only)
        #[arg(short = 'p', long)]
        push: bool,

        /// Stage all changes before committing (--commit only)
        #[arg(short = 'a', long)]
        all: bool,

        /// Amend the last commit (--commit only)
        #[arg(long)]
        amend: bool,

        /// Add AI model/provider tag to the commit message (--commit only)
        #[arg(long, default_value = "true")]
        ai_author: bool,

        /// Do not add AI model/provider tag (--commit only)
        #[arg(long = "no-ai-author")]
        no_ai_author: bool,

        /// Write commit message to file instead of committing (internal)
        #[arg(long, hide = true)]
        write_to: Option<String>,

        /// Suppress interactive prompts (internal)
        #[arg(long, hide = true)]
        silent: bool,
    },

    /// Fix merge/rebase/cherry-pick conflicts (semantic synthesis)
    ///
    /// Default: inspect and propose. Use --apply to write + stage.
    #[command(visible_alias = "f")]
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
    #[command(visible_alias = "r")]
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
    ///
    /// Use --install to add the prepare-commit-msg hook.
    /// Use --uninstall to remove it.
    Hook {
        /// Install the prepare-commit-msg hook
        #[arg(long)]
        install: bool,

        /// Uninstall the prepare-commit-msg hook
        #[arg(long)]
        uninstall: bool,
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

    // === Compatibility Aliases ===
    // These are thin wrappers for migration / muscle memory

    /// [Alias] Same as `gitar plan`
    #[command(hide = true)]
    Run {
        #[arg(long, default_value_t = false)]
        apply: bool,
        #[arg(long, default_value_t = false)]
        fix: bool,
        #[arg(long, default_value_t = false)]
        suggest: bool,
        #[arg(long)]
        working: bool,
        #[arg(long)]
        staged: bool,
        #[arg(long, value_name = "REF")]
        history: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, short = 'i', default_value_t = true)]
        interactive: bool,
        #[arg(long, conflicts_with = "interactive")]
        yes: bool,
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },

    /// [Alias] Same as `gitar fix`
    #[command(hide = true)]
    Resolve {
        #[arg(long, default_value_t = false)]
        apply: bool,
        #[arg(long, default_value_t = false)]
        yes: bool,
        #[arg(long, default_value_t = false)]
        stream: bool,
    },

    /// [Alias] Same as `gitar tell --commit`
    #[command(hide = true)]
    Commit {
        #[arg(short = 'p', long)]
        push: bool,
        #[arg(short = 'a', long)]
        all: bool,
        #[arg(long)]
        amend: bool,
        #[arg(long, default_value = "true")]
        ai_author: bool,
        #[arg(long = "no-ai-author")]
        no_ai_author: bool,
        #[arg(long, hide = true)]
        write_to: Option<String>,
        #[arg(long, hide = true)]
        silent: bool,
        #[arg(long, default_value = "false")]
        stream: bool,
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },

    /// [Alias] Same as `gitar tell --pr`
    #[command(hide = true)]
    Pr {
        #[arg(value_name = "REF")]
        base: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        staged: bool,
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },

    /// [Alias] Same as `gitar tell --changelog`
    #[command(hide = true)]
    Changelog {
        #[arg(value_name = "REF")]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        until: Option<String>,
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },

    /// [Alias] Same as `gitar tell --history`
    #[command(hide = true)]
    History {
        #[arg(value_name = "REF")]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        until: Option<String>,
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        #[arg(long, default_value = "500")]
        delay: u64,
        #[arg(long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..=4))]
        algo: u8,
    },
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
gitar tell --commit --write-to "$COMMIT_MSG_FILE" --silent
"#;
