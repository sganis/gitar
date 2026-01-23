// src/command/rewrite/mod.rs
use anyhow::{bail, Context, Result};
use std::io::{self, Write};

use crate::client::LlmClient;
use crate::git::run_git;
use crate::context::preset::Preset;
use crate::context::secret::SecretAction;
use crate::context::template::{commit_system_with_context, COMMIT_USER};
use crate::context::repo::load_all_context;

use super::{apply_smart_diff_with_context, AnalysisContext};

pub async fn cmd_rewrite(
    client: &LlmClient,
    preset: Preset,
    target: String,
    stream: bool,
    algo: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    // Parse target: either a number (e.g., "5") or a ref (e.g., "HEAD~5", "v1.0.0")
    let (base_ref, num_commits) = parse_target(&target)?;

    println!("Analyzing {} commit(s) to rewrite...\n", num_commits);

    // Get the commits to be rewritten
    let commits = get_commits_to_rewrite(&base_ref)?;

    if commits.is_empty() {
        bail!("No commits found to rewrite");
    }

    // Display commits that will be rewritten
    println!("Commits to rewrite:");
    println!("===================");
    for (idx, commit) in commits.iter().enumerate() {
        println!("{}. {} - {}", idx + 1, commit.hash, commit.subject);
    }
    println!();

    // Warning about history rewriting
    println!("⚠️  WARNING: This will rewrite git history!");
    println!("   Only do this if you haven't pushed these commits,");
    println!("   or if you understand the implications of force-pushing.\n");

    print!("Continue? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Rewrite cancelled.");
        return Ok(());
    }

    println!();

    // Process each commit interactively
    let mut new_messages = Vec::new();

    for (idx, commit) in commits.iter().enumerate() {
        println!("─────────────────────────────────────────────────────────");
        println!("Commit {}/{}: {}", idx + 1, commits.len(), commit.hash);
        println!("Current message: {}", commit.subject);
        println!();

        // Get the diff for this specific commit
        let diff = get_commit_diff(&commit.hash)?;

        if diff.trim().is_empty() {
            println!("No changes in this commit, keeping original message.\n");
            new_messages.push(commit.subject.clone());
            continue;
        }

        // Generate new message
        let new_message = loop {
            println!("Generating new message...");

            let context = AnalysisContext::new()
                .with_provider(client.provider())
                .with_model(client.model());

            let shaped_diff = apply_smart_diff_with_context(
                &diff,
                max_diff_chars,
                false,
                algo,
                Some(&context),
                secret_action,
            )?;

            let (project_ctx, user_ctx) = load_all_context();
            let system = commit_system_with_context(
                preset,
                project_ctx.as_deref(),
                user_ctx.as_deref(),
            );

            let prompt = COMMIT_USER.replace("{diff}", &shaped_diff);
            let msg = client.chat(&system, &prompt, stream).await?;

            if stream {
                println!();
            } else {
                println!("\nProposed: {}\n", msg);
            }

            println!("{}", "=".repeat(50));
            println!("  [Enter] Accept | [k] Keep original | [g] Regenerate | [e] Edit | [s] Skip all");
            println!("{}", "=".repeat(50));
            print!("> ");
            io::stdout().flush()?;

            let mut choice = String::new();
            io::stdin().read_line(&mut choice)?;

            match choice.trim().to_lowercase().as_str() {
                "" => break msg.trim().to_string(),
                "k" => {
                    println!("Keeping original message.\n");
                    break commit.subject.clone();
                }
                "g" => {
                    println!("Regenerating...\n");
                    continue;
                }
                "e" => {
                    print!("New message: ");
                    io::stdout().flush()?;
                    let mut ed = String::new();
                    io::stdin().read_line(&mut ed)?;
                    break if ed.trim().is_empty() {
                        msg.trim().to_string()
                    } else {
                        ed.trim().to_string()
                    };
                }
                "s" => {
                    println!("Skipping remaining commits. Keeping all original messages.");
                    // Add remaining original messages
                    for remaining in &commits[idx..] {
                        new_messages.push(remaining.subject.clone());
                    }
                    // Execute rewrite with what we have
                    execute_rewrite(&base_ref, &commits, &new_messages)?;
                    return Ok(());
                }
                _ => {
                    println!("Rewrite cancelled.");
                    return Ok(());
                }
            }
        };

        new_messages.push(new_message);
        println!();
    }

    // Execute the rewrite
    execute_rewrite(&base_ref, &commits, &new_messages)?;

    Ok(())
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

