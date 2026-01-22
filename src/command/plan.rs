// src/command/plan.rs
use anyhow::Result;

use crate::git;

/// Print (and optionally execute) a simple execution plan for the current repo state.
///
/// v0 scaffold:
/// - conflicts => suggest `gitar resolve`
/// - otherwise summarize staged/unstaged/untracked and suggest `split` or `commit`
pub fn cmd_plan(apply: bool, suggest: bool) -> Result<()> {
    if apply {
        eprintln!("Plan execution is not implemented yet. Run without --apply.");
        return Ok(());
    }

    if git::has_conflicts()? {
        println!("Plan:");
        println!("  - Conflicts detected: run `gitar resolve`");
        return Ok(());
    }

    let staged = !git::run_git(&["diff", "--cached", "--name-only"])?
        .trim()
        .is_empty();
    let unstaged = !git::run_git(&["diff", "--name-only"])?.trim().is_empty();
    let untracked = !git::run_git(&["ls-files", "--others", "--exclude-standard"])?
        .trim()
        .is_empty();

    println!("Plan:");
    println!(
        "  - Changes: staged={}, unstaged={}, untracked={}",
        staged, unstaged, untracked
    );

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
        assert!(cmd_plan(true, true).is_ok());
    }
}
