// src/command/squash/mod.rs
use anyhow::{bail, Context, Result};

use crate::client::LlmClient;
use crate::color::success;
use crate::context::preset::Preset;
use crate::context::repo::load_all_context;
use crate::context::secret::SecretAction;
use crate::context::template::{commit_system_with_context, COMMIT_USER};
use crate::git::{run_git, run_git_status};
use crate::prompt;

use super::{apply_smart_diff_with_context, AnalysisContext};

pub async fn cmd_squash(
    client: &LlmClient,
    preset: Preset,
    target: String,
    stream: bool,
    algo: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    // Parse target: either a number (e.g., "3") or a ref (e.g., "HEAD~5", "v1.0.0")
    let (base_ref, num_commits) = parse_target(&target)?;

    println!("Analyzing {} commit(s) to squash...\n", num_commits);

    // Get the commits to be squashed
    let commits = get_commits_to_squash(&base_ref, num_commits)?;

    if commits.is_empty() {
        bail!("No commits found to squash");
    }

    // Display commits that will be squashed
    println!("Commits to squash:");
    println!("==================");
    for (idx, commit) in commits.iter().enumerate() {
        println!("{}. {} - {}", idx + 1, commit.hash, commit.subject);
    }
    println!();

    // Get the combined diff for all commits
    let combined_diff = get_combined_diff(&base_ref)?;

    if combined_diff.trim().is_empty() {
        bail!("No changes found in the specified commits");
    }

    // Generate AI commit message for the squashed commit
    println!("Generating unified commit message...\n");

    let context = AnalysisContext::new()
        .with_provider(client.provider())
        .with_model(client.model());

    let diff = apply_smart_diff_with_context(
        &combined_diff,
        max_diff_chars,
        false,
        algo,
        Some(&context),
        secret_action,
    )?;

    let (project_ctx, user_ctx) = load_all_context();
    let system = commit_system_with_context(preset, project_ctx.as_deref(), user_ctx.as_deref());

    let prompt = COMMIT_USER.replace("{diff}", &diff);
    let suggested_message = client.chat(&system, &prompt, stream).await?;

    if stream {
        println!();
    } else {
        println!("{}\n", suggested_message);
    }

    // Confirmation
    println!("{}", "=".repeat(50));

    if !prompt::confirm("Proceed with squash?", false)? {
        println!("Squash cancelled.");
        return Ok(());
    }

    // Perform the squash
    println!("\nSquashing commits...");

    // Reset to base_ref but keep changes staged
    run_git(&["reset", "--soft", &base_ref])?;

    // Commit with the new message
    let full_msg = format!("{} [AI:{}]", suggested_message.trim(), client.model());
    let (out, err, ok) = run_git_status(&["commit", "-m", &full_msg]);

    if !ok {
        println!("{}{}", out, err);
        bail!("Squash commit failed. Your changes are staged. Use 'git reset --soft HEAD@{{1}}' to undo the reset.");
    }

    success(format!(
        "Successfully squashed {} commits into one",
        num_commits
    ));
    println!("{}{}", out, err);

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
            bail!("Cannot squash 0 commits");
        }
        let base_ref = format!("HEAD~{}", num);
        Ok((base_ref, num))
    } else {
        // It's a ref (tag, commit, branch)
        // Count commits between ref and HEAD
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

fn get_commits_to_squash(base_ref: &str, _num: usize) -> Result<Vec<CommitInfo>> {
    let output = run_git(&["log", &format!("{}..HEAD", base_ref), "--format=%H %s"])?;

    let commits: Vec<CommitInfo> = output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() == 2 {
                Some(CommitInfo {
                    hash: parts[0].chars().take(7).collect(),
                    subject: parts[1].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(commits)
}

fn get_combined_diff(base_ref: &str) -> Result<String> {
    run_git(&["diff", &format!("{}..HEAD", base_ref)])
}