struct CommitInfo {
    hash: String,
    subject: String,
}

fn parse_target(target: &str) -> Result<(String, usize)> {
    // Try to parse as a number first
    if let Ok(num) = target.parse::<usize>() {
        if num == 0 {
            bail!("Cannot rewrite 0 commits");
        }
        let base_ref = format!("HEAD~{}", num);
        Ok((base_ref, num))
    } else {
        // It's a ref (tag, commit, branch)
        let count_output = run_git(&["rev-list", "--count", &format!("{}..HEAD", target)])
            .context("Failed to count commits. Check if the ref exists.")?;

        let num = count_output
            .trim()
            .parse::<usize>()
            .context("Failed to parse commit count")?;

        if num == 0 {
            bail!("No commits found between {} and HEAD", target);
        }

        Ok((target.to_string(), num))
    }
}

fn get_commits_to_rewrite(base_ref: &str) -> Result<Vec<CommitInfo>> {
    let output = run_git(&[
        "log",
        &format!("{}..HEAD", base_ref),
        "--format=%H %s",
        "--reverse", // Oldest first for rewriting
    ])?;

    let commits: Vec<CommitInfo> = output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() == 2 {
                Some(CommitInfo {
                    hash: parts[0].to_string(),
                    subject: parts[1].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(commits)
}

fn get_commit_diff(commit_hash: &str) -> Result<String> {
    run_git(&["show", "--format=", commit_hash])
}

fn execute_rewrite(
    base_ref: &str,
    commits: &[CommitInfo],
    new_messages: &[String],
) -> Result<()> {
    if commits.len() != new_messages.len() {
        bail!(
            "Mismatch: {} commits but {} messages",
            commits.len(),
            new_messages.len()
        );
    }

    println!("Rewriting commit history...");

    // Create a temporary file with the new messages
    let _msg_map: Vec<String> = commits
        .iter()
        .zip(new_messages.iter())
        .map(|(c, m)| format!("{}\n{}", c.hash, m))
        .collect();

    // Use git filter-branch with msg-filter
    // Actually, we'll use a simpler approach: git rebase -i with EDITOR
    // For now, let's use git commit --amend in a loop with reset

    // This is complex - using reset --soft + commit repeatedly
    println!("Using git reset/commit to rewrite {} commits...", commits.len());

    // Reset to base
    run_git(&["reset", "--soft", base_ref])?;

    // Re-commit with new messages
    for (idx, (commit, new_msg)) in commits.iter().zip(new_messages.iter()).enumerate() {
        // Get the files from the original commit
        let files = run_git(&["diff-tree", "--no-commit-id", "--name-only", "-r", &commit.hash])?;

        // Stage all the files
        for file in files.lines() {
            if !file.trim().is_empty() {
                let _ = run_git(&["add", file.trim()]);
            }
        }

        // Commit with new message
        run_git(&["commit", "-m", new_msg])
            .with_context(|| format!("Failed to commit step {}/{}", idx + 1, commits.len()))?;
    }

    println!("\n✓ Successfully rewrote {} commits", commits.len());
    println!("  Old: {}..HEAD", base_ref);
    println!("  New: History rewritten with AI-generated messages");
    println!("\nNote: If you've already pushed, you'll need to force push:");
    println!("  git push --force-with-lease");

    Ok(())
}
