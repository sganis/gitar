// Cargo.toml:
// [package]
// name = "git-ai"
// version = "0.1.0"
// edition = "2021"
//
// [dependencies]
// async-openai = { version = "0.32", features = ["chat-completion", "byot"] }
// tokio = { version = "1", features = ["full"] }
// serde_json = "1"
// clap = { version = "4", features = ["derive"] }
// anyhow = "1"
// dotenvy = "0.15"

use anyhow::{bail, Context, Result};
use async_openai::Client;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::process::Command;

// =============================================================================
// PROMPTS
// =============================================================================

const COMMIT_SYSTEM_PROMPT: &str = r#"You are an expert software engineer who writes clear, informative Git commit messages following best practices.

## Commit Message Format
```
<Type>(<scope>):
<description line 1>
<description line 2 if needed>
<more lines for complex changes>
```

## Types
- Feat: New feature
- Fix: Bug fix
- Refactor: Code restructuring without behavior change
- Docs: Documentation changes
- Style: Formatting, whitespace (no code logic change)
- Test: Adding or modifying tests
- Chore: Build process, dependencies, config
- Perf: Performance improvement

## Rules
1. First line: Type(scope): only, capitalized (no description on this line)
2. Following lines: describe WHAT changed and WHY
3. Scale detail to complexity: simple changes get 1-2 lines, complex changes get more
4. Use imperative mood ("Add" not "Added")
5. Be specific about impact and reasoning
6. Use plain ASCII characters only. Do not use emojis or Unicode symbols.

## Examples

Simple change:
Feat(docker):
Add 'll' alias for directory listing.

Medium change:
Fix(api):
Handle null response from payment gateway.
Prevents 500 errors when gateway times out during peak traffic.

Complex change:
Refactor(auth):
Extract token validation into dedicated middleware.
Centralizes JWT verification logic previously duplicated across 5 controllers.
Adds automatic token refresh for requests within 5 minutes of expiry.
Improves testability by isolating auth concerns.

Analyze the diff carefully. Identify:
- Files changed and their purpose
- The nature of the change (new feature, bug fix, refactor, etc.)
- Any patterns suggesting the intent (error handling, optimization, etc.)"#;

const COMMIT_USER_PROMPT: &str = r#"Generate a commit message for this diff.
First line: Type(scope): only (capitalized, nothing else on this line)
Following lines: describe what and why (1-5 lines depending on complexity)

**Original message (if any):** {original_message}

**Diff:**
```
{diff}
```
Respond with ONLY the commit message (no markdown, no extra explanation)."#;


const QUICK_COMMIT_SYSTEM_PROMPT: &str = r#"You generate concise Git commit messages from diffs.

Rules:
1. Focus on PURPOSE, not file listings
2. Ignore build/minified files
3. Single line, no markdown
4. Be specific
5. Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Examples:
"Add user authentication with OAuth2 support"
"Fix payment timeout with retry logic"
"Refactor database queries for connection pooling"
"#;

const QUICK_COMMIT_USER_PROMPT: &str = r#"Generate a concise single-line commit message.
```
{diff}
```
Respond with ONLY the commit message (single line)."#;

const PR_SYSTEM_PROMPT: &str = r#"Write a PR description.

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Format:
## Summary
Brief overview.

## What Changed
- Key changes

## Why
Motivation.

## Risks
- Issues or "None"

## Testing
- How tested

## Rollout
- Deploy notes or "Standard""#;


const PR_USER_PROMPT: &str = r#"Generate PR description.

**Branch:** {branch}
**Commits:**
{commits}

**Stats:**
{stats}

**Diff:**
```
{diff}
```
"#;

const CHANGELOG_SYSTEM_PROMPT: &str = r#"Create release notes.

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Format:
# Release Notes
## Features
## Fixes
## Improvements
## Breaking Changes
## Infrastructure

Group related changes, omit empty sections."#;


