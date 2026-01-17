// src/main.rs
mod claude;
mod client;
mod config;
mod gemini;
mod git;
mod openai;
mod prompts;
mod types;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use client::LlmClient;
use config::{normalize_provider, Config, ResolvedConfig};
use git::*;
use prompts::*;

// =============================================================================
// CLI
// =============================================================================
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
    
    gitar version v1.0.0            # Version bump since tag"
)]
struct Cli {
    #[arg(long, global = true)]
    api_key: Option<String>,
    #[arg(long, global = true)]
    model: Option<String>,
    #[arg(long, global = true)]
    max_tokens: Option<u32>,
    #[arg(long, global = true)]
    temperature: Option<f32>,
    #[arg(long, env = "OPENAI_BASE_URL", global = true)]
    base_url: Option<String>,
    #[arg(long, global = true)]
    base_branch: Option<String>,
    #[arg(long, global = true, value_parser = ["openai", "claude", "anthropic", "gemini", "google", "groq", "ollama", "local"])]
    provider: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
    },

    /// Generate an AI commit message for currently staged changes
    ///
    /// Prints the message to stdout (does not create a commit).
    Staged,

    /// Generate an AI commit message for unstaged working tree changes
    ///
    /// Prints the message to stdout (does not create a commit).
    Unstaged,

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
}


#[derive(Subcommand, Clone)]
enum HookCommands {
    /// Install the prepare-commit-msg hook
    Install,
    /// Uninstall the prepare-commit-msg hook
    Uninstall,
}

// =============================================================================
// HOOK SCRIPTS
// =============================================================================
const HOOK_SCRIPT_UNIX: &str = r#"#!/bin/sh
# gitar-hook: Auto-generated by gitar
# Generates AI commit messages automatically

# Skip if gitar is not installed
if ! command -v gitar >/dev/null 2>&1; then
    exit 0
fi

COMMIT_MSG_FILE=$1
COMMIT_SOURCE=$2

# Skip if message was provided via -m, -F, or is a merge/squash
if [ -n "$COMMIT_SOURCE" ]; then
    exit 0
fi

# Generate commit message and write to file
gitar commit --write-to "$COMMIT_MSG_FILE" --silent --no-tag < /dev/tty
"#;

const HOOK_SCRIPT_WINDOWS: &str = r#"@echo off
REM gitar-hook: Auto-generated by gitar
REM Generates AI commit messages automatically

REM Check if gitar is installed
where gitar >nul 2>nul
if %errorlevel% neq 0 exit /b 0

set COMMIT_MSG_FILE=%1
set COMMIT_SOURCE=%2

REM Skip if message was provided via -m or -F
if not "%COMMIT_SOURCE%"=="" exit /b 0

REM Generate commit message and write to file
gitar commit --write-to "%COMMIT_MSG_FILE%" --silent --no-tag
"#;

// =============================================================================
// COMMANDS
// =============================================================================
async fn cmd_commit(
    client: &LlmClient,
    push: bool,
    all: bool,
    tag: bool,
    write_to: Option<String>,
    silent: bool,
) -> Result<()> {
    let staged = run_git(&["diff", "--cached"]).unwrap_or_default();
    let unstaged = run_git(&["diff"]).unwrap_or_default();

    let mut diff = String::new();
    if !staged.trim().is_empty() {
        diff.push_str(&staged);
    }
    if !unstaged.trim().is_empty() {
        if !diff.is_empty() { diff.push('\n'); }
        diff.push_str(&unstaged);
    }

    if diff.trim().is_empty() {
        if !silent {
            println!("Nothing to commit.");
        }
        return Ok(());
    }

    let diff = truncate_diff(diff, 100000);

    // Hook mode: generate message and write to file
    if let Some(ref output_file) = write_to {
        let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
        let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt).await?;
        fs::write(output_file, format!("{}\n", msg.trim()))?;
        return Ok(());
    }

    // Interactive mode
    let commit_message = loop {
        let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
        let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt).await?;

        if silent {
            break msg;
        }

        println!("\n{}\n", msg);
        println!("{}", "=".repeat(50));
        println!("  [Enter] Accept | [g] Regenerate | [e] Edit | [other] Cancel");
        println!("{}", "=".repeat(50));
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "" => break msg,
            "g" => { println!("Regenerating...\n"); continue; }
            "e" => {
                print!("New message: ");
                io::stdout().flush()?;
                let mut ed = String::new();
                io::stdin().read_line(&mut ed)?;
                break if ed.trim().is_empty() { msg } else { ed.trim().into() };
            }
            _ => { println!("Canceled."); return Ok(()); }
        }
    };

    if all {
        if !silent { println!("Staging all..."); }
        run_git(&["add", "-A"])?;
    }

    if !silent { println!("Committing..."); }
    let full_msg = if tag {
        format!("{} [AI:{}]", commit_message, client.model())
    } else {
        commit_message
    };

    let (out, err, ok) = if all {
        run_git_status(&["commit", "-am", &full_msg])
    } else {
        run_git_status(&["commit", "-m", &full_msg])
    };
    if !silent { println!("{}{}", out, err); }

    if !ok {
        if !silent { println!("Commit failed."); }
        return Ok(());
    }

    if push {
        if !silent { println!("Pushing..."); }
        let (o, e, _) = run_git_status(&["push"]);
        if !silent { println!("{}{}", o, e); }
    }

    Ok(())
}

