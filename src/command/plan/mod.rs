// src/command/plan/mod.rs

pub mod analyze;
pub mod context;
pub mod editor;
pub mod execute;
pub mod group;
pub mod model;
pub mod prompt;
pub mod scoring;

use crate::client::LlmClient;
use crate::command::cmd_fix;
use crate::config::ResolvedConfig;
use crate::git;
use anyhow::{bail, Result};

// Re-export public API
pub use analyze::AnalysisMode;

/// Main entry point for plan command
pub async fn cmd_plan(
    client: &LlmClient,
    config: &ResolvedConfig,
    mode: Option<AnalysisMode>,
    apply: bool,
    fix: bool,
    suggest: bool,
    interactive: bool,
    algo: u8,
) -> Result<()> {
    // Handle legacy suggest mode (simple state inspection)
    if suggest {
        return cmd_plan_suggest(mode);
    }

    // Check for conflicts and fix if requested
    if git::has_conflicts()? {
        if fix {
            println!("Conflicts detected. Resolving...\n");
            cmd_fix(
                client,
                apply, // Use same apply flag as plan
                true,  // Auto-confirm (yes=true)
                config.stream,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?;
            println!("\nConflicts resolved. Continuing with analysis...\n");
        } else {
            println!("Conflicts detected. Run `gitar fix` or use `gitar plan --fix`.");
            return Ok(());
        }
    }

    // Step 1: Analyze repository state
    let analysis = analyze::detect_and_analyze(mode)?;

    // Check if there are any changes
    if analysis.files.is_empty() {
        match analysis.mode {
            AnalysisMode::Auto => {
                println!("Working tree clean. Nothing to analyze.");
                return Ok(());
            }
            _ => {
                println!("No changes found for the specified mode.");
                return Ok(());
            }
        }
    }

    println!("Analyzing {} file(s)...", analysis.files.len());

    // Step 2: Generate commit groups with LLM
    let groups = group::create_groups(
        &analysis,
        client,
        config.preset,
        algo,
        config.max_diff_chars,
        config.secret_action,
    )
    .await?;

    if groups.is_empty() {
        bail!("Failed to generate commit strategy. No groups created.");
    }

    println!("\nGenerated strategy with {} commit(s)", groups.len());

    // Step 3: Interactive editing (if interactive)
    // execute_interactive tracks whether to prompt for each commit during execution
    let (approved_groups, execute_interactive) = if interactive {
        let mut editor = editor::PlanEditor::new(groups);
        match editor
            .run_interactive(
                client,
                &analysis,
                config.preset,
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        {
            editor::EditResult::ApprovedAll(g) => (g, false),
            editor::EditResult::ApprovedOneByOne(g) => (g, true),
            editor::EditResult::Cancelled => {
                println!("Execution cancelled.");
                return Ok(());
            }
        }
    } else {
        // Non-interactive: display plan and ask for confirmation
        println!("\nCommit Strategy:");
        println!("===========================================================");
        for (idx, group) in groups.iter().enumerate() {
            println!("\nCommit {}/{}", idx + 1, groups.len());
            println!("{}", group.message);
            println!("Files ({}):", group.files_with_status.len());
            for file in &group.files_with_status {
                println!("  {} {}", file.status.label(), file.path);
            }
        }
        println!("\n===========================================================\n");
        (groups, false)
    };

    // Step 4: Execute strategy (if apply flag is set)
    if apply {
        execute::execute_plan(
            &approved_groups,
            &analysis.mode,
            false,
            execute_interactive,
            client.model(),
        )?;
    } else {
        println!("Dry run complete. Use --apply to execute commits.");
    }

    Ok(())
}

/// Legacy suggest mode - simple state inspection
fn cmd_plan_suggest(mode: Option<AnalysisMode>) -> Result<()> {
    let analysis = analyze::detect_and_analyze(mode)?;

    if git::has_conflicts()? {
        println!("Recommendation:");
        println!("  - Conflicts detected: run `gitar fix`");
        return Ok(());
    }

    let (staged, unstaged, untracked) = match &analysis.mode {
        AnalysisMode::Auto => (false, false, false),
        AnalysisMode::Staged => (true, false, false),
        AnalysisMode::WorkingTree => {
            let staged_diff = git::run_git(&["diff", "--cached", "--name-only"])?;
            let has_staged = !staged_diff.trim().is_empty();
            (has_staged, true, true)
        }
        AnalysisMode::History { .. } => (false, false, false),
    };

    println!("Recommendation:");
    println!(
        "  - Changes: staged={}, unstaged={}, untracked={}",
        staged, unstaged, untracked
    );
    println!("  - Files detected: {}", analysis.files.len());

    if staged && (unstaged || untracked) {
        println!("  - Suggestion: run `gitar plan --apply`");
    } else if unstaged || untracked {
        println!("  - Suggestion: run `gitar plan --apply`");
    } else if staged {
        println!("  - Suggestion: run `gitar commit`");
    } else {
        println!("  - Suggestion: working tree clean");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_suggest_mode() {
        // Should not panic (may fail if not in git repo, which is ok)
        let _result = cmd_plan_suggest(None);
        // Test passes if it doesn't panic
    }

    #[test]
    fn analyze_mode_variants() {
        // Test that mode variants can be created
        let _auto = AnalysisMode::Auto;
        let _working = AnalysisMode::WorkingTree;
        let _staged = AnalysisMode::Staged;
        let _history = AnalysisMode::History {
            from: "v1.0.0".to_string(),
            to: None,
        };
    }
}