const CHANGELOG_USER_PROMPT: &str = r#"Generate release notes.

**Range:** {range}
**Count:** {count}

**Commits:**
{commits}"#;

const EXPLAIN_SYSTEM_PROMPT: &str = r#"Explain code changes to non-technical stakeholders.
No jargon, focus on user impact, be brief.

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Format:
## What's Changing
Summary.

## User Impact
- Effects

## Risk Level
Low/Medium/High

## Actions
- QA needed"#;

const EXPLAIN_USER_PROMPT: &str = r#"Explain for non-technical person.

**Stats:**
{stats}

**Diff:**
```
{diff}
```"#;

const VERSION_SYSTEM_PROMPT: &str = r#"Recommend semantic version bump.
- MAJOR: Breaking changes
- MINOR: New features
- PATCH: Fixes/refactors

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Output: Recommendation + Reasoning + Breaking: Yes/No"#;

const VERSION_USER_PROMPT: &str = r#"Recommend version bump.

**Current:** {version}
**Diff:**
```
{diff}
```"#;

// =============================================================================
// EXCLUDE PATTERNS
// =============================================================================

const EXCLUDE_PATTERNS: &[&str] = &[
    ":(exclude)*.lock",
    ":(exclude)package-lock.json",
    ":(exclude)yarn.lock",
    ":(exclude)pnpm-lock.yaml",
    ":(exclude)dist/*",
    ":(exclude)build/*",
    ":(exclude)*.min.js",
    ":(exclude)*.min.css",
    ":(exclude)*.map",
    ":(exclude).env*",
    ":(exclude)target/*",
];

// =============================================================================
// GIT UTILITIES
// =============================================================================

fn run_git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("Failed to execute git")?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_status(args: &[&str]) -> (String, String, bool) {
    match Command::new("git").args(args).output() {
        Ok(o) => (
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
            o.status.success(),
        ),
        Err(e) => (String::new(), e.to_string(), false),
    }
}

fn is_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn get_current_branch() -> String {
    run_git(&["branch", "--show-current"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "HEAD".to_string())
}

fn get_default_branch() -> String {
    for b in ["main", "master"] {
        if run_git(&["rev-parse", "--verify", b]).is_ok() {
            return b.to_string();
        }
    }
    "main".to_string()
}

#[derive(Debug)]
struct CommitInfo {
    hash: String,
    author: String,
    date: String,
    message: String,
}

fn get_commit_logs(limit: Option<usize>, since: Option<&str>, range: Option<&str>) -> Result<Vec<CommitInfo>> {
    let mut args_vec: Vec<String> = vec![
        "log".into(),
        "--pretty=format:%H|%an|%ad|%s".into(),
        "--date=iso".into(),
    ];
    if let Some(n) = limit {
        args_vec.push(format!("-n{}", n));
    }
    if let Some(s) = since {
        args_vec.push(format!("--since={}", s));
    }
    if let Some(r) = range {
        args_vec.push(r.to_string());
    }

    let args: Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();
    let output = run_git(&args)?;

    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let p: Vec<&str> = l.splitn(4, '|').collect();
            if p.len() >= 4 {
                Some(CommitInfo {
                    hash: p[0].into(),
                    author: p[1].into(),
                    date: p[2].into(),
                    message: p[3].into(),
                })
            } else {
                None
            }
        })
        .collect())
}

fn get_commit_diff(hash: &str, max_chars: usize) -> Result<Option<String>> {
    let parent_ref = format!("{}^", hash);
    let has_parent = run_git(&["rev-parse", &parent_ref]).is_ok();

    let diff = if has_parent {
        let diff_ref = format!("{}^!", hash);
        let mut args = vec!["diff", &diff_ref, "--unified=3", "--", "."];
        args.extend(EXCLUDE_PATTERNS);
        run_git(&args)?
    } else {
        let mut args = vec!["diff-tree", "--patch", "--unified=3", "--root", hash, "--", "."];
        args.extend(EXCLUDE_PATTERNS);
        run_git(&args)?
    };

    if diff.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(truncate_diff(diff, max_chars)))
}