async fn cmd_staged(client: &LlmClient) -> Result<()> {
    let diff = get_diff(None, true, 100000)?;
    if diff.trim().is_empty() { bail!("No staged changes."); }
    let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
    let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", msg);
    Ok(())
}

async fn cmd_unstaged(client: &LlmClient) -> Result<()> {
    let diff = get_diff(None, false, 100000)?;
    if diff.trim().is_empty() { bail!("No unstaged changes."); }
    let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
    let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", msg);
    Ok(())
}

async fn cmd_history(
    client: &LlmClient,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    limit: Option<usize>,
    delay: u64,
) -> Result<()> {
    let limit = match (&from, limit) {
        (Some(_), None) => None,
        (None, None) => Some(50),
        (_, Some(n)) => Some(n),
    };

    let end = to.as_deref().unwrap_or("HEAD");
    let range = from.as_ref().map(|r| format!("{}..{}", r, end));

    let display = match (&from, &to, &since, &until) {
        (Some(r), Some(t), _, _) => format!("{}..{}", r, t),
        (Some(r), None, _, _) => format!("{}..HEAD", r),
        (None, None, Some(s), _) => format!("--since {}", s),
        _ => "recent".into(),
    };

    println!("Fetching commits ({})...", display);
    let commits = get_commit_logs(limit, since.as_deref(), until.as_deref(), range.as_deref())?;

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    println!("Processing {} commits...\n", commits.len());

    for (i, c) in commits.iter().enumerate() {
        let h = &c.hash[..8.min(c.hash.len())];
        let d = &c.date[..10.min(c.date.len())];
        let a = if c.author.len() > 15 { &c.author[..15] } else { &c.author };
        let m = if c.message.len() > 40 { &c.message[..40] } else { &c.message };

        println!("[{}/{}] {} | {} | {:15} | {}", i + 1, commits.len(), h, d, a, m);

        let diff = match get_commit_diff(&c.hash, 12000)? {
            Some(d) if !d.trim().is_empty() => d,
            _ => { println!("  - No diff"); continue; }
        };

        let prompt = HISTORY_USER_PROMPT
            .replace("{original_message}", &c.message)
            .replace("{diff}", &diff);

        match client.chat(HISTORY_SYSTEM_PROMPT, &prompt).await {
            Ok(r) => {
                for (j, l) in r.lines().enumerate() {
                    if !l.trim().is_empty() {
                        println!("{}{}", if j == 0 { "  - " } else { "    " }, l);
                    }
                }
            }
            Err(e) => println!("  x {}", e),
        }

        if i < commits.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
    }

    Ok(())
}

