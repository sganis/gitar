// src/command/changelog/mod.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::context::load_all_context;
use crate::context::secret::SecretAction;
use crate::context::template::{changelog_system_with_context, CHANGELOG_USER};
use crate::git::{get_commit_logs, get_diff};

use super::{apply_smart_diff_with_context, get_latest_tag, AnalysisContext, SHORT_HASH_LEN};

pub async fn cmd_changelog(
    client: &LlmClient,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    limit: Option<usize>,
    stream: bool,
    alg: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    // Determine the effective 'from' reference: use provided, or latest tag, or default to 50 commits
    let (effective_from, limit) = match (&from, limit) {
        (Some(r), _) => (Some(r.clone()), None), // Explicit from: no limit
        (None, Some(n)) => (None, Some(n)),      // Explicit limit
        (None, None) => {
            // Default: try latest tag, otherwise 50 commits
            match get_latest_tag() {
                Ok(Some(tag)) => (Some(tag), None),
                _ => (None, Some(50)),
            }
        }
    };

    let end = to.as_deref().unwrap_or("HEAD");
    let range = effective_from.as_ref().map(|r| format!("{}..{}", r, end));

    let display = match (&effective_from, &to, &since, &until) {
        (Some(r), Some(t), _, _) => format!("{}..{}", r, t),
        (Some(r), None, _, _) => format!("{}..HEAD", r),
        (None, Some(t), _, _) => format!("..{}", t),
        (None, None, Some(s), Some(u)) => format!("--since {} --until {}", s, u),
        (None, None, Some(s), None) => format!("--since {}", s),
        (None, None, None, Some(u)) => format!("--until {}", u),
        (None, None, None, None) => format!("last {} commits", limit.unwrap_or(50)),
    };

    let commits = get_commit_logs(limit, since.as_deref(), until.as_deref(), range.as_deref())?;

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    println!("Processing {} commits ({})...\n", commits.len(), display);

    let context = AnalysisContext::new()
        .with_provider(client.provider())
        .with_model(client.model());

    // Build commit list with messages
    let ct = commits
        .iter()
        .map(|c| {
            format!(
                "- [{}] {}",
                &c.hash[..SHORT_HASH_LEN.min(c.hash.len())],
                c.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Get combined diff for the range
    let diff = if let Some(ref base) = effective_from {
        let raw_diff = get_diff(Some(&format!("{}..{}", base, end)), false, usize::MAX)?;
        if raw_diff.trim().is_empty() {
            String::new()
        } else {
            apply_smart_diff_with_context(
                &raw_diff,
                max_diff_chars,
                false,
                alg,
                Some(&context),
                secret_action,
            )?
        }
    } else if let Some(first_commit) = commits.last() {
        // Use oldest commit's parent as base
        let raw_diff = get_diff(
            Some(&format!("{}^..{}", first_commit.hash, end)),
            false,
            usize::MAX,
        )
        .unwrap_or_default();
        if raw_diff.trim().is_empty() {
            String::new()
        } else {
            apply_smart_diff_with_context(
                &raw_diff,
                max_diff_chars,
                false,
                alg,
                Some(&context),
                secret_action,
            )?
        }
    } else {
        String::new()
    };

    let prompt = CHANGELOG_USER
        .replace("{range}", &display)
        .replace("{count}", &commits.len().to_string())
        .replace("{commits}", &ct)
        .replace("{diff}", &diff);

    let (project_ctx, user_ctx) = load_all_context();
    let system = changelog_system_with_context(project_ctx.as_deref(), user_ctx.as_deref());

    let r = client.chat(&system, &prompt, stream).await?;
    if stream {
        println!();
    } else {
        println!("{}", r);
    }
    Ok(())
}
