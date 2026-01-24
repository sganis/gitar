// src/command/explain/mod.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::context::load_all_context;
use crate::context::secret::SecretAction;
use crate::context::template::{explain_system_with_context, EXPLAIN_USER};
use crate::git::{build_diff_target, get_commit_logs, get_diff, get_diff_stats};

use super::{apply_smart_diff_with_context, get_latest_tag, AnalysisContext};

pub async fn cmd_explain(
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

    let (diff, stats, display) = if staged {
        println!("Explaining staged changes...\n");
        let raw_diff = get_diff(None, true, usize::MAX)?;
        let diff = apply_smart_diff_with_context(
            &raw_diff,
            max_diff_chars,
            false,
            alg,
            Some(&context),
            secret_action,
        )?;
        (diff, get_diff_stats(None, true)?, "staged".to_string())
    } else {
        // Determine effective 'from': use provided, or latest tag, or None (working tree)
        let (effective_from, display) = match (&from, &since, &until) {
            (Some(r), _, _) => (Some(r.clone()), format!("{}..HEAD", r)),
            (None, Some(s), Some(u)) => {
                let commits = get_commit_logs(None, Some(s), Some(u), None)?;
                let display = format!("--since {} --until {} ({} commits)", s, u, commits.len());
                (commits.last().map(|c| c.hash.clone()), display)
            }
            (None, Some(s), None) => {
                let commits = get_commit_logs(None, Some(s), None, None)?;
                let display = format!("--since {} ({} commits)", s, commits.len());
                (commits.last().map(|c| c.hash.clone()), display)
            }
            (None, None, Some(u)) => {
                let commits = get_commit_logs(None, None, Some(u), None)?;
                let display = format!("--until {} ({} commits)", u, commits.len());
                (commits.last().map(|c| c.hash.clone()), display)
            }
            (None, None, None) => {
                // Default: try latest tag, otherwise working tree
                match get_latest_tag() {
                    Ok(Some(tag)) => (Some(tag.clone()), format!("{}..HEAD", tag)),
                    _ => (None, "working tree".to_string()),
                }
            }
        };

        println!("Processing changes ({})...\n", display);

        let diff_target = build_diff_target(effective_from.as_deref(), to.as_deref(), base_branch);
        let diff_target_ref = if diff_target.is_empty() {
            None
        } else {
            Some(diff_target.as_str())
        };

        let raw_diff = get_diff(diff_target_ref, false, usize::MAX)?;
        let diff = apply_smart_diff_with_context(
            &raw_diff,
            max_diff_chars,
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

    let prompt = EXPLAIN_USER
        .replace("{range}", if staged { "staged" } else { &display })
        .replace("{stats}", &stats)
        .replace("{diff}", &diff);

    let (project_ctx, user_ctx) = load_all_context();
    let system = explain_system_with_context(project_ctx.as_deref(), user_ctx.as_deref());

    let r = client.chat(&system, &prompt, stream).await?;
    if stream {
        println!();
    } else {
        println!("{}", r);
    }
    Ok(())
}
