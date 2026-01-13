use anyhow::{bail, Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::process::Command;

// =============================================================================
// PROMPTS
// =============================================================================

const COMMIT_SYSTEM_PROMPT: &str = r#"You are an expert software engineer who writes clear, informative Git commit messages.

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
5. Be specific about impact and reasoning"#;

const COMMIT_USER_PROMPT: &str = r#"Generate a commit message for this diff.
First line: Type(scope): only (capitalized, nothing else on this line)
Following lines: describe what and why (1-5 lines depending on complexity)

**Original message (if any):** {original_message}

**Diff:**
```
{diff}
```

Respond with ONLY the commit message (no markdown, no extra explanation)."#;

const QUICK_COMMIT_SYSTEM_PROMPT: &str = r#"You generate concise, meaningful Git commit messages from diffs.

## Rules
1. Focus on PURPOSE of changes, not file listings
2. Ignore build directories, minified files, compiled output
3. Only analyze actual source code changes
4. Ignore changes to build assets (auto-generated)
5. Keep response in a SINGLE LINE, no markdown
6. Understand features added and bugs fixed
7. Be specific about what changed

## Examples
"Add user authentication with OAuth2 support and session management"
"Fix payment timeout bug by adding retry logic with exponential backoff"
"Refactor database queries to use connection pooling for better performance"
"#;

const QUICK_COMMIT_USER_PROMPT: &str = r#"Analyze this git diff and generate a concise commit message in a single line.

```
{diff}
```

Respond with ONLY the commit message (single line, no markdown)."#;

const PR_SYSTEM_PROMPT: &str = r#"You are a senior engineer writing a PR description for code review.

## Output Format
## Summary
Brief 1-2 sentence overview of the change.

## What Changed
- Bullet points of key changes
- Be specific about files/components affected

## Why
Motivation, context, or issue being solved.

## Risks & Considerations
- Potential issues or areas needing careful review
- "None identified" if truly low-risk

## Testing
- How this was tested
- Suggested manual testing steps

## Rollout Notes
- Any deployment considerations
- "Standard deployment" if nothing special

Be concise but thorough."#;

const PR_USER_PROMPT: &str = r#"Generate a PR description for this diff.

**Branch:** {branch}
**Commits:**
{commits}

**File stats:**
{stats}

**Diff:**
```
{diff}
```

Respond with the PR description in the format specified."#;

const CHANGELOG_SYSTEM_PROMPT: &str = r#"You are a technical writer creating release notes from Git commits.

## Output Format
# Release Notes

## ✨ New Features
- Feature descriptions grouped logically

## 🐛 Bug Fixes
- Fix descriptions

## 🔧 Improvements
- Refactors, performance, DX improvements

## ⚠️ Breaking Changes
- Any backwards-incompatible changes

## 🏗️ Infrastructure
- CI/CD, dependencies, config changes

Rules:
1. Group related changes together
2. Write for end-users/stakeholders, not devs
3. Skip trivial changes
4. Highlight breaking changes prominently
5. Omit empty sections"#;

const CHANGELOG_USER_PROMPT: &str = r#"Generate release notes from these commits.

**Range:** {range}
**Commit count:** {count}

**Commits with messages:**
{commits}

Respond with release notes in the format specified."#;

const EXPLAIN_SYSTEM_PROMPT: &str = r#"You are explaining code changes to a non-technical stakeholder.

## Rules
1. NO jargon - translate technical terms
2. Focus on USER IMPACT
3. Be brief - 3-5 bullet points max
4. Call out anything visible to users
5. Mention if QA/testing is recommended

## Output Format
## What's Changing
Brief plain-English summary.

## User Impact
- How this affects the product/users
- Visible changes (if any)

## Risk Level
Low / Medium / High + brief explanation

## Recommended Actions
- Any QA, communication, or documentation needed"#;

const EXPLAIN_USER_PROMPT: &str = r#"Explain this code change for a non-technical person.

**Diff stats:**
{stats}

**Diff:**
```
{diff}
```

Respond with a plain-English explanation (no code, no jargon)."#;

const VERSION_SYSTEM_PROMPT: &str = r#"You analyze code changes to recommend semantic version bumps.

## Semantic Versioning Rules
- MAJOR (X.0.0): Breaking changes
- MINOR (0.X.0): New features, deprecations
- PATCH (0.0.X): Bug fixes, internal refactors

## Output Format
Recommendation: MAJOR|MINOR|PATCH

Reasoning:
- Key point 1
- Key point 2

Breaking changes: Yes/No

Be conservative - when in doubt, go higher."#;

const VERSION_USER_PROMPT: &str = r#"Analyze this diff and recommend a semantic version bump.

**Current version:** {version}
**Diff:**
```
{diff}
```

Respond with your recommendation and reasoning."#;

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
    ":(exclude)*.pyc",
    ":(exclude)__pycache__/*",
    ":(exclude)target/*",
];