fn get_diff(target: Option<&str>, staged: bool, max_chars: usize) -> Result<String> {
    let mut args = vec!["diff", "--unified=3"];
    if staged {
        args.push("--cached");
    } else if let Some(t) = target {
        args.push(t);
    }
    args.extend(&["--", "."]);
    args.extend(EXCLUDE_PATTERNS);
    Ok(truncate_diff(run_git(&args)?, max_chars))
}

fn get_diff_stats(target: Option<&str>, staged: bool) -> Result<String> {
    let mut args = vec!["diff", "--stat"];
    if staged {
        args.push("--cached");
    } else if let Some(t) = target {
        args.push(t);
    }
    run_git(&args)
}

fn get_current_version() -> String {
    run_git(&["describe", "--tags", "--abbrev=0"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "0.0.0".to_string())
}

fn truncate_diff(diff: String, max: usize) -> String {
    if diff.len() <= max {
        return diff;
    }
    let mut t = diff[..max].to_string();
    if let Some(p) = t.rfind("\ndiff --git") {
        if p > max / 2 {
            t.truncate(p);
        }
    }
    t.push_str("\n\n[... truncated ...]");
    t
}

// =============================================================================
// AI CLIENT (using BYOT pattern)
// =============================================================================

async fn call_ai(
    client: &Client<async_openai::config::OpenAIConfig>,
    system: &str,
    user: &str,
    model: &str,
    max_tokens: u32,
) -> Result<String> {
    let response: Value = client
        .chat()
        .create_byot(json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ],
            "max_tokens": max_tokens,
            "temperature": 0.3
        }))
        .await
        .context("OpenAI API call failed")?;

    response["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .context("No content in response")
}

// =============================================================================
// CLI
// =============================================================================

#[derive(Parser)]
#[command(name = "git-ai", about = "Git AI Assistant")]
struct Cli {
    #[arg(long, default_value = "gpt-4o")]
    model: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive commit with AI message
    Commit {
        #[arg(short = 'p', long)]
        push: bool,
        #[arg(short = 'a', long)]
        all: bool,
        #[arg(long, default_value = "true")]
        tag: bool,
        #[arg(long = "no-tag")]
        no_tag: bool,
    },
    /// Generate messages for history
    Commits {
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long, default_value = "500")]
        delay: u64,
    },
    /// Generate PR description
    Pr {
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        staged: bool,
    },
    /// Generate changelog
    Changelog {
        #[arg(long)]
        since_tag: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,
    },
    /// Explain for stakeholders
    Explain {
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        staged: bool,
    },
    /// Suggest version bump
    Version {
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        current: Option<String>,
    },
}

// =============================================================================
// COMMANDS
// =============================================================================

async fn cmd_commit(client: &Client<async_openai::config::OpenAIConfig>, model: &str, push: bool, all: bool, tag: bool) -> Result<()> {
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
        println!("Nothing to commit.");
        return Ok(());
    }

    let diff = truncate_diff(diff, 100000);

    let commit_message = loop {
        let prompt = QUICK_COMMIT_USER_PROMPT.replace("{diff}", &diff);
        let msg = call_ai(client, QUICK_COMMIT_SYSTEM_PROMPT, &prompt, model, 200).await?;

        println!("\n{}\n", msg);
        println!("{}", "=".repeat(50));
        println!("  [Enter] Accept | [g] Regenerate | [e] Edit | [other] Cancel");
        println!("{}", "=".repeat(50));
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        match input.as_str() {
            "" => break msg,
            "g" => { println!("Regenerating...\n"); continue; }
            "e" => {
                print!("New message: ");
                io::stdout().flush()?;
                let mut edited = String::new();
                io::stdin().read_line(&mut edited)?;
                break if edited.trim().is_empty() { msg } else { edited.trim().to_string() };
            }
            _ => { println!("Canceled."); return Ok(()); }
        }
    };

    if all {
        println!("Staging all...");
        run_git(&["add", "-A"])?;
    }

    println!("Committing...");
    let full_msg = if tag { format!("{} [AI:{}]", commit_message, model) } else { commit_message };
    let (out, err, ok) = if all {
        run_git_status(&["commit", "-am", &full_msg])
    } else {
        run_git_status(&["commit", "-m", &full_msg])
    };
    println!("{}{}", out, err);

    if !ok {
        println!("Commit failed.");
        return Ok(());
    }

    if push {
        println!("Pushing...");
        let (out, err, _) = run_git_status(&["push"]);
        println!("{}{}", out, err);
    }
    Ok(())
}

