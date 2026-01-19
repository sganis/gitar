// src/git.rs
use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;

// =============================================================================
// EXCLUDE PATTERNS
// =============================================================================
pub const EXCLUDE_PATTERNS: &[&str] = &[
    ":(exclude)*.lock",
    ":(exclude)package-lock.json",
    ":(exclude)yarn.lock",
    ":(exclude)pnpm-lock.yaml",
    ":(exclude)dist/*",
    ":(exclude)build/*",
    ":(exclude)*.min.js",
    ":(exclude)*.min.css",
    ":(exclude)*.map",
    ":(exclude).env*",
    ":(exclude)target/*",
];

// =============================================================================
// COMMIT INFO
// =============================================================================
#[derive(Debug)]
pub struct CommitInfo {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

// =============================================================================
// GIT UTILITIES
// =============================================================================
pub fn run_git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let error_msg = if !stderr.is_empty() {
            stderr.to_string()
        } else if !stdout.is_empty() {
            stdout.to_string()
        } else {
            format!("exit code {}", output.status.code().unwrap_or(-1))
        };
        bail!("git {} failed: {}", args.join(" "), error_msg.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run git command and return (stdout, stderr, success) tuple.
/// Unlike run_git(), this does not fail on non-zero exit.
pub fn run_git_status(args: &[&str]) -> (String, String, bool) {
    match Command::new("git").args(args).output() {
        Ok(o) => (
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
            o.status.success(),
        ),
        Err(e) => (String::new(), e.to_string(), false),
    }
}

/// Run git command, returning Ok(stdout) on success or Ok(None) on failure.
/// Useful for commands where failure is expected (e.g., checking if ref exists).
pub fn run_git_optional(args: &[&str]) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute git: {}", e))?;

    if output.status.success() {
        Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
    } else {
        Ok(None)
    }
}

pub fn is_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Returns the repo root (top-level) directory as a String.
pub fn get_repo_root() -> Result<String> {
    let out = run_git(&["rev-parse", "--show-toplevel"])?;
    let root = out.trim().to_string();
    if root.is_empty() {
        bail!("git rev-parse --show-toplevel returned empty output");
    }
    Ok(root)
}

/// Same as get_repo_root(), but as a PathBuf.
pub fn get_repo_root_path() -> Result<PathBuf> {
    Ok(PathBuf::from(get_repo_root()?))
}

pub fn get_git_dir() -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Some(PathBuf::from(path_str))
}

pub fn get_current_branch() -> String {
    if let Ok(out) = run_git(&["branch", "--show-current"]) {
        let b = out.trim().to_string();
        if !b.is_empty() {
            return b;
        }
    }
    if let Ok(out) = run_git(&["rev-parse", "--abbrev-ref", "HEAD"]) {
        let b = out.trim().to_string();
        if !b.is_empty() {
            return b;
        }
    }
    "HEAD".to_string()
}

pub fn get_default_branch() -> String {
    for b in ["main", "master"] {
        if let Ok(Some(_)) = run_git_optional(&["rev-parse", "--verify", b]) {
            return b.into();
        }
    }
    "main".into()
}

pub fn get_commit_logs(
    limit: Option<usize>,
    since: Option<&str>,
    until: Option<&str>,
    range: Option<&str>,
) -> Result<Vec<CommitInfo>> {
    let mut args_vec: Vec<String> = vec![
        "log".into(),
        "--pretty=format:%H|%an|%ad|%s".into(),
        "--date=iso".into(),
    ];

    if let Some(n) = limit {
        args_vec.push(format!("-n{}", n));
    }
    if let Some(s) = since {
        args_vec.push(format!("--since={}", s));
    }
    if let Some(u) = until {
        args_vec.push(format!("--until={}", u));
    }
    if let Some(r) = range {
        args_vec.push(r.to_string());
    }

    let args: Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();
    let output = run_git(&args)?;

    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let p: Vec<&str> = l.splitn(4, '|').collect();
            if p.len() >= 4 {
                Some(CommitInfo {
                    hash: p[0].into(),
                    author: p[1].into(),
                    date: p[2].into(),
                    message: p[3].into(),
                })
            } else {
                None
            }
        })
        .collect())
}

pub fn get_commit_diff(hash: &str, max_chars: usize) -> Result<Option<String>> {
    let parent_ref = format!("{}^", hash);
    let has_parent = run_git_optional(&["rev-parse", &parent_ref])?.is_some();

    let diff = if has_parent {
        let diff_ref = format!("{}^!", hash);
        let mut args = vec!["diff", &diff_ref, "--unified=3", "--", "."];
        args.extend(EXCLUDE_PATTERNS);
        run_git(&args)?
    } else {
        let mut args = vec!["diff-tree", "--patch", "--unified=3", "--root", hash, "--", "."];
        args.extend(EXCLUDE_PATTERNS);
        run_git(&args)?
    };

    if diff.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(truncate_diff(diff, max_chars)))
}

pub fn get_diff(target: Option<&str>, staged: bool, max_chars: usize) -> Result<String> {
    let mut args = vec!["diff", "--unified=3"];
    if staged {
        args.push("--cached");
    } else if let Some(t) = target {
        args.push(t);
    }
    args.extend(&["--", "."]);
    args.extend(EXCLUDE_PATTERNS);
    Ok(truncate_diff(run_git(&args)?, max_chars))
}

pub fn get_diff_stats(target: Option<&str>, staged: bool) -> Result<String> {
    let mut args = vec!["diff", "--stat"];
    if staged {
        args.push("--cached");
    } else if let Some(t) = target {
        args.push(t);
    }
    run_git(&args)
}

