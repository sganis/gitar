// src/commands/explain.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::git::{build_diff_target, get_commit_logs, get_diff, get_diff_stats};
use crate::prompt::{EXPLAIN_SYSTEM_PROMPT, EXPLAIN_USER_PROMPT};

use super::apply_smart_diff;

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
        let raw_diff = get_diff(None, true, usize::MAX)?;
        let diff = apply_smart_diff(&raw_diff, max_diff_chars, false, alg)?;
        (diff, get_diff_stats(None, true)?)
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
        let diff_target_ref = if diff_target.is_empty() {
            None
        } else {
            Some(diff_target.as_str())
        };

        let raw_diff = get_diff(diff_target_ref, false, usize::MAX)?;
        let diff = apply_smart_diff(&raw_diff, max_diff_chars, false, alg)?;
        (diff, get_diff_stats(diff_target_ref, false)?)
    };

    if diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let prompt = EXPLAIN_USER_PROMPT
        .replace("{range}", if staged { "staged" } else { &display })
        .replace("{stats}", &stats)
        .replace("{diff}", &diff);

    let r = client.chat(EXPLAIN_SYSTEM_PROMPT, &prompt, stream).await?;
    if stream {
        println!();
    } else {
        println!("{}", r);
    }
    Ok(())
}