async fn cmd_commits(client: &Client<async_openai::config::OpenAIConfig>, model: &str, limit: Option<usize>, since: Option<String>, delay: u64) -> Result<()> {
    println!("Fetching commits...");
    let commits = get_commit_logs(limit, since.as_deref(), None)?;
    if commits.is_empty() { println!("No commits."); return Ok(()); }

    println!("Processing {} commits...\n", commits.len());
    for (i, c) in commits.iter().enumerate() {
        let h = &c.hash[..8.min(c.hash.len())];
        let d = &c.date[..10.min(c.date.len())];
        let a = if c.author.len() > 15 { &c.author[..15] } else { &c.author };
        let m = if c.message.len() > 40 { &c.message[..40] } else { &c.message };
        println!("[{}/{}] {} | {} | {:15} | {}", i+1, commits.len(), h, d, a, m);

        let diff = match get_commit_diff(&c.hash, 12000)? {
            Some(d) if !d.trim().is_empty() => d,
            _ => { println!("  ⚠ No diff"); continue; }
        };

        let prompt = COMMIT_USER_PROMPT.replace("{original_message}", &c.message).replace("{diff}", &diff);
        match call_ai(client, COMMIT_SYSTEM_PROMPT, &prompt, model, 300).await {
            Ok(r) => {
                for (j, l) in r.lines().enumerate() {
                    if !l.trim().is_empty() {
                        println!("{}{}", if j == 0 { "  ✓ " } else { "    " }, l);
                    }
                }
            }
            Err(e) => println!("  ✗ {}", e),
        }
        if i < commits.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
    }
    Ok(())
}

async fn cmd_pr(client: &Client<async_openai::config::OpenAIConfig>, model: &str, base: Option<String>, staged: bool) -> Result<()> {
    let branch = get_current_branch();
    let base = base.unwrap_or_else(get_default_branch);
    println!("PR: {} → {}\n", branch, base);

    let (diff, stats, commits_text) = if staged {
        (get_diff(None, true, 15000)?, get_diff_stats(None, true)?, "(staged)".into())
    } else {
        let target = format!("{}...{}", base, branch);
        let range = format!("{}..{}", base, branch);
        let commits = get_commit_logs(Some(20), None, Some(&range))?;
        let ct = commits.iter().map(|c| format!("- {}", c.message)).collect::<Vec<_>>().join("\n");
        (get_diff(Some(&target), false, 15000)?, get_diff_stats(Some(&target), false)?, if ct.is_empty() { "(none)".into() } else { ct })
    };

    if diff.trim().is_empty() { println!("No changes."); return Ok(()); }

    let prompt = PR_USER_PROMPT.replace("{branch}", &branch).replace("{commits}", &commits_text).replace("{stats}", &stats).replace("{diff}", &diff);
    match call_ai(client, PR_SYSTEM_PROMPT, &prompt, model, 1000).await {
        Ok(r) => println!("{}", r),
        Err(e) => bail!("Failed: {}", e),
    }
    Ok(())
}

