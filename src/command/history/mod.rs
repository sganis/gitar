// src/command/history/mod.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::context::load_all_context;
use crate::git::{get_commit_diff, get_commit_logs};
use crate::prompt::preset::Preset;
use crate::prompt::secret::SecretAction;
use crate::prompt::template::{history_system_with_context, HISTORY_USER};

use super::{apply_smart_diff, SHORT_HASH_LEN};

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

        let diff = apply_smart_diff(&raw_diff, max_diff_chars, true, alg, secret_action)?;

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
