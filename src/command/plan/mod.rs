// src/command/plan/mod.rs

pub mod analyze;

use anyhow::Result;
use crate::git;

// Re-export public API from analyze (will be used by future modules)
#[allow(unused_imports)]
pub use analyze::{
    AnalysisMode, AnalysisResult, FileChange, ChangeStatus, FileCategory,
    detect_and_analyze, categorize_file,
};

/// Print (and optionally execute) a simple execution plan for the current repo state.
///
/// Current implementation (Phase 1 stub):
/// - Uses new analyze module to detect repo state
/// - Suggests appropriate next actions
/// - Full orchestration will be implemented in later phases
pub fn cmd_plan(apply: bool, suggest: bool) -> Result<()> {
    if apply {
        eprintln!("Plan execution is not implemented yet. Run without --apply.");
        return Ok(());
    }

    // Use new analyze module
    let analysis = analyze::detect_and_analyze(None)?;

    // Handle conflicts
    if git::has_conflicts()? {
        println!("Plan:");
        println!("  - Conflicts detected: run `gitar resolve`");
        return Ok(());
    }

    // Determine state from analysis mode
    let (staged, unstaged, untracked) = match &analysis.mode {
        AnalysisMode::Auto => {
            // Clean working tree
            (false, false, false)
        }
        AnalysisMode::Staged => {
            (true, false, false)
        }
        AnalysisMode::WorkingTree => {
            // Check if there are actually staged files too
            let staged_diff = git::run_git(&["diff", "--cached", "--name-only"])?;
            let has_staged = !staged_diff.trim().is_empty();
            (has_staged, true, true)
        }
        AnalysisMode::History { .. } => {
            // History mode not yet supported in suggestions
            (false, false, false)
        }
    };

    println!("Plan:");
    println!(
        "  - Changes: staged={}, unstaged={}, untracked={}",
        staged, unstaged, untracked
    );
    println!("  - Files detected: {}", analysis.files.len());

    if suggest {
        if staged && (unstaged || untracked) {
            println!("  - Suggestion: run `gitar split`");
        } else if unstaged || untracked {
            println!("  - Suggestion: run `gitar split`");
        } else if staged {
            println!("  - Suggestion: run `gitar commit`");
        } else {
            println!("  - Suggestion: working tree clean");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_apply_is_safe_noop() {
        // apply=true should always succeed (just prints warning)
        let result = cmd_plan(true, true);
        assert!(result.is_ok());
    }

    #[test]
    fn plan_suggest_flag() {
        // Should not panic (may fail if not in git repo, which is ok)
        let _result = cmd_plan(false, true);
        // Test passes if it doesn't panic
    }

    #[test]
    fn plan_no_flags() {
        // Should not panic (may fail if not in git repo, which is ok)
        let _result = cmd_plan(false, false);
        // Test passes if it doesn't panic
    }
}
