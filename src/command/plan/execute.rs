// src/command/plan/execute.rs
use anyhow::Result;
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
            AnalysisMode::History { .. } => {
                println!("Warning: History mode execution not fully implemented");
                continue;
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
