// src/command/commit.rs
use anyhow::{bail, Result};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::client::LlmClient;
use crate::git::{get_diff, run_git, run_git_status};
use crate::prompt::preset::Preset;
use crate::prompt::secret::SecretAction;
use crate::prompt::template::{commit_system_with_context, COMMIT_USER};

use super::{apply_smart_diff_with_context, AnalysisContext};

fn read_text_file_if_exists(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}

fn home_dir() -> Option<PathBuf> {
    // Cross-platform enough without adding deps:
    // - Unix/macOS: HOME
    // - Windows: USERPROFILE, or HOMEDRIVE+HOMEPATH
    if let Ok(h) = std::env::var("HOME") {
        if !h.trim().is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    if let Ok(h) = std::env::var("USERPROFILE") {
        if !h.trim().is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    let drive = std::env::var("HOMEDRIVE").ok();
    let path = std::env::var("HOMEPATH").ok();
    match (drive, path) {
        (Some(d), Some(p)) if !d.trim().is_empty() && !p.trim().is_empty() => Some(PathBuf::from(format!("{}{}", d, p))),
        _ => None,
    }
}

fn load_user_context() -> Option<String> {
    let hd = home_dir()?;
    let p = hd.join(".gitar").join("gitar.md");
    read_text_file_if_exists(&p)
}

fn load_project_context() -> Option<String> {
    // Always from current working dir: .gitar/gitar.md
    // (If you later want "search upward to repo root", do that in git.rs using rev-parse.)
    let p = PathBuf::from(".gitar").join("gitar.md");
    read_text_file_if_exists(&p)
}

pub async fn cmd_commit(
    client: &LlmClient,
    preset: Preset,
    push: bool,
    all: bool,
    tag: bool,
    write_to: Option<String>,
    silent: bool,
    stream: bool,
    alg: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
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

    let project_ctx = load_project_context();
    let user_ctx = load_user_context();
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

    // Stage all changes if requested
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

    // Use -m (not -am) since we already staged with add -A if needed
    let (out, err, ok) = run_git_status(&["commit", "-m", &full_msg]);
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

    let project_ctx = load_project_context();
    let user_ctx = load_user_context();
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

    let project_ctx = load_project_context();
    let user_ctx = load_user_context();
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
