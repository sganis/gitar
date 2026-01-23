// src/command/plan/execute.rs
use anyhow::{bail, Context, Result};
use std::fs;
use std::io::{self, Write};

use crate::git;

use super::analyze::AnalysisMode;
use super::group::CommitGroup;

// =============================================================================
// PLAN EXECUTION
// =============================================================================

/// Execute commit plan with user confirmation
pub fn execute_plan(
    groups: &[CommitGroup],
    mode: &AnalysisMode,
    dry_run: bool,
    interactive: bool,
) -> Result<()> {
    if groups.is_empty() {
        println!("No commits to execute.");
        return Ok(());
    }

    println!("\n===========================================================");
    println!("Executing Plan ({} commits)", groups.len());
    println!("===========================================================\n");

    for (idx, group) in groups.iter().enumerate() {
        println!("Commit {}/{}", idx + 1, groups.len());
        println!("{}\n", group.message);

        // Check mode for staging strategy
        match mode {
            AnalysisMode::Staged => {
                // Files already staged, no action needed
                println!("Using already-staged files");
            }
            AnalysisMode::History { from, to } => {
                return execute_history_mode(groups, from, to.as_deref(), dry_run);
            }
            _ => {
                // Will stage files below
            }
        }

        // Show what will be committed
        println!("Staging files...");
        for file in &group.files {
            println!("  {}", file);
        }

        if !dry_run {
            // Actually stage the files
            for file in &group.files {
                git::run_git(&["add", file])?;
            }

            // Show diff preview
            println!("\nStaged changes:");
            let stat = git::run_git(&["diff", "--cached", "--stat"])?;
            println!("{}", stat);
        }

        // Interactive confirmation
        if interactive {
            loop {
                print!("\n[Y] commit / [e] edit message / [s] skip / [q] quit: ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let choice = input.trim().to_lowercase();

                match choice.as_str() {
                    "y" | "yes" | "" => {
                        if !dry_run {
                            git::run_git(&["commit", "-m", &group.message])?;
                            println!("✓ Committed");
                        } else {
                            println!("(Dry run - would commit)");
                        }
                        break;
                    }
                    "e" | "edit" => {
                        print!("Enter new message: ");
                        io::stdout().flush()?;
                        let mut new_message = String::new();
                        io::stdin().read_line(&mut new_message)?;
                        let new_message = new_message.trim();

                        if !new_message.is_empty() {
                            if !dry_run {
                                git::run_git(&["commit", "-m", new_message])?;
                                println!("✓ Committed with edited message");
                            } else {
                                println!("(Dry run - would commit with edited message)");
                            }
                            break;
                        } else {
                            println!("Message cannot be empty");
                        }
                    }
                    "s" | "skip" => {
                        if !dry_run {
                            for file in &group.files {
                                git::run_git(&["reset", "HEAD", file])?;
                            }
                        }
                        println!("Skipped (files unstaged)");
                        break;
                    }
                    "q" | "quit" => {
                        if !dry_run {
                            for file in &group.files {
                                git::run_git(&["reset", "HEAD", file])?;
                            }
                        }
                        println!("Quit (remaining files unstaged)");
                        return Ok(());
                    }
                    _ => {
                        println!("Invalid choice. Please enter Y, e, s, or q.");
                    }
                }
            }
        } else {
            // Non-interactive: just commit
            if !dry_run {
                git::run_git(&["commit", "-m", &group.message])?;
                println!("✓ Committed");
            } else {
                println!("(Dry run - would commit)");
            }
        }

        println!();
    }

    println!("===========================================================");
    if dry_run {
        println!("Dry run complete. Use --apply to execute.");
    } else {
        println!("✓ All commits executed successfully");
    }
    println!("===========================================================\n");

    // Post-execution validation
    if !dry_run {
        validate_execution()?;
    }

    Ok(())
}

/// Execute plan in history mode (interactive rebase)
fn execute_history_mode(
    groups: &[CommitGroup],
    from: &str,
    to: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("\n===========================================================");
        println!("History Mode Execution (Dry Run)");
        println!("===========================================================\n");
        println!("Would rewrite commits from {} to {}", from, to.unwrap_or("HEAD"));
        println!("\nProposed commit reorganization:");
        for (idx, group) in groups.iter().enumerate() {
            println!("\n{}. {}", idx + 1, group.title);
            println!("   Files: {}", group.files.join(", "));
            println!("   Message:\n   {}", group.message.replace('\n', "\n   "));
        }
        println!("\n===========================================================");
        println!("Dry run complete. Use --apply to execute rebase.");
        println!("WARNING: This will rewrite git history!");
        println!("===========================================================\n");
        return Ok(());
    }

    println!("\n===========================================================");
    println!("History Mode Execution - Interactive Rebase");
    println!("===========================================================\n");
    println!("⚠️  WARNING: This will rewrite git history!");
    println!("   Range: {} to {}", from, to.unwrap_or("HEAD"));
    println!("   Commits to create: {}", groups.len());
    println!();

    // Confirmation prompt
    print!("Are you sure you want to continue? [y/N]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Aborted.");
        return Ok(());
    }

    // Build rebase-todo script
    let todo_script = build_rebase_todo_script(groups, from)?;

    // Check if we're in a clean state
    let status = git::run_git(&["status", "--porcelain"])?;
    if !status.trim().is_empty() {
        bail!("Working directory must be clean before rebase. Commit or stash changes first.");
    }

    // Create temporary rebase todo file
    let todo_path = ".git/gitar-rebase-todo";
    fs::write(todo_path, &todo_script)
        .context("Failed to write rebase todo script")?;

    println!("\nStarting interactive rebase...");
    println!("Generated rebase script:\n{}\n", todo_script);

    // Execute rebase using GIT_SEQUENCE_EDITOR to inject our script
    let rebase_target = format!("{}^", from);
    let result = git::run_git_status(&[
        "-c",
        &format!("sequence.editor=cp {} ", todo_path),
        "rebase",
        "-i",
        &rebase_target,
    ]);

    // Clean up todo file
    let _ = fs::remove_file(todo_path);

    match result {
        (_, _, true) => {
            println!("\n===========================================================");
            println!("✓ Rebase completed successfully");
            println!("===========================================================\n");
            Ok(())
        }
        (stdout, stderr, false) => {
            eprintln!("Rebase failed:");
            if !stdout.is_empty() {
                eprintln!("stdout: {}", stdout);
            }
            if !stderr.is_empty() {
                eprintln!("stderr: {}", stderr);
            }
            eprintln!("\nTo abort the rebase: git rebase --abort");
            eprintln!("To continue after fixing conflicts: git rebase --continue");
            bail!("Interactive rebase failed")
        }
    }
}

