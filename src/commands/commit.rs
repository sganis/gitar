// src/commands/commit.rs
use anyhow::{bail, Result};
use std::fs;
use std::io::{self, Write};

use crate::client::LlmClient;
use crate::git::{get_diff, run_git, run_git_status};
use crate::prompt::{COMMIT_SYSTEM_PROMPT, COMMIT_USER_PROMPT};

use super::apply_smart_diff;

pub async fn cmd_commit(
    client: &LlmClient,
    push: bool,
    all: bool,
    tag: bool,
    write_to: Option<String>,
    silent: bool,
    stream: bool,
    alg: u8,
    max_diff_chars: usize,
) -> Result<()> {
    let staged = run_git(&["diff", "--cached"]).unwrap_or_default();
    let unstaged = run_git(&["diff"]).unwrap_or_default();

    let mut raw_diff = String::new();
    if !staged.trim().is_empty() {
        raw_diff.push_str(&staged);
    }
    if !unstaged.trim().is_empty() {
        if !raw_diff.is_empty() {
            raw_diff.push('\n');
        }
        raw_diff.push_str(&unstaged);
    }

    if raw_diff.trim().is_empty() {
        if !silent {
            println!("Nothing to commit.");
        }
        return Ok(());
    }

    let diff = apply_smart_diff(&raw_diff, max_diff_chars, silent, alg)?;

    // Hook mode: never stream (hooks expect file output only)
    if let Some(ref output_file) = write_to {
        let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
        let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt, false).await?;
        fs::write(output_file, format!("{}\n", msg.trim()))?;
        return Ok(());
    }

    // Interactive mode
    let commit_message = loop {
        let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);

        let do_stream = stream && !silent;
        let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt, do_stream).await?;

        if silent {
            break msg;
        }

        if do_stream {
            println!();
        } else {
            println!("\n{}\n", msg);
        }

        println!("{}", "=".repeat(50));
        println!("  [Enter] Accept | [g] Regenerate | [e] Edit | [other] Cancel");
        println!("{}", "=".repeat(50));
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "" => break msg,
            "g" => {
                println!("Regenerating...\n");
                continue;
            }
            "e" => {
                print!("New message: ");
                io::stdout().flush()?;
                let mut ed = String::new();
                io::stdin().read_line(&mut ed)?;
                break if ed.trim().is_empty() { msg } else { ed.trim().into() };
            }
            _ => {
                println!("Canceled.");
                return Ok(());
            }
        }
    };

    if all {
        if !silent {
            println!("Staging all...");
        }
        run_git(&["add", "-A"])?;
    }

    if !silent {
        println!("Committing...");
    }

    let full_msg = if tag {
        format!("{} [AI:{}]", commit_message, client.model())
    } else {
        commit_message
    };

    let (out, err, ok) = if all {
        run_git_status(&["commit", "-am", &full_msg])
    } else {
        run_git_status(&["commit", "-m", &full_msg])
    };
    if !silent {
        println!("{}{}", out, err);
    }

    if !ok {
        if !silent {
            println!("Commit failed.");
        }
        return Ok(());
    }

    if push {
        if !silent {
            println!("Pushing...");
        }
        let (o, e, _) = run_git_status(&["push"]);
        if !silent {
            println!("{}{}", o, e);
        }
    }

    Ok(())
}

pub async fn cmd_staged(client: &LlmClient, stream: bool, alg: u8, max_diff_chars: usize) -> Result<()> {
    let raw_diff = get_diff(None, true, usize::MAX)?;
    if raw_diff.trim().is_empty() {
        bail!("No staged changes.");
    }

    let diff = apply_smart_diff(&raw_diff, max_diff_chars, false, alg)?;

    let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
    let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt, stream).await?;
    if stream {
        println!();
    } else {
        println!("{}", msg);
    }
    Ok(())
}

pub async fn cmd_unstaged(client: &LlmClient, stream: bool, alg: u8, max_diff_chars: usize) -> Result<()> {
    let raw_diff = get_diff(None, false, usize::MAX)?;
    if raw_diff.trim().is_empty() {
        bail!("No unstaged changes.");
    }

    let diff = apply_smart_diff(&raw_diff, max_diff_chars, false, alg)?;
    let prompt = COMMIT_USER_PROMPT.replace("{diff}", &diff);
    let msg = client.chat(COMMIT_SYSTEM_PROMPT, &prompt, stream).await?;
    if stream {
        println!();
    } else {
        println!("{}", msg);
    }
    Ok(())
}