// src/command/pr.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::context::load_all_context;
use crate::git::{
    build_diff_target, build_range, get_commit_logs, get_current_branch, get_diff, get_diff_stats,
};
use crate::prompt::secret::SecretAction;
use crate::prompt::template::{pr_system_with_context, PR_USER};

use super::apply_smart_diff;

pub async fn cmd_pr(
    client: &LlmClient,
    base: Option<String>,
    to: Option<String>,
    base_branch: &str,
    staged: bool,
    stream: bool,
    alg: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    let branch = to.clone().unwrap_or_else(get_current_branch);
    let target_base = base.as_deref().unwrap_or(base_branch);

    println!("PR: {} -> {}\n", branch, target_base);

    let (diff, stats, commits_text) = if staged {
        let raw_diff = get_diff(None, true, usize::MAX)?;
        let diff = apply_smart_diff(&raw_diff, max_diff_chars, false, alg, secret_action)?;
        (diff, get_diff_stats(None, true)?, "(staged changes)".into())
    } else {
        let diff_target = build_diff_target(base.as_deref(), to.as_deref(), base_branch);
        let range = build_range(base.as_deref(), to.as_deref(), base_branch);

        let commits = get_commit_logs(Some(20), None, None, range.as_deref())?;
        let ct = commits
            .iter()
            .map(|c| format!("- {}", c.message))
            .collect::<Vec<_>>()
            .join("\n");

        let diff_target_ref = if diff_target.is_empty() {
            None
        } else {
            Some(diff_target.as_str())
        };

        let raw_diff = get_diff(diff_target_ref, false, usize::MAX)?;
        let diff = apply_smart_diff(&raw_diff, max_diff_chars, false, alg, secret_action)?;

        (
            diff,
            get_diff_stats(diff_target_ref, false)?,
            if ct.is_empty() {
                "(no commits)".into()
            } else {
                ct
            },
        )
    };

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = PR_USER
        .replace("{branch}", &branch)
        .replace("{commits}", &commits_text)
        .replace("{stats}", &stats)
        .replace("{diff}", &diff);

    let (project_ctx, user_ctx) = load_all_context();
    let system = pr_system_with_context(project_ctx.as_deref(), user_ctx.as_deref());

    let r = client.chat(&system, &prompt, stream).await?;
    if stream {
        println!();
    } else {
        println!("{}", r);
    }
    Ok(())
}