// =============================================================================
// GIT UTILITIES
// =============================================================================

fn run_git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("Failed to execute git command")?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_with_status(args: &[&str]) -> (String, String, bool) {
    match Command::new("git").args(args).output() {
        Ok(output) => (
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
            output.status.success(),
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
    for branch in ["main", "master"] {
        if run_git(&["rev-parse", "--verify", branch]).is_ok() {
            return branch.to_string();
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
    let mut args = vec!["log", "--pretty=format:%H|%an|%ad|%s", "--date=iso"];
    
    let limit_str;
    if let Some(n) = limit {
        limit_str = format!("-n{}", n);
        args.push(&limit_str);
    }
    
    let since_str;
    if let Some(s) = since {
        since_str = format!("--since={}", s);
        args.push(&since_str);
    }
    
    if let Some(r) = range {
        args.push(r);
    }
    
    let output = run_git(&args)?;
    
    let commits = output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() >= 4 {
                Some(CommitInfo {
                    hash: parts[0].to_string(),
                    author: parts[1].to_string(),
                    date: parts[2].to_string(),
                    message: parts[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect();
    
    Ok(commits)
}

fn get_commit_diff(hash: &str, max_chars: usize) -> Result<Option<String>> {
    let has_parent = run_git(&["rev-parse", &format!("{}^", hash)]).is_ok();
    
    let mut args: Vec<&str> = if has_parent {
        vec!["diff", &format!("{}^!", hash), "--unified=3", "--"]
    } else {
        vec!["diff-tree", "--patch", "--unified=3", "--root", hash, "--"]
    };
    
    args.push(".");
    args.extend(EXCLUDE_PATTERNS.iter());
    
    let diff = run_git(&args)?;
    
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
    
    args.push("--");
    args.push(".");
    args.extend(EXCLUDE_PATTERNS.iter());
    
    let diff = run_git(&args)?;
    Ok(truncate_diff(diff, max_chars))
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

fn truncate_diff(diff: String, max_chars: usize) -> String {
    if diff.len() <= max_chars {
        return diff;
    }
    
    let mut trunc = diff[..max_chars].to_string();
    if let Some(pos) = trunc.rfind("\ndiff --git") {
        if pos > max_chars / 2 {
            trunc.truncate(pos);
        }
    }
    trunc.push_str("\n\n[... diff truncated ...]");
    trunc
}

// =============================================================================
// AI CLIENT
// =============================================================================

async fn call_ai(
    client: &Client<OpenAIConfig>,
    system_prompt: &str,
    user_prompt: &str,
    model: &str,
    max_tokens: u32,
) -> Result<String> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .max_tokens(max_tokens)
        .temperature(0.3f32)
        .messages(vec![
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(system_prompt)
                    .build()?,
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(user_prompt)
                    .build()?,
            ),
        ])
        .build()?;

    let response = client.chat().create(request).await?;

    response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .context("No response from API")
}

// =============================================================================
// CLI
// =============================================================================

#[derive(Parser)]
#[command(name = "git-ai")]
#[command(about = "Git AI Assistant - generate commit messages, PR descriptions, changelogs, and more")]
struct Cli {
    #[arg(long, default_value = "gpt-4o")]
    model: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive commit with AI-generated message
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
    /// Generate commit messages for history
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
    /// Generate release notes / changelog
    Changelog {
        #[arg(long)]
        since_tag: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,
    },
    /// Explain changes for PM/stakeholders
    Explain {
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        staged: bool,
    },
    /// Suggest semantic version bump
    Version {
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        current: Option<String>,
    },
}

// =============================================================================
// COMMAND IMPLEMENTATIONS
// =============================================================================

async fn cmd_commit(
    client: &Client<OpenAIConfig>,
    model: &str,
    push: bool,
    all: bool,
    tag: bool,
) -> Result<()> {
    // Get both staged and unstaged changes
    let staged = run_git(&["diff", "--cached"]).unwrap_or_default();
    let unstaged = run_git(&["diff"]).unwrap_or_default();

    let mut diff = String::new();
    if !staged.trim().is_empty() {
        diff.push_str(&staged);
    }
    if !unstaged.trim().is_empty() {
        if !diff.is_empty() {
            diff.push('\n');
        }
        diff.push_str(&unstaged);
    }

    if diff.trim().is_empty() {
        println!("Nothing to commit.");
        return Ok(());
    }

    // Truncate if too long
    let diff = truncate_diff(diff, 100000);

    let commit_message = loop {
        let prompt = QUICK_COMMIT_USER_PROMPT.replace("{diff}", &diff);

        let message = call_ai(client, QUICK_COMMIT_SYSTEM_PROMPT, &prompt, model, 200).await?;

        println!("\n{}\n", message);
        println!("{}", "=".repeat(50));
        println!("  Is this message good?");
        println!("{}", "=".repeat(50));
        println!("  [  Enter  ] → Accept and commit");
        println!("  [    g    ] → Generate a new message");
        println!("  [    e    ] → Edit message manually");
        println!("  [ Any key ] → Cancel");
        println!("{}", "=".repeat(50));

        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        match input.as_str() {
            "" => break message,
            "g" => {
                println!("Regenerating...\n");
                continue;
            }
            "e" => {
                println!("Current: {}", message);
                print!("New message: ");
                io::stdout().flush()?;
                let mut edited = String::new();
                io::stdin().read_line(&mut edited)?;
                let edited = edited.trim();
                if edited.is_empty() {
                    break message;
                } else {
                    break edited.to_string();
                }
            }
            _ => {
                println!("Commit canceled.");
                return Ok(());
            }
        }
    };

    if all {
        println!("Staging all changes...");
        run_git(&["add", "-A"])?;
    }

    println!("Committing...");

    let full_message = if tag {
        format!("{} [AI:{}]", commit_message, model)
    } else {
        commit_message
    };

    let (stdout, stderr, success) = if all {
        run_git_with_status(&["commit", "-am", &full_message])
    } else {
        run_git_with_status(&["commit", "-m", &full_message])
    };

    println!("{}", stdout);
    if !stderr.is_empty() {
        println!("{}", stderr);
    }

    if !success {
        println!("Commit failed.");
        return Ok(());
    }

    if push {
        println!("Pushing...");
        let (stdout, stderr, _) = run_git_with_status(&["push"]);
        println!("{}", stdout);
        if !stderr.is_empty() {
            println!("{}", stderr);
        }
    }

    Ok(())
}

async fn cmd_commits(
    client: &Client<OpenAIConfig>,
    model: &str,
    limit: Option<usize>,
    since: Option<String>,
    delay: u64,
) -> Result<()> {
    println!("Fetching commit history...");

    let commits = get_commit_logs(limit, since.as_deref(), None)?;

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    println!("Processing {} commits...\n", commits.len());

    for (i, commit) in commits.iter().enumerate() {
        let short_hash = &commit.hash[..8.min(commit.hash.len())];
        let date_short = &commit.date[..10.min(commit.date.len())];
        let author = if commit.author.len() > 15 {
            &commit.author[..15]
        } else {
            &commit.author
        };
        let msg = if commit.message.len() > 40 {
            &commit.message[..40]
        } else {
            &commit.message
        };

        println!(
            "[{}/{}] {} | {} | {:15} | {}",
            i + 1,
            commits.len(),
            short_hash,
            date_short,
            author,
            msg
        );

        let diff = match get_commit_diff(&commit.hash, 12000)? {
            Some(d) if !d.trim().is_empty() => d,
            _ => {
                println!("  ⚠ No diff available, skipping");
                continue;
            }
        };

        let prompt = COMMIT_USER_PROMPT
            .replace("{original_message}", &commit.message)
            .replace("{diff}", &diff);

        match call_ai(client, COMMIT_SYSTEM_PROMPT, &prompt, model, 300).await {
            Ok(response) => {
                for (j, line) in response.lines().enumerate() {
                    if !line.trim().is_empty() {
                        if j == 0 {
                            println!("  ✓ {}", line);
                        } else {
                            println!("    {}", line);
                        }
                    }
                }
            }
            Err(e) => println!("  ✗ Failed: {}", e),
        }

        if i < commits.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
    }

    Ok(())
}

async fn cmd_pr(
    client: &Client<OpenAIConfig>,
    model: &str,
    base: Option<String>,
    staged: bool,
) -> Result<()> {
    let branch = get_current_branch();
    let base = base.unwrap_or_else(get_default_branch);

    println!("Generating PR description: {} → {}\n", branch, base);

    let (diff, stats, commits_text) = if staged {
        (
            get_diff(None, true, 15000)?,
            get_diff_stats(None, true)?,
            "(staged changes)".to_string(),
        )
    } else {
        let target = format!("{}...{}", base, branch);
        let range = format!("{}..{}", base, branch);
        let commits = get_commit_logs(Some(20), None, Some(&range))?;
        let commits_text = commits
            .iter()
            .map(|c| format!("- {}", c.message))
            .collect::<Vec<_>>()
            .join("\n");
        (
            get_diff(Some(&target), false, 15000)?,
            get_diff_stats(Some(&target), false)?,
            if commits_text.is_empty() {
                "(no commits)".to_string()
            } else {
                commits_text
            },
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

    match call_ai(client, PR_SYSTEM_PROMPT, &prompt, model, 1000).await {
        Ok(response) => println!("{}", response),
        Err(e) => bail!("Failed to generate PR description: {}", e),
    }

    Ok(())
}

async fn cmd_changelog(
    client: &Client<OpenAIConfig>,
    model: &str,
    since_tag: Option<String>,
    since: Option<String>,
    limit: usize,
) -> Result<()> {
    let (range, range_display) = if let Some(ref tag) = since_tag {
        (Some(format!("{}..HEAD", tag)), format!("{} → HEAD", tag))
    } else if since.is_some() {
        (None, format!("since {}", since.as_ref().unwrap()))
    } else {
        (None, "recent commits".to_string())
    };

    println!("Generating changelog for {}...\n", range_display);

    let commits = get_commit_logs(Some(limit), since.as_deref(), range.as_deref())?;

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    let commits_text = commits
        .iter()
        .map(|c| format!("- [{}] {}", &c.hash[..8.min(c.hash.len())], c.message))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = CHANGELOG_USER_PROMPT
        .replace("{range}", &range_display)
        .replace("{count}", &commits.len().to_string())
        .replace("{commits}", &commits_text);

    match call_ai(client, CHANGELOG_SYSTEM_PROMPT, &prompt, model, 1500).await {
        Ok(response) => println!("{}", response),
        Err(e) => bail!("Failed to generate changelog: {}", e),
    }

    Ok(())
}

async fn cmd_explain(
    client: &Client<OpenAIConfig>,
    model: &str,
    base: Option<String>,
    staged: bool,
) -> Result<()> {
    let base = base.unwrap_or_else(get_default_branch);
    let branch = get_current_branch();

    println!("Generating PM-friendly explanation...\n");

    let (diff, stats) = if staged {
        (get_diff(None, true, 15000)?, get_diff_stats(None, true)?)
    } else {
        let target = format!("{}...{}", base, branch);
        (
            get_diff(Some(&target), false, 15000)?,
            get_diff_stats(Some(&target), false)?,
        )
    };

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = EXPLAIN_USER_PROMPT
        .replace("{stats}", &stats)
        .replace("{diff}", &diff);

    match call_ai(client, EXPLAIN_SYSTEM_PROMPT, &prompt, model, 800).await {
        Ok(response) => println!("{}", response),
        Err(e) => bail!("Failed to generate explanation: {}", e),
    }

    Ok(())
}

async fn cmd_version(
    client: &Client<OpenAIConfig>,
    model: &str,
    base: Option<String>,
    current: Option<String>,
) -> Result<()> {
    let base = base.unwrap_or_else(get_default_branch);
    let branch = get_current_branch();
    let current = current.unwrap_or_else(get_current_version);

    println!("Analyzing changes for version bump (current: {})...\n", current);

    let target = format!("{}...{}", base, branch);
    let diff = get_diff(Some(&target), false, 15000)?;

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = VERSION_USER_PROMPT
        .replace("{version}", &current)
        .replace("{diff}", &diff);

    match call_ai(client, VERSION_SYSTEM_PROMPT, &prompt, model, 400).await {
        Ok(response) => println!("{}", response),
        Err(e) => bail!("Failed to analyze version: {}", e),
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
        bail!("OPENAI_API_KEY not found.\nCreate a .env file with: OPENAI_API_KEY=your-key-here");
    }

    if !is_git_repo() {
        bail!("Not a git repository");
    }

    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Commands::Commit { push, all, tag, no_tag } => {
            let use_tag = tag && !no_tag;
            cmd_commit(&client, &cli.model, push, all, use_tag).await?;
        }
        Commands::Commits { limit, since, delay } => {
            cmd_commits(&client, &cli.model, limit, since, delay).await?;
        }
        Commands::Pr { base, staged } => {
            cmd_pr(&client, &cli.model, base, staged).await?;
        }
        Commands::Changelog { since_tag, since, limit } => {
            cmd_changelog(&client, &cli.model, since_tag, since, limit).await?;
        }
        Commands::Explain { base, staged } => {
            cmd_explain(&client, &cli.model, base, staged).await?;
        }
        Commands::Version { base, current } => {
            cmd_version(&client, &cli.model, base, current).await?;
        }
    }

    Ok(())
}