async fn cmd_changelog(client: &Client<async_openai::config::OpenAIConfig>, model: &str, since_tag: Option<String>, since: Option<String>, limit: usize) -> Result<()> {
    let (range, display) = if let Some(ref t) = since_tag {
        (Some(format!("{}..HEAD", t)), format!("{} → HEAD", t))
    } else if since.is_some() {
        (None, format!("since {}", since.as_ref().unwrap()))
    } else {
        (None, "recent".into())
    };

    println!("Changelog for {}...\n", display);
    let commits = get_commit_logs(Some(limit), since.as_deref(), range.as_deref())?;
    if commits.is_empty() { println!("No commits."); return Ok(()); }

    let ct = commits.iter().map(|c| format!("- [{}] {}", &c.hash[..8.min(c.hash.len())], c.message)).collect::<Vec<_>>().join("\n");
    let prompt = CHANGELOG_USER_PROMPT.replace("{range}", &display).replace("{count}", &commits.len().to_string()).replace("{commits}", &ct);

    match call_ai(client, CHANGELOG_SYSTEM_PROMPT, &prompt, model, 1500).await {
        Ok(r) => println!("{}", r),
        Err(e) => bail!("Failed: {}", e),
    }
    Ok(())
}

async fn cmd_explain(client: &Client<async_openai::config::OpenAIConfig>, model: &str, base: Option<String>, staged: bool) -> Result<()> {
    let base = base.unwrap_or_else(get_default_branch);
    let branch = get_current_branch();
    println!("Explaining...\n");

    let (diff, stats) = if staged {
        (get_diff(None, true, 15000)?, get_diff_stats(None, true)?)
    } else {
        let target = format!("{}...{}", base, branch);
        (get_diff(Some(&target), false, 15000)?, get_diff_stats(Some(&target), false)?)
    };

    if diff.trim().is_empty() { println!("No changes."); return Ok(()); }

    let prompt = EXPLAIN_USER_PROMPT.replace("{stats}", &stats).replace("{diff}", &diff);
    match call_ai(client, EXPLAIN_SYSTEM_PROMPT, &prompt, model, 800).await {
        Ok(r) => println!("{}", r),
        Err(e) => bail!("Failed: {}", e),
    }
    Ok(())
}

async fn cmd_version(client: &Client<async_openai::config::OpenAIConfig>, model: &str, base: Option<String>, current: Option<String>) -> Result<()> {
    let base = base.unwrap_or_else(get_default_branch);
    let branch = get_current_branch();
    let current = current.unwrap_or_else(get_current_version);
    println!("Version analysis (current: {})...\n", current);

    let target = format!("{}...{}", base, branch);
    let diff = get_diff(Some(&target), false, 15000)?;
    if diff.trim().is_empty() { println!("No changes."); return Ok(()); }

    let prompt = VERSION_USER_PROMPT.replace("{version}", &current).replace("{diff}", &diff);
    match call_ai(client, VERSION_SYSTEM_PROMPT, &prompt, model, 400).await {
        Ok(r) => println!("{}", r),
        Err(e) => bail!("Failed: {}", e),
    }
    Ok(())
}

// =============================================================================
// MAIN
// =============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    if std::env::var("OPENAI_API_KEY").is_err() {
        bail!("OPENAI_API_KEY not set");
    }
    if !is_git_repo() {
        bail!("Not a git repository");
    }

    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Commands::Commit { push, all, tag, no_tag } => cmd_commit(&client, &cli.model, push, all, tag && !no_tag).await?,
        Commands::Commits { limit, since, delay } => cmd_commits(&client, &cli.model, limit, since, delay).await?,
        Commands::Pr { base, staged } => cmd_pr(&client, &cli.model, base, staged).await?,
        Commands::Changelog { since_tag, since, limit } => cmd_changelog(&client, &cli.model, since_tag, since, limit).await?,
        Commands::Explain { base, staged } => cmd_explain(&client, &cli.model, base, staged).await?,
        Commands::Version { base, current } => cmd_version(&client, &cli.model, base, current).await?,
    }
    Ok(())
}