async fn cmd_pr(
    client: &LlmClient,
    base: Option<String>,
    to: Option<String>,
    base_branch: &str,
    staged: bool,
) -> Result<()> {
    let branch = to.clone().unwrap_or_else(get_current_branch);
    let target_base = base.as_deref().unwrap_or(base_branch);

    println!("PR: {} -> {}\n", branch, target_base);

    let (diff, stats, commits_text) = if staged {
        (get_diff(None, true, 15000)?, get_diff_stats(None, true)?, "(staged changes)".into())
    } else {
        let diff_target = build_diff_target(base.as_deref(), to.as_deref(), base_branch);
        let range = build_range(base.as_deref(), to.as_deref(), base_branch);

        let commits = get_commit_logs(Some(20), None, None, range.as_deref())?;
        let ct = commits.iter().map(|c| format!("- {}", c.message)).collect::<Vec<_>>().join("\n");

        let diff_target_ref = if diff_target.is_empty() { None } else { Some(diff_target.as_str()) };

        (
            get_diff(diff_target_ref, false, 15000)?,
            get_diff_stats(diff_target_ref, false)?,
            if ct.is_empty() { "(no commits)".into() } else { ct },
        )
    };

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = PR_USER_PROMPT
        .replace("{branch}", &branch)
        .replace("{commits}", &commits_text)
        .replace("{stats}", &stats)
        .replace("{diff}", &diff);

    let r = client.chat(PR_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", r);
    Ok(())
}

async fn cmd_changelog(
    client: &LlmClient,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let limit = match (&from, limit) {
        (Some(_), None) => None,
        (None, None) => Some(50),
        (_, Some(n)) => Some(n),
    };

    let end = to.as_deref().unwrap_or("HEAD");
    let range = from.as_ref().map(|r| format!("{}..{}", r, end));

    let display = match (&from, &to, &since, &until) {
        (Some(r), Some(t), _, _) => format!("{}..{}", r, t),
        (Some(r), None, _, _) => format!("{}..HEAD", r),
        (None, Some(t), _, _) => format!("..{}", t),
        (None, None, Some(s), Some(u)) => format!("--since {} --until {}", s, u),
        (None, None, Some(s), None) => format!("--since {}", s),
        (None, None, None, Some(u)) => format!("--until {}", u),
        (None, None, None, None) => "recent (last 50 commits)".into(),
    };

    println!("Changelog for {}...\n", display);
    let commits = get_commit_logs(limit, since.as_deref(), until.as_deref(), range.as_deref())?;

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    println!("Found {} commits.\n", commits.len());

    let ct = commits
        .iter()
        .map(|c| format!("- [{}] {}", &c.hash[..8.min(c.hash.len())], c.message))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = CHANGELOG_USER_PROMPT
        .replace("{range}", &display)
        .replace("{count}", &commits.len().to_string())
        .replace("{commits}", &ct);

    let r = client.chat(CHANGELOG_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", r);
    Ok(())
}

async fn cmd_explain(
    client: &LlmClient,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    base_branch: &str,
    staged: bool,
) -> Result<()> {
    let display = match (&from, &to, &since, &until) {
        (Some(r), Some(t), _, _) => format!("{}..{}", r, t),
        (Some(r), None, _, _) => format!("{}..HEAD", r),
        (None, Some(t), _, _) => format!("..{}", t),
        (None, None, Some(s), Some(u)) => format!("--since {} --until {}", s, u),
        (None, None, Some(s), None) => format!("--since {}", s),
        (None, None, None, Some(u)) => format!("--until {}", u),
        (None, None, None, None) => "working tree vs HEAD".into(),
    };

    let mut commit_count: Option<usize> = None;

    let (diff, stats) = if staged {
        println!("Explaining staged changes...\n");
        (get_diff(None, true, 15000)?, get_diff_stats(None, true)?)
    } else {
        let effective_from = match (&from, &since, &until) {
            (Some(_), _, _) => from.clone(),
            (None, Some(_), _) | (None, None, Some(_)) => {
                let commits = get_commit_logs(None, since.as_deref(), until.as_deref(), None)?;
                commit_count = Some(commits.len());
                commits.last().map(|c| c.hash.clone())
            }
            _ => None,
        };

        match commit_count {
            Some(n) => println!("Explaining changes for {} ({} commits)...\n", display, n),
            None => println!("Explaining changes for {}...\n", display),
        }

        let diff_target = build_diff_target(effective_from.as_deref(), to.as_deref(), base_branch);
        let diff_target_ref = if diff_target.is_empty() { None } else { Some(diff_target.as_str()) };

        (get_diff(diff_target_ref, false, 15000)?, get_diff_stats(diff_target_ref, false)?)
    };

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = EXPLAIN_USER_PROMPT
        .replace("{range}", if staged { "staged" } else { &display })
        .replace("{stats}", &stats)
        .replace("{diff}", &diff);

    let r = client.chat(EXPLAIN_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", r);
    Ok(())
}

async fn cmd_version(
    client: &LlmClient,
    base: Option<String>,
    to: Option<String>,
    base_branch: &str,
    current: Option<String>,
) -> Result<()> {
    let current = current.unwrap_or_else(get_current_version);
    println!("Version analysis (current: {})...\n", current);

    let diff_target = build_diff_target(base.as_deref(), to.as_deref(), base_branch);
    let diff_target_ref = if diff_target.is_empty() { None } else { Some(diff_target.as_str()) };

    let diff = get_diff(diff_target_ref, false, 15000)?;

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = VERSION_USER_PROMPT
        .replace("{version}", &current)
        .replace("{diff}", &diff);

    let r = client.chat(VERSION_SYSTEM_PROMPT, &prompt).await?;
    println!("{}", r);
    Ok(())
}

fn cmd_hook(command: HookCommands) -> Result<()> {
    let git_dir = get_git_dir().context("Not in a git repository")?;
    let hooks_dir = git_dir.join("hooks");

    // Create hooks directory if it doesn't exist
    if !hooks_dir.exists() {
        fs::create_dir_all(&hooks_dir)?;
    }

    // Determine filename and script content based on OS
    let (hook_filename, script_content) = if cfg!(windows) {
        ("prepare-commit-msg.bat", HOOK_SCRIPT_WINDOWS)
    } else {
        ("prepare-commit-msg", HOOK_SCRIPT_UNIX)
    };

    let hook_path = hooks_dir.join(hook_filename);

    match command {
        HookCommands::Install => {
            // Check for existing hook
            if hook_path.exists() {
                let existing = fs::read_to_string(&hook_path)?;
                if existing.contains("gitar-hook") {
                    println!("Hook already installed at {}", hook_path.display());
                    return Ok(());
                }
                println!("Warning: A {} hook already exists at {}", hook_filename, hook_path.display());
                println!("To use gitar, either:");
                println!("  1. Remove the existing hook and run 'gitar hook install' again");
                println!("  2. Manually add the following to your hook:\n");
                println!("{}", script_content);
                return Ok(());
            }

            // Write the hook script
            fs::write(&hook_path, script_content)?;

            // Set executable permission on Unix
            #[cfg(unix)]
            {
                let mut perms = fs::metadata(&hook_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&hook_path, perms)?;
            }

            println!("Hook installed at {}", hook_path.display());
            println!();
            println!("Now when you run 'git commit', gitar will automatically");
            println!("generate a commit message from your staged changes.");
            println!();
            if cfg!(windows) {
                println!("Try it: git add . & git commit");
            } else {
                println!("Try it: git add . && git commit");
            }
        }
        HookCommands::Uninstall => {
            if !hook_path.exists() {
                println!("No hook found at {}", hook_path.display());
                return Ok(());
            }

            let content = fs::read_to_string(&hook_path)?;
            if !content.contains("gitar-hook") {
                println!("The hook at {} was not created by gitar.", hook_path.display());
                println!("Please remove it manually if needed.");
                return Ok(());
            }

            fs::remove_file(&hook_path)?;
            println!("Hook uninstalled from {}", hook_path.display());
        }
    }

    Ok(())
}

fn cmd_init(cli: &Cli, file: &Config) -> Result<()> {
    let mut config = file.clone();

    // Determine which provider to configure
    let provider = cli.provider.as_ref()
        .map(|p| normalize_provider(p).to_string());

    if let Some(ref p) = provider {
        // Configure specific provider section
        let pc = config.get_provider_mut(p);
        if cli.api_key.is_some() { pc.api_key = cli.api_key.clone(); }
        if cli.model.is_some() { pc.model = cli.model.clone(); }
        if cli.max_tokens.is_some() { pc.max_tokens = cli.max_tokens; }
        if cli.temperature.is_some() { pc.temperature = cli.temperature; }
        if cli.base_url.is_some() { pc.base_url = cli.base_url.clone(); }
        
        // Always set as default provider
        config.default_provider = Some(p.clone());
    }

    // Global settings
    if cli.base_branch.is_some() { config.base_branch = cli.base_branch.clone(); }

    config.save()?;

    if let Some(p) = &provider {
        println!("Default provider set to: {}", p);
    }

    Ok(())
}

fn cmd_config() -> Result<()> {
    let config = Config::load();
    let path = Config::path().map(|p| p.display().to_string()).unwrap_or_else(|| "(unknown)".into());

    println!("Config file: {}\n", path);
    println!("default_provider: {}", config.default_provider.as_deref().unwrap_or("(not set)"));
    println!("base_branch:      {}", config.base_branch.as_deref().unwrap_or("(not set)"));

    let providers = [
        ("openai", &config.openai, "OPENAI_API_KEY"),
        ("claude", &config.claude, "ANTHROPIC_API_KEY"),
        ("gemini", &config.gemini, "GEMINI_API_KEY"),
        ("groq", &config.groq, "GROQ_API_KEY"),
        ("ollama", &config.ollama, "(none)"),
    ];

    for (name, pc, env_var) in providers {
        if let Some(p) = pc {
            println!("\n[{}]", name);
            println!("  api_key:     {}", p.api_key.as_deref()
                .map(|k| format!("{}...", &k[..8.min(k.len())]))
                .unwrap_or_else(|| format!("(env: {})", env_var)));
            println!("  model:       {}", p.model.as_deref().unwrap_or("(default)"));
            println!("  max_tokens:  {}", p.max_tokens.map(|t| t.to_string()).unwrap_or_else(|| "(default)".into()));
            println!("  temperature: {}", p.temperature.map(|t| t.to_string()).unwrap_or_else(|| "(default)".into()));
            if let Some(url) = &p.base_url {
                println!("  base_url:    {}", url);
            }
        }
    }

    println!("\nUsage: gitar --provider <name> [command]");
    println!("Priority: CLI args > provider config > env var > defaults");
    Ok(())
}

async fn cmd_models(client: &LlmClient) -> Result<()> {
    println!("Fetching available models...\n");
    let models = client.list_models().await?;

    if models.is_empty() {
        println!("No models found.");
    } else {
        println!("Available models:");
        for model in models {
            println!("  {}", model);
        }
    }
    Ok(())
}

// =============================================================================
// MAIN
// =============================================================================
#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();
    let file_config = Config::load();

    match &cli.command {
        Commands::Init => return cmd_init(&cli, &file_config),
        Commands::Config => return cmd_config(),
        Commands::Hook { command } => return cmd_hook(command.clone()),
        _ => {}
    }

    if !is_git_repo() {
        bail!("Not a git repository");
    }

    let config = ResolvedConfig::new(
        cli.api_key.as_ref(),
        cli.model.as_ref(),
        cli.max_tokens,
        cli.temperature,
        cli.base_url.as_ref(),
        cli.provider.as_ref(),
        cli.base_branch.as_ref(),
        &file_config,
        get_default_branch,
    );
    let client = LlmClient::new(&config)?;

    match cli.command {
        Commands::Commit { push, all, tag, no_tag, write_to, silent } => {
            cmd_commit(&client, push, all, tag && !no_tag, write_to, silent).await?
        }
        Commands::Staged => cmd_staged(&client).await?,
        Commands::Unstaged => cmd_unstaged(&client).await?,
        Commands::History { from, to, since, until, limit, delay } => {
            cmd_history(&client, from, to, since, until, limit, delay).await?
        }
        Commands::Pr { base, to, staged } => cmd_pr(&client, base, to, &config.base_branch, staged).await?,
        Commands::Changelog { from, to, since, until, limit } => {
            cmd_changelog(&client, from, to, since, until, limit).await?
        }
        Commands::Explain { from, to, since, until, staged } => {
            cmd_explain(&client, from, to, since, until, &config.base_branch, staged).await?
        }
        Commands::Version { base, to, current } => cmd_version(&client, base, to, &config.base_branch, current).await?,
        Commands::Init | Commands::Config | Commands::Hook { .. } => unreachable!(),
        Commands::Models => cmd_models(&client).await?,
    }

    Ok(())
}

// =============================================================================
// CLI TESTS
// =============================================================================
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
    fn cli_parses_commit_with_flags() {
        let cli = Cli::try_parse_from(["gitar", "commit", "-p", "-a"]).unwrap();
        if let Commands::Commit { push, all, .. } = cli.command {
            assert!(push);
            assert!(all);
        } else { panic!("Expected Commit command"); }
    }

    #[test]
    fn cli_parses_staged_command() {
        let cli = Cli::try_parse_from(["gitar", "staged"]).unwrap();
        assert!(matches!(cli.command, Commands::Staged));
    }

    #[test]
    fn cli_parses_unstaged_command() {
        let cli = Cli::try_parse_from(["gitar", "unstaged"]).unwrap();
        assert!(matches!(cli.command, Commands::Unstaged));
    }

    #[test]
    fn cli_parses_pr_with_base() {
        let cli = Cli::try_parse_from(["gitar", "pr", "develop"]).unwrap();
        if let Commands::Pr { base, to, staged } = cli.command {
            assert_eq!(base, Some("develop".into()));
            assert!(to.is_none());
            assert!(!staged);
        } else { panic!("Expected Pr command"); }
    }

    #[test]
    fn cli_parses_global_options() {
        let cli = Cli::try_parse_from([
            "gitar", "--model", "gpt-4", "--max-tokens", "2048", "--temperature", "0.5", "staged",
        ]).unwrap();
        assert_eq!(cli.model, Some("gpt-4".into()));
        assert_eq!(cli.max_tokens, Some(2048));
        assert_eq!(cli.temperature, Some(0.5));
    }

    #[test]
    fn cli_parses_init_command() {
        let cli = Cli::try_parse_from(["gitar", "--model", "claude-3", "--base-branch", "develop", "init"]).unwrap();
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
        assert!(matches!(cli.command, Commands::Staged));
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
    fn cli_provider_with_model() {
        let cli = Cli::try_parse_from([
            "gitar", "--provider", "gemini", "--model", "gemini-2.5-pro", "staged",
        ]).unwrap();
        assert_eq!(cli.provider, Some("gemini".into()));
        assert_eq!(cli.model, Some("gemini-2.5-pro".into()));
    }

    #[test]
    fn cli_provider_with_api_key() {
        let cli = Cli::try_parse_from([
            "gitar", "--provider", "groq", "--api-key", "gsk_test123", "staged",
        ]).unwrap();
        assert_eq!(cli.provider, Some("groq".into()));
        assert_eq!(cli.api_key, Some("gsk_test123".into()));
    }

    #[test]
    fn cli_parses_hook_install() {
        let cli = Cli::try_parse_from(["gitar", "hook", "install"]).unwrap();
        assert!(matches!(cli.command, Commands::Hook { command: HookCommands::Install }));
    }

    #[test]
    fn cli_parses_hook_uninstall() {
        let cli = Cli::try_parse_from(["gitar", "hook", "uninstall"]).unwrap();
        assert!(matches!(cli.command, Commands::Hook { command: HookCommands::Uninstall }));
    }

    #[test]
    fn cli_parses_commit_with_write_to() {
        let cli = Cli::try_parse_from(["gitar", "commit", "--write-to", "/tmp/msg", "--silent"]).unwrap();
        if let Commands::Commit { write_to, silent, .. } = cli.command {
            assert_eq!(write_to, Some("/tmp/msg".into()));
            assert!(silent);
        } else { panic!("Expected Commit command"); }
    }

    #[test]
    fn hook_script_unix_contains_marker() {
        assert!(HOOK_SCRIPT_UNIX.contains("gitar-hook"));
    }

    #[test]
    fn hook_script_windows_contains_marker() {
        assert!(HOOK_SCRIPT_WINDOWS.contains("gitar-hook"));
    }

    #[test]
    fn hook_scripts_skip_when_message_provided() {
        assert!(HOOK_SCRIPT_UNIX.contains("COMMIT_SOURCE"));
        assert!(HOOK_SCRIPT_WINDOWS.contains("COMMIT_SOURCE"));
    }

    #[test]
    fn hook_scripts_check_gitar_installed() {
        assert!(HOOK_SCRIPT_UNIX.contains("command -v gitar"));
        assert!(HOOK_SCRIPT_WINDOWS.contains("where gitar"));
    }
}
