// src/command/fix/git_helper.rs
use anyhow::Result;

use crate::git;

pub fn git_add(path: &str) -> Result<()> {
    git::run_git(&["add", "--", path])?;
    Ok(())
}

pub fn git_conflicted_names() -> Result<Vec<String>> {
    let out = git::run_git(&["diff", "--name-only", "--diff-filter=U"])?;
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

#[allow(dead_code)]
pub fn git_cached_names() -> Result<Vec<String>> {
    let out = git::run_git(&["diff", "--cached", "--name-only"])?;
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

pub fn git_has_unmerged_entries(path: &str) -> Result<bool> {
    // During conflicts, Git stores multiple index stages (1/2/3).
    // After successful `git add`, the path should have no `-u` entries.
    let out = git::run_git(&["ls-files", "-u", "--", path])?;
    Ok(!out.trim().is_empty())
}
