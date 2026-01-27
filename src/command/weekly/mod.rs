// src/command/weekly/mod.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::context::load_all_context;
use crate::context::secret::SecretAction;
use crate::context::template::{get_weekly_user, weekly_system_with_context};
use crate::git::{build_diff_target, get_commit_logs, get_diff, get_diff_stats};

use super::{apply_smart_diff_with_context, AnalysisContext};

const MAX_COMMITS: usize = 50;
const MAX_DIFF_CHARS: usize = 8000; // Tighter limit for weekly due to long system prompt

pub async fn cmd_weekly(
    client: &LlmClient,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    base_branch: &str,
    staged: bool,
    stream: bool,
    alg: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    let context = AnalysisContext::new()
        .with_provider(client.provider())
        .with_model(client.model());

    // Use tighter diff limit for weekly (long system prompt needs room)
    let effective_max_chars = max_diff_chars.min(MAX_DIFF_CHARS);

    let (diff, stats, display) = if staged {
        println!("Generating weekly highlights from staged changes...\n");
        let raw_diff = get_diff(None, true, usize::MAX)?;
        let diff = apply_smart_diff_with_context(
            &raw_diff,
            effective_max_chars,
            false,
            alg,
            Some(&context),
            secret_action,
        )?;
        (diff, get_diff_stats(None, true)?, "staged".to_string())
    } else {
        // Determine effective 'from': use provided, or latest tag, or None (working tree)
        // Cap commits at MAX_COMMITS to avoid token limits
        let (effective_from, display) = match (&from, &since, &until) {
            (Some(r), _, _) => (Some(r.clone()), format!("{}..HEAD", r)),
            (None, Some(s), Some(u)) => {
                let commits = get_commit_logs(None, Some(s), Some(u), None)?;
                let (from_hash, display) = cap_commits(&commits, &format!("--since {} --until {}", s, u));
                (from_hash, display)
            }
            (None, Some(s), None) => {
                let commits = get_commit_logs(None, Some(s), None, None)?;
                let (from_hash, display) = cap_commits(&commits, &format!("--since {}", s));
                (from_hash, display)
            }
            (None, None, Some(u)) => {
                let commits = get_commit_logs(None, None, Some(u), None)?;
                let (from_hash, display) = cap_commits(&commits, &format!("--until {}", u));
                (from_hash, display)
            }
            (None, None, None) => {
                // Default: last 7 days
                let commits = get_commit_logs(None, Some("7 days ago"), None, None)?;
                let (from_hash, display) = cap_commits(&commits, "last 7 days");
                (from_hash, display)
            }
        };

        println!("Generating weekly highlights ({})...\n", display);

        let diff_target = build_diff_target(effective_from.as_deref(), to.as_deref(), base_branch);
        let diff_target_ref = if diff_target.is_empty() {
            None
        } else {
            Some(diff_target.as_str())
        };

        let raw_diff = get_diff(diff_target_ref, false, usize::MAX)?;
        let diff = apply_smart_diff_with_context(
            &raw_diff,
            effective_max_chars,
            false,
            alg,
            Some(&context),
            secret_action,
        )?;
        (diff, get_diff_stats(diff_target_ref, false)?, display)
    };

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = get_weekly_user()
        .replace("{range}", if staged { "staged" } else { &display })
        .replace("{stats}", &stats)
        .replace("{diff}", &diff);

    let (project_ctx, user_ctx) = load_all_context();
    let system = weekly_system_with_context(project_ctx.as_deref(), user_ctx.as_deref());

    // Weekly always uses non-streaming for reliability with long prompts
    let _ = stream; // Ignore passed stream parameter
    let r = client.chat(&system, &prompt, false).await?;
    if r.trim().is_empty() {
        println!("No response from model. Try a smaller date range or fewer commits.");
    } else {
        println!("{}", r);
    }
    Ok(())
}

fn cap_commits(
    commits: &[crate::git::CommitInfo],
    range_desc: &str,
) -> (Option<String>, String) {
    use crate::color::{styled, warning_style};

    let total = commits.len();
    if total <= MAX_COMMITS {
        let display = format!("{} ({} commits)", range_desc, total);
        (commits.last().map(|c| c.hash.clone()), display)
    } else {
        // Take only the most recent MAX_COMMITS (commits are newest-first)
        let capped = &commits[..MAX_COMMITS];
        let display = format!(
            "{} ({} of {} commits, capped at {})",
            range_desc, MAX_COMMITS, total, MAX_COMMITS
        );
        eprintln!(
            "{} {} commits in range but only using most recent {}. Use a narrower --since window for complete coverage.",
            styled("[WARN]", warning_style()),
            total,
            MAX_COMMITS
        );
        (capped.last().map(|c| c.hash.clone()), display)
    }
}
