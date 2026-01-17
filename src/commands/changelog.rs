// src/commands/changelog.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::git::{get_commit_logs, get_diff};
use crate::prompt::{CHANGELOG_SYSTEM_PROMPT, CHANGELOG_USER_PROMPT};

use super::apply_smart_diff;

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

    // Build commit list with messages
    let ct = commits
        .iter()
        .map(|c| format!("- [{}] {}", &c.hash[..8.min(c.hash.len())], c.message))
        .collect::<Vec<_>>()
        .join("\n");

    // Get combined diff for the range
    let diff = if let Some(ref base) = from {
        let raw_diff = get_diff(Some(&format!("{}..{}", base, end)), false, usize::MAX)?;
        if raw_diff.trim().is_empty() {
            String::new()
        } else {
            apply_smart_diff(&raw_diff, max_diff_chars, false, alg)?
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
            apply_smart_diff(&raw_diff, max_diff_chars, false, alg)?
        }
    } else {
        String::new()
    };

    let prompt = CHANGELOG_USER_PROMPT
        .replace("{range}", &display)
        .replace("{count}", &commits.len().to_string())
        .replace("{commits}", &ct)
        .replace("{diff}", &diff);

    let r = client.chat(CHANGELOG_SYSTEM_PROMPT, &prompt, stream).await?;
    if stream {
        println!();
    } else {
        println!("{}", r);
    }
    Ok(())
}