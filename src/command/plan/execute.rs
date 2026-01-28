// src/command/plan/execute.rs
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::color::{success, warning};
use crate::git;
use crate::prompt;

use super::analyze::AnalysisMode;
use super::group::{CommitGroup, FileStatus};

// =============================================================================
// LARGE FILE HANDLING
// =============================================================================

/// Handle large file group - unstage if staged, otherwise ignore
fn handle_large_file_group(group: &CommitGroup, mode: &AnalysisMode, dry_run: bool) -> Result<()> {
    match mode {
        AnalysisMode::Staged => {
            // Unstage large files that were staged
            println!("\nUnstaging large files...");
            if !dry_run {
                for file in &group.files {
                    // Extract actual path (remove size suffix if present)
                    let path = file.split(" (").next().unwrap_or(file);
                    let result = git::run_git_status(&["reset", "HEAD", "--", path]);
                    if result.2 {
                        println!("  Unstaged: {}", path);
                    } else {
                        warning(format!("  Failed to unstage: {}", path));
                    }
                }
            } else {
                println!("  (Dry run - would unstage {} file(s))", group.files.len());
            }
        }
        _ => {
            // For working tree mode, just skip these files
            println!("\nIgnoring large files (not staging).");
            if !dry_run {
                println!("  {} file(s) will not be committed.", group.files.len());
            } else {
                println!("  (Dry run - would skip {} file(s))", group.files.len());
            }
        }
    }

    Ok(())
}

// =============================================================================
// STAGING HELPER
// =============================================================================

/// Stage a file based on its status
fn stage_file(path: &str, status: &FileStatus) -> Result<()> {
    match status {
        FileStatus::Deleted => {
            // For deleted files, use git rm --cached (file already deleted from disk)
            // Or use git add -u which handles deletions
            let result = git::run_git_status(&["add", "-u", "--", path]);
            if !result.2 {
                // Fallback: try git rm --cached if git add -u fails
                git::run_git(&["rm", "--cached", "--", path])?;
            }
        }
        FileStatus::Renamed => {
            // For renamed files, stage both the deletion and addition
            // git add -A handles this correctly
            git::run_git(&["add", "-A", "--", path])?;
        }
        _ => {
            // For added and modified files, use standard git add
            git::run_git(&["add", "--", path])?;
        }
    }
    Ok(())
}

/// Execute a commit group (stage files and commit)
fn execute_commit_group(group: &CommitGroup, mode: &AnalysisMode, model: &str) -> Result<()> {
    // Stage files if not already staged
    if !matches!(mode, AnalysisMode::Staged) {
        for file in &group.files_with_status {
            stage_file(&file.path, &file.status)?;
        }
    }

    // Commit
    let msg = format!("{} [AI:{}]", group.message, model);
    git::run_git(&["commit", "-m", &msg])?;
    success("Committed");
    Ok(())
}

// =============================================================================
// PLAN EXECUTION
// =============================================================================

