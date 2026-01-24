// src/command/history/mod.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::context::load_all_context;
use crate::context::preset::Preset;
use crate::context::secret::SecretAction;
use crate::context::template::{history_system_with_context, HISTORY_USER};
use crate::git::{get_commit_diff, get_commit_logs};

use super::{apply_smart_diff_with_context, get_latest_tag, AnalysisContext, SHORT_HASH_LEN};

const MAX_AUTHOR_LEN: usize = 15;
const MAX_MESSAGE_PREVIEW_LEN: usize = 40;
const DATE_LEN: usize = 10;

pub async fn cmd_history(
    client: &LlmClient,
    preset: Preset,
    from: Option<String>,
    to: Option<String>,
    since: Option<String>,
    until: Option<String>,
    limit: Option<usize>,
    delay: u64,
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
        (None, None, Some(s), _) => format!("--since {}", s),
        _ => format!("last {} commits", limit.unwrap_or(50)),
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

    let (project_ctx, user_ctx) = load_all_context();
    let system = history_system_with_context(preset, project_ctx.as_deref(), user_ctx.as_deref());

    for (i, c) in commits.iter().enumerate() {
        let h = &c.hash[..SHORT_HASH_LEN.min(c.hash.len())];
        let d = &c.date[..DATE_LEN.min(c.date.len())];
        let a = if c.author.len() > MAX_AUTHOR_LEN {
            &c.author[..MAX_AUTHOR_LEN]
        } else {
            &c.author
        };
        let m = if c.message.len() > MAX_MESSAGE_PREVIEW_LEN {
            &c.message[..MAX_MESSAGE_PREVIEW_LEN]
        } else {
            &c.message
        };

        println!(
            "[{}/{}] {} | {} | {:width$} | {}",
            i + 1,
            commits.len(),
            h,
            d,
            a,
            m,
            width = MAX_AUTHOR_LEN
        );

        let raw_diff = match get_commit_diff(&c.hash, usize::MAX)? {
            Some(d) if !d.trim().is_empty() => d,
            _ => {
                println!("  - No diff");
                continue;
            }
        };

        let diff = apply_smart_diff_with_context(
            &raw_diff,
            max_diff_chars,
            true,
            alg,
            Some(&context),
            secret_action,
        )?;

        let prompt = HISTORY_USER
            .replace("{original_message}", &c.message)
            .replace("{diff}", &diff);

        match client.chat(&system, &prompt, stream).await {
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
