// src/commands/history.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::git::{get_commit_diff, get_commit_logs};
use crate::prompt::{HISTORY_SYSTEM_PROMPT, HISTORY_USER_PROMPT};

use super::apply_smart_diff;

pub async fn cmd_history(
    client: &LlmClient,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    limit: Option<usize>,
    delay: u64,
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
        (None, None, Some(s), _) => format!("--since {}", s),
        _ => "recent".into(),
    };

    println!("Fetching commits ({})...", display);
    let commits = get_commit_logs(limit, since.as_deref(), until.as_deref(), range.as_deref())?;

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    println!("Processing {} commits...\n", commits.len());

    for (i, c) in commits.iter().enumerate() {
        let h = &c.hash[..8.min(c.hash.len())];
        let d = &c.date[..10.min(c.date.len())];
        let a = if c.author.len() > 15 {
            &c.author[..15]
        } else {
            &c.author
        };
        let m = if c.message.len() > 40 {
            &c.message[..40]
        } else {
            &c.message
        };

        println!(
            "[{}/{}] {} | {} | {:15} | {}",
            i + 1,
            commits.len(),
            h,
            d,
            a,
            m
        );

        let raw_diff = match get_commit_diff(&c.hash, usize::MAX)? {
            Some(d) if !d.trim().is_empty() => d,
            _ => {
                println!("  - No diff");
                continue;
            }
        };

        let diff = apply_smart_diff(&raw_diff, max_diff_chars, true, alg)?;

        let prompt = HISTORY_USER_PROMPT
            .replace("{original_message}", &c.message)
            .replace("{diff}", &diff);

        match client.chat(HISTORY_SYSTEM_PROMPT, &prompt, stream).await {
            Ok(r) => {
                if stream {
                    println!();
                } else {
                    for (j, l) in r.lines().enumerate() {
                        if !l.trim().is_empty() {
                            println!("{}{}", if j == 0 { "  - " } else { "    " }, l);
                        }
                    }
                }
            }
            Err(e) => println!("  x {}", e),
        }

        if i < commits.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
    }

    Ok(())
}