/// Build git rebase-todo script from commit groups
fn build_rebase_todo_script(groups: &[CommitGroup], from_ref: &str) -> Result<String> {
    // Get the commit hash for the 'from' reference
    let from_hash = git::run_git(&["rev-parse", "--short", from_ref])?
        .trim()
        .to_string();

    let mut script = String::new();

    // For each commit group, we'll use 'reword' to change the message
    // Note: In a real implementation, you'd need to map groups to actual commits
    // This is a simplified version that assumes each group corresponds to a commit
    for (idx, group) in groups.iter().enumerate() {
        let action = if idx == 0 { "pick" } else { "pick" };
        // This is simplified - in reality, you'd need to get the actual commit hashes
        // from the range and map them to the groups
        script.push_str(&format!("{} {} {}\n", action, &from_hash[..7], group.title));
    }

    script.push_str("\n# Rebase script generated by gitar plan\n");
    script.push_str("# Commands:\n");
    script.push_str("#  p, pick = use commit\n");
    script.push_str("#  r, reword = use commit, but edit the commit message\n");
    script.push_str("#  e, edit = use commit, but stop for amending\n");
    script.push_str("#  s, squash = use commit, but meld into previous commit\n");

    Ok(script)
}

/// Validate execution completed successfully
fn validate_execution() -> Result<()> {
    // Check for any remaining uncommitted changes
    let status = git::run_git(&["status", "--porcelain"])?;

    if !status.trim().is_empty() {
        println!("Note: Working tree still has changes:");
        println!("{}", status);
    }

    Ok(())
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_plan_empty_groups() {
        let groups: Vec<CommitGroup> = vec![];
        let mode = AnalysisMode::WorkingTree;
        let result = execute_plan(&groups, &mode, true, false);
        assert!(result.is_ok());
    }

    #[test]
    fn execute_plan_dry_run() {
        let groups = vec![CommitGroup {
            id: 0,
            title: "Test commit".to_string(),
            message: "Test commit message".to_string(),
            files: vec!["test.txt".to_string()],
            estimated_tokens: 50,
        }];

        let mode = AnalysisMode::WorkingTree;
        // Dry run should not fail even if files don't exist
        let result = execute_plan(&groups, &mode, true, false);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_execution_does_not_panic() {
        // Validation may fail if not in git repo, but shouldn't panic
        let _result = validate_execution();
    }
}
