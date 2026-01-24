// src/command/fix/git_helper.rs
use anyhow::{bail, Context, Result};
use std::process::Command;

pub fn git_add(path: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["add", "--", path])
        .status()
        .with_context(|| format!("git add failed for {}", path))?;

    if !status.success() {
        bail!("git add failed for {}", path);
    }
    Ok(())
}

pub fn git_conflicted_names() -> Result<Vec<String>> {
    let out = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=U"])
        .output()
        .context("git diff --name-only --diff-filter=U failed")?;
    if !out.status.success() {
        bail!("git diff --name-only --diff-filter=U failed");
    }
    let s = String::from_utf8_lossy(&out.stdout);
    Ok(s.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

#[allow(dead_code)]
pub fn git_cached_names() -> Result<Vec<String>> {
    let out = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .output()
        .context("git diff --cached --name-only failed")?;
    if !out.status.success() {
        bail!("git diff --cached --name-only failed");
    }
    let s = String::from_utf8_lossy(&out.stdout);
    Ok(s.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

pub fn git_has_unmerged_entries(path: &str) -> Result<bool> {
    // During conflicts, Git stores multiple index stages (1/2/3).
    // After successful `git add`, the path should have no `-u` entries.
    let out = Command::new("git")
        .args(["ls-files", "-u", "--", path])
        .output()
        .with_context(|| format!("git ls-files -u failed for {}", path))?;

    if !out.status.success() {
        bail!("git ls-files -u failed for {}", path);
    }

    let s = String::from_utf8_lossy(&out.stdout);
    Ok(!s.trim().is_empty())
}
