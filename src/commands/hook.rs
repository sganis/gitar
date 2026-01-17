// src/commands/hook.rs
use anyhow::{bail, Context, Result};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::cli::{HookCommands, HOOK_SCRIPT};
use crate::git::get_git_dir;

pub fn cmd_hook(command: HookCommands) -> Result<()> {
    let git_dir =
        get_git_dir().context("Could not locate .git directory. Are you in a git repo?")?;
    let hook_path = git_dir.join("hooks").join("prepare-commit-msg");

    match command {
        HookCommands::Install => {
            if hook_path.exists() {
                let existing = fs::read_to_string(&hook_path).unwrap_or_default();
                if existing.contains("gitar-hook") {
                    println!("Gitar hook is already installed.");
                    return Ok(());
                }
                bail!(
                    "A prepare-commit-msg hook already exists at {:?}. Please back it up or delete it first.",
                    hook_path
                );
            }

            fs::write(&hook_path, HOOK_SCRIPT)?;

            #[cfg(unix)]
            {
                let mut perms = fs::metadata(&hook_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&hook_path, perms)?;
            }

            println!("Universal hook installed at {:?}", hook_path);
        }
        HookCommands::Uninstall => {
            if !hook_path.exists() {
                println!("No hook found to uninstall.");
                return Ok(());
            }

            let content = fs::read_to_string(&hook_path)?;
            if content.contains("gitar-hook") {
                fs::remove_file(&hook_path)?;
                println!("Hook uninstalled successfully.");
            } else {
                println!("The existing hook was not created by gitar. Manual removal required.");
            }
        }
    }
    Ok(())
}