/// Execute commit plan with user confirmation
pub fn execute_plan(
    groups: &[CommitGroup],
    mode: &AnalysisMode,
    dry_run: bool,
    interactive: bool,
    include_large_files: bool,
    model: &str,
) -> Result<()> {
    if groups.is_empty() {
        println!("No commits to execute.");
        return Ok(());
    }

    // Count groups
    let total_count = groups.len();
    let large_file_count = groups.iter().filter(|g| g.is_large_file_group).count();

    println!("\n===========================================================");
    if large_file_count > 0 {
        if include_large_files {
            println!("Executing Plan ({} groups, binary/large files INCLUDED)", total_count);
        } else {
            println!("Executing Plan ({} groups, binary/large files SKIPPED)", total_count);
        }
    } else {
        println!("Executing Plan ({} groups)", total_count);
    }
    println!("===========================================================\n");

    for (idx, group) in groups.iter().enumerate() {
        // Handle large file groups
        if group.is_large_file_group {
            println!("Group {}/{} [BINARY/LARGE FILES]", idx + 1, total_count);
            println!("Files ({}):", group.files.len());
            for file in &group.files_with_status {
                println!("  {} {}", file.status.label(), file.path);
            }
            println!();

            if include_large_files {
                // User chose to include - commit these files
                println!("Committing binary/large files...");
                if !dry_run {
                    execute_commit_group(group, mode, model)?;
                } else {
                    println!("  (Dry run - would commit {} file(s))", group.files.len());
                }
            } else {
                // Default: skip/unstage these files
                handle_large_file_group(group, mode, dry_run)?;
            }
            println!();
            continue;
        }

        println!("Group {}/{}", idx + 1, total_count);
        println!("{}\n", group.message);

        // Handle history mode separately
        if let AnalysisMode::History { from, to } = mode {
            return execute_history_mode(groups, from, to.as_deref(), dry_run);
        }

        // For staged mode: unstage everything first, then stage only this group's files
        // This ensures each commit only includes its designated files
        if matches!(mode, AnalysisMode::Staged) && !dry_run {
            // Unstage all files first
            let _ = git::run_git_status(&["reset", "HEAD"]);
        }

        // Stage only this group's files
        println!("Staging files...");
        for file in &group.files_with_status {
            println!("  {} {}", file.status.label(), file.path);
        }

        if !dry_run {
            // Stage files using files_with_status which has proper paths
            for file in &group.files_with_status {
                stage_file(&file.path, &file.status)?;
            }

            // Show diff preview
            println!("\nStaged changes:");
            let stat = git::run_git(&["diff", "--cached", "--stat"])?;
            println!("{}", stat);
        }

        // Interactive confirmation
        if interactive {
            let options = ["Commit", "Edit message", "Skip", "Quit"];
            match prompt::select("Action", &options, 0)? {
                0 => {
                    // Commit
                    if !dry_run {
                        let msg = format!("{} [AI:{}]", group.message, model);
                        git::run_git(&["commit", "-m", &msg])?;
                        success("Committed");
                    } else {
                        println!("(Dry run - would commit)");
                    }
                }
                1 => {
                    // Edit message
                    let new_message = prompt::input("Enter new message", Some(&group.message))?;
                    if !new_message.is_empty() {
                        if !dry_run {
                            let msg = format!("{} [AI:{}]", new_message, model);
                            git::run_git(&["commit", "-m", &msg])?;
                            success("Committed with edited message");
                        } else {
                            println!("(Dry run - would commit with edited message)");
                        }
                    } else {
                        println!("Message cannot be empty - skipping");
                        if !dry_run {
                            for file in &group.files_with_status {
                                git::run_git(&["reset", "HEAD", &file.path])?;
                            }
                        }
                    }
                }
                2 => {
                    // Skip
                    if !dry_run {
                        for file in &group.files_with_status {
                            git::run_git(&["reset", "HEAD", &file.path])?;
                        }
                    }
                    println!("Skipped (files unstaged)");
                }
                _ => {
                    // Quit
                    if !dry_run {
                        for file in &group.files_with_status {
                            git::run_git(&["reset", "HEAD", &file.path])?;
                        }
                    }
                    println!("Quit (remaining files unstaged)");
                    return Ok(());
                }
            }
        } else {
            // Non-interactive: just commit
            if !dry_run {
                let msg = format!("{} [AI:{}]", group.message, model);
                git::run_git(&["commit", "-m", &msg])?;
                success("Committed");
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
        success("Plan executed successfully");
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
        println!(
            "Would rewrite commits from {} to {}",
            from,
            to.unwrap_or("HEAD")
        );
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
    warning("WARNING: This will rewrite git history!");
    println!("   Range: {} to {}", from, to.unwrap_or("HEAD"));
    println!("   Commits to create: {}", groups.len());
    println!();

    // Confirmation prompt
    if !prompt::confirm("Are you sure you want to continue?", false)? {
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

    // Create temporary rebase todo file (use repo root for path)
    let repo_root = PathBuf::from(git::get_repo_root()?);
    let todo_path = repo_root.join(".git/gitar-rebase-todo");
    fs::write(&todo_path, &todo_script).context("Failed to write rebase todo script")?;

    println!("\nStarting interactive rebase...");
    println!("Generated rebase script:\n{}\n", todo_script);

    // Execute rebase using GIT_SEQUENCE_EDITOR to inject our script
    let rebase_target = format!("{}^", from);
    let todo_path_str = todo_path.to_string_lossy();
    let result = git::run_git_status(&[
        "-c",
        &format!("sequence.editor=cp {} ", todo_path_str),
        "rebase",
        "-i",
        &rebase_target,
    ]);

    // Clean up todo file
    let _ = fs::remove_file(&todo_path);

    match result {
        (_, _, true) => {
            println!("\n===========================================================");
            success("Rebase completed successfully");
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

    script.push_str("\n# Rebase script generated by gitar run\n");
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
        // Parse and display user-friendly status
        let mut untracked = Vec::new();
        let mut modified = Vec::new();
        let mut staged = Vec::new();

        for line in status.lines() {
            if line.len() < 3 {
                continue;
            }
            let code = &line[..2];
            let file = line[3..].trim();

            match code {
                "??" => untracked.push(file),
                " M" | " D" => modified.push(file),
                "M " | "A " | "D " | "R " => staged.push(file),
                _ => modified.push(file), // Catch-all for other statuses
            }
        }

        println!("\nNote: Working tree still has changes:");
        if !untracked.is_empty() {
            println!("  Untracked files (not in git):");
            for f in &untracked {
                println!("    {}", f);
            }
        }
        if !modified.is_empty() {
            println!("  Modified files (unstaged):");
            for f in &modified {
                println!("    {}", f);
            }
        }
        if !staged.is_empty() {
            println!("  Staged files (not committed):");
            for f in &staged {
                println!("    {}", f);
            }
        }
    }

    Ok(())
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::plan::group::{FileStatus, FileWithStatus};

    #[test]
    fn execute_plan_empty_groups() {
        let groups: Vec<CommitGroup> = vec![];
        let mode = AnalysisMode::WorkingTree;
        let result = execute_plan(&groups, &mode, true, false, false, "test-model");
        assert!(result.is_ok());
    }

    #[test]
    fn execute_plan_dry_run() {
        let groups = vec![CommitGroup {
            id: 0,
            title: "Test commit".to_string(),
            message: "Test commit message".to_string(),
            files: vec!["test.txt".to_string()],
            files_with_status: vec![FileWithStatus {
                path: "test.txt".to_string(),
                status: FileStatus::Added,
            }],
            estimated_tokens: 50,
            is_large_file_group: false,
        }];

        let mode = AnalysisMode::WorkingTree;
        // Dry run should not fail even if files don't exist
        let result = execute_plan(&groups, &mode, true, false, false, "gpt-4");
        assert!(result.is_ok());
    }

    #[test]
    fn execute_plan_with_large_files() {
        let groups = vec![
            CommitGroup {
                id: 0,
                title: "Regular commit".to_string(),
                message: "Regular commit message".to_string(),
                files: vec!["src/main.rs".to_string()],
                files_with_status: vec![FileWithStatus {
                    path: "src/main.rs".to_string(),
                    status: FileStatus::Modified,
                }],
                estimated_tokens: 50,
                is_large_file_group: false,
            },
            CommitGroup {
                id: 1,
                title: "Large files (1 file, >= 50MB) [default: SKIP]".to_string(),
                message: "Large files detected".to_string(),
                files: vec!["assets/video.mp4".to_string()],
                files_with_status: vec![FileWithStatus {
                    path: "assets/video.mp4".to_string(),
                    status: FileStatus::Added,
                }],
                estimated_tokens: 0,
                is_large_file_group: true,
            },
        ];

        let mode = AnalysisMode::WorkingTree;
        // Dry run should handle large file groups correctly
        let result = execute_plan(&groups, &mode, true, false, false, "gpt-4");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_execution_does_not_panic() {
        // Validation may fail if not in git repo, but shouldn't panic
        let _result = validate_execution();
    }
}
