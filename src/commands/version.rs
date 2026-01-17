// src/commands/version.rs
use anyhow::Result;

use crate::client::LlmClient;
use crate::git::{build_diff_target, get_current_version, get_diff};
use crate::prompt::{VERSION_SYSTEM_PROMPT, VERSION_USER_PROMPT};

use super::apply_smart_diff;

pub async fn cmd_version(
    client: &LlmClient,
    base: Option<String>,
    to: Option<String>,
    base_branch: &str,
    current: Option<String>,
    stream: bool,
    alg: u8,
    max_diff_chars: usize,
) -> Result<()> {
    let current = current.unwrap_or_else(get_current_version);
    println!("Version analysis (current: {})...\n", current);

    let diff_target = build_diff_target(base.as_deref(), to.as_deref(), base_branch);
    let diff_target_ref = if diff_target.is_empty() {
        None
    } else {
        Some(diff_target.as_str())
    };

    let raw_diff = get_diff(diff_target_ref, false, usize::MAX)?;

    if raw_diff.trim().is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    let diff = apply_smart_diff(&raw_diff, max_diff_chars, false, alg)?;

    let prompt = VERSION_USER_PROMPT
        .replace("{version}", &current)
        .replace("{diff}", &diff);

    let r = client.chat(VERSION_SYSTEM_PROMPT, &prompt, stream).await?;
    if stream {
        println!();
    } else {
        println!("{}", r);
    }
    Ok(())
}