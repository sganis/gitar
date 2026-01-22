// src/command/plan.rs
use anyhow::Result;

use crate::git::{get_repo_state, RepoState};

pub fn cmd_plan(apply: bool, suggest: bool) -> Result<()> {
    // For now: suggest-only preview. Later: orchestrate resolve/clean/complete/amend/split.
    let state = get_repo_state()?;

    if state.is_clean() {
        println!("Working tree clean. Nothing to do.");
        return Ok(());
    }

    print_repo_summary(&state);

    if state.merge_in_progress || state.rebase_in_progress || state.cherry_pick_in_progress {
        println!("Next: resolve conflicts (gitar resolve) before planning commits.");
    } else {
        println!("Next: build an execution plan (resolve -> clean -> complete -> amend -> split).");
    }

    if apply {
        // Not executing anything yet in this first iteration.
        println!("Note: plan --apply is not implemented yet. This is a preview.");
    } else if !suggest {
        // clap defaults suggest=true; keep behavior stable if user flips flags.
        println!("Note: plan runs in suggest mode by default.");
    }

    Ok(())
}

fn print_repo_summary(s: &RepoState) {
    let mut ops = Vec::new();
    if s.merge_in_progress {
        ops.push("merge");
    }
    if s.rebase_in_progress {
        ops.push("rebase");
    }
    if s.cherry_pick_in_progress {
        ops.push("cherry-pick");
    }

    if !ops.is_empty() {
        println!("Operation in progress: {}", ops.join(", "));
    }

    if !s.conflicted_files.is_empty() {
        println!("Conflicts: {}", s.conflicted_files.len());
        for p in s.conflicted_files.iter().take(10) {
            println!("  - {}", p);
        }
        if s.conflicted_files.len() > 10 {
            println!("  ...");
        }
    }

    println!(
        "Changed files: {} staged, {} unstaged, {} untracked",
        s.staged_files.len(),
        s.unstaged_files.len(),
        s.untracked_files.len()
    );
}
