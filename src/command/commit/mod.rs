// src/command/commit/mod.rs
use anyhow::{bail, Result};
use std::fs;

use crate::client::LlmClient;
use crate::git::{get_diff, run_git, run_git_status};
use crate::context::preset::Preset;
use crate::context::secret::SecretAction;
use crate::context::template::{commit_system_with_context, COMMIT_USER};
use crate::context::repo::load_all_context;
use crate::prompt;

use super::{apply_smart_diff_with_context, AnalysisContext};

pub async fn cmd_commit(
    client: &LlmClient,
    preset: Preset,
    push: bool,
    all: bool,
    amend: bool,
    tag: bool,
    write_to: Option<String>,
    silent: bool,
    stream: bool,
    alg: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    // If amending, get the diff from HEAD~1..HEAD
    let raw_diff = if amend {
        if !silent {
            println!("Generating new message for last commit...");
        }
        // Get the diff of the last commit
        run_git(&["diff", "HEAD~1..HEAD"])?
    } else {
        // Normal commit flow: get staged and unstaged changes
        let staged = run_git(&["diff", "--cached"]).unwrap_or_default();
        let unstaged = run_git(&["diff"]).unwrap_or_default();

        let mut diff = String::new();
        if !staged.trim().is_empty() {
            diff.push_str(&staged);
        }
        if !unstaged.trim().is_empty() {
            if !diff.is_empty() {
                diff.push('\n');
            }
            diff.push_str(&unstaged);
        }
        diff
    };

    if raw_diff.trim().is_empty() {
        if !silent {
            if amend {
                println!("Last commit has no changes.");
            } else {
                println!("Nothing to commit.");
            }
        }
        return Ok(());
    }

    let context = AnalysisContext::new()
        .with_provider(client.provider())
        .with_model(client.model());

    let diff = apply_smart_diff_with_context(
        &raw_diff,
        max_diff_chars,
        silent,
        alg,
        Some(&context),
        secret_action,
    )?;

    let (project_ctx, user_ctx) = load_all_context();
    let system = commit_system_with_context(
        preset,
        project_ctx.as_deref(),
        user_ctx.as_deref(),
    );

    // Hook mode: never stream (hooks expect file output only)
    if let Some(ref output_file) = write_to {
        let prompt = COMMIT_USER.replace("{diff}", &diff);
        let msg = client.chat(&system, &prompt, false).await?;
        fs::write(output_file, format!("{}\n", msg.trim()))?;
        return Ok(());
    }

    // Interactive mode
    let commit_message = loop {
        let prompt = COMMIT_USER.replace("{diff}", &diff);

        let do_stream = stream && !silent;
        let msg = client.chat(&system, &prompt, do_stream).await?;

        if silent {
            break msg;
        }

        if do_stream {
            println!();
        } else {
            println!("\n{}\n", msg);
        }

        println!("{}", "=".repeat(50));

        let options = ["Accept", "Regenerate", "Edit message", "Cancel"];
        match prompt::select("Action", &options, 0)? {
            0 => break msg, // Accept
            1 => {
                // Regenerate
                println!("Regenerating...\n");
                continue;
            }
            2 => {
                // Edit message
                let edited = prompt::input("New message", Some(&msg))?;
                break if edited.trim().is_empty() { msg } else { edited };
            }
            _ => {
                // Cancel
                println!("Canceled.");
                return Ok(());
            }
        }
    };

    // Stage all changes if requested (not applicable when amending)
    if all && !amend {
        if !silent {
            println!("Staging all...");
        }
        run_git(&["add", "-A"])?;
    }

    if !silent {
        if amend {
            println!("Amending commit...");
        } else {
            println!("Committing...");
        }
    }

    let full_msg = if tag {
        format!("{} [AI:{}]", commit_message, client.model())
    } else {
        commit_message
    };

    // Use --amend when amending, otherwise regular commit
    let (out, err, ok) = if amend {
        run_git_status(&["commit", "--amend", "-m", &full_msg])
    } else {
        run_git_status(&["commit", "-m", &full_msg])
    };

    if !silent {
        println!("{}{}", out, err);
    }

    if !ok {
        bail!("Commit failed. Check git status and try again.");
    }

    if push {
        if !silent {
            println!("Pushing...");
        }
        let (o, e, push_ok) = run_git_status(&["push"]);
        if !silent {
            println!("{}{}", o, e);
        }
        if !push_ok {
            bail!("Push failed.");
        }
    }

    Ok(())
}

pub async fn cmd_staged(
    client: &LlmClient,
    preset: Preset,
    stream: bool,
    alg: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    let raw_diff = get_diff(None, true, usize::MAX)?;
    if raw_diff.trim().is_empty() {
        bail!("No staged changes.");
    }

    let context = AnalysisContext::new()
        .with_provider(client.provider())
        .with_model(client.model());

    let diff = apply_smart_diff_with_context(
        &raw_diff,
        max_diff_chars,
        false,
        alg,
        Some(&context),
        secret_action,
    )?;

    let (project_ctx, user_ctx) = load_all_context();
    let system = commit_system_with_context(
        preset,
        project_ctx.as_deref(),
        user_ctx.as_deref(),
    );

    let prompt = COMMIT_USER.replace("{diff}", &diff);
    let msg = client.chat(&system, &prompt, stream).await?;

    if stream {
        println!();
    } else {
        println!("{}", msg);
    }
    Ok(())
}

pub async fn cmd_unstaged(
    client: &LlmClient,
    preset: Preset,
    stream: bool,
    alg: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    let raw_diff = get_diff(None, false, usize::MAX)?;
    if raw_diff.trim().is_empty() {
        bail!("No unstaged changes.");
    }

    let context = AnalysisContext::new()
        .with_provider(client.provider())
        .with_model(client.model());

    let diff = apply_smart_diff_with_context(
        &raw_diff,
        max_diff_chars,
        false,
        alg,
        Some(&context),
        secret_action,
    )?;

    let (project_ctx, user_ctx) = load_all_context();
    let system = commit_system_with_context(
        preset,
        project_ctx.as_deref(),
        user_ctx.as_deref(),
    );

    let prompt = COMMIT_USER.replace("{diff}", &diff);
    let msg = client.chat(&system, &prompt, stream).await?;
    if stream {
        println!();
    } else {
        println!("{}", msg);
    }
    Ok(())
}