pub fn get_current_version() -> String {
    run_git(&["describe", "--tags", "--abbrev=0"])
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "0.0.0".into())
}

pub fn truncate_diff(diff: String, max: usize) -> String {
    if diff.len() <= max {
        return diff;
    }
    let mut t = diff[..max].to_string();
    if let Some(p) = t.rfind("\ndiff --git") {
        if p > max / 2 {
            t.truncate(p);
        }
    }
    t.push_str("\n\n[... truncated ...]");
    t
}

pub fn build_range(from: Option<&str>, to: Option<&str>, base_branch: &str) -> Option<String> {
    let end = to.unwrap_or("HEAD");
    from.map(|r| format!("{}..{}", r, end)).or_else(|| {
        let branch = get_current_branch();
        if branch != base_branch {
            Some(format!(
                "{}..{}",
                base_branch,
                if to.is_some() { end } else { &branch }
            ))
        } else {
            None
        }
    })
}

pub fn build_diff_target(from: Option<&str>, to: Option<&str>, base_branch: &str) -> String {
    let end = to.unwrap_or("HEAD");
    match from {
        Some(r) => format!("{}..{}", r, end),
        None => {
            let branch = get_current_branch();
            if branch != base_branch {
                format!(
                    "{}...{}",
                    base_branch,
                    if to.is_some() { end } else { &branch }
                )
            } else {
                let tag = get_current_version();
                if tag != "0.0.0" {
                    format!("{}..{}", tag, end)
                } else {
                    String::new()
                }
            }
        }
    }
}

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Git command execution tests
    // ==========================================================================

    #[test]
    fn run_git_succeeds_on_valid_command() {
        let result = run_git(&["--version"]);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("git version"));
    }

    #[test]
    fn run_git_fails_on_invalid_command() {
        let result = run_git(&["invalid-command-xyz-123"]);
        assert!(result.is_err());
    }

    #[test]
    fn run_git_optional_returns_none_on_failure() {
        let result = run_git_optional(&["rev-parse", "nonexistent-ref-xyz-123"]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn run_git_optional_returns_some_on_success() {
        let result = run_git_optional(&["--version"]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn run_git_status_returns_tuple() {
        let (stdout, stderr, success) = run_git_status(&["--version"]);
        assert!(success);
        assert!(stdout.contains("git version"));
        assert!(stderr.is_empty() || !stderr.contains("fatal"));
    }

    // ==========================================================================
    // Truncation tests
    // ==========================================================================

    #[test]
    fn truncate_diff_short_unchanged() {
        let diff = "short diff content".to_string();
        let result = truncate_diff(diff.clone(), 1000);
        assert_eq!(result, diff);
    }

    #[test]
    fn truncate_diff_long_truncated() {
        let diff = "a".repeat(500);
        let result = truncate_diff(diff, 100);
        assert!(result.len() < 500);
        assert!(result.contains("[... truncated ...]"));
    }

    #[test]
    fn truncate_diff_preserves_file_boundaries() {
        let diff = format!(
            "diff --git a/file1.rs\n{}\ndiff --git a/file2.rs\n{}",
            "a".repeat(100),
            "b".repeat(100)
        );
        let result = truncate_diff(diff, 150);
        assert!(result.contains("diff --git a/file1.rs"));
        assert!(result.contains("[... truncated ...]"));
    }

    // ==========================================================================
    // Range building tests
    // ==========================================================================

    #[test]
    fn build_range_with_ref() {
        assert_eq!(build_range(Some("v1.0.0"), None, "main"), Some("v1.0.0..HEAD".to_string()));
    }

    #[test]
    fn build_range_with_ref_and_to() {
        assert_eq!(build_range(Some("v1.0.0"), Some("v1.0.1"), "main"), Some("v1.0.0..v1.0.1".to_string()));
    }

    #[test]
    fn build_diff_target_with_ref() {
        assert_eq!(build_diff_target(Some("v1.0.0"), None, "main"), "v1.0.0..HEAD");
    }

    #[test]
    fn build_diff_target_with_ref_and_to() {
        assert_eq!(build_diff_target(Some("v1.0.0"), Some("v1.0.1"), "main"), "v1.0.0..v1.0.1");
    }

    // ==========================================================================
    // Commit log parsing tests
    // ==========================================================================

    #[test]
    fn parse_commit_log_line() {
        let line = "abc123def|John Doe|2024-01-15 10:30:00|Fix bug in parser";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "abc123def");
        assert_eq!(parts[1], "John Doe");
        assert_eq!(parts[3], "Fix bug in parser");
    }

    #[test]
    fn parse_commit_log_with_pipe_in_message() {
        let line = "abc123|Author|2024-01-15|Message with | pipe | chars";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[3], "Message with | pipe | chars");
    }

    // ==========================================================================
    // Exclude patterns tests
    // ==========================================================================

    #[test]
    fn exclude_patterns_format() {
        for pattern in EXCLUDE_PATTERNS {
            assert!(pattern.starts_with(":(exclude)"), "Pattern should start with :(exclude): {}", pattern);
        }
    }

    #[test]
    fn exclude_patterns_contains_expected() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains("*.lock")));
        assert!(patterns.iter().any(|p| p.contains("package-lock.json")));
        assert!(patterns.iter().any(|p| p.contains("target/*")));
        assert!(patterns.iter().any(|p| p.contains(".env")));
    }

    // ==========================================================================
    // Repo detection tests (conditional on being in a repo)
    // ==========================================================================

    #[test]
    fn get_default_branch_returns_valid() {
        let branch = get_default_branch();
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn get_current_branch_returns_string() {
        let branch = get_current_branch();
        assert!(!branch.is_empty());
    }
}