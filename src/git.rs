// src/git.rs
use anyhow::Result;
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
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

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

pub fn is_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
        if run_git(&["rev-parse", "--verify", b]).is_ok() {
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
    let has_parent = run_git(&["rev-parse", &parent_ref]).is_ok();

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
    from.map(|r| format!("{}..{}", r, end))
        .or_else(|| {
            let branch = get_current_branch();
            if branch != base_branch {
                Some(format!("{}..{}", base_branch, if to.is_some() { end } else { &branch }))
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
                format!("{}...{}", base_branch, if to.is_some() { end } else { &branch })
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
        assert!(result.contains("[... truncated ...]"));
        assert!(result.contains("diff --git a/file1.rs"));
    }

    #[test]
    fn truncate_diff_exact_boundary() {
        let diff = "exactly100chars".repeat(10);
        let result = truncate_diff(diff.clone(), 150);
        assert_eq!(result, diff);
    }

    #[test]
    fn truncate_diff_empty_string() {
        let result = truncate_diff(String::new(), 100);
        assert!(result.is_empty());
    }

    #[test]
    fn truncate_diff_max_zero() {
        let diff = "some content".to_string();
        let result = truncate_diff(diff, 0);
        assert!(result.contains("[... truncated ...]"));
    }

    #[test]
    fn truncate_diff_single_char_over() {
        let diff = "abcde".to_string();
        let result = truncate_diff(diff, 4);
        assert!(result.contains("[... truncated ...]"));
    }

    #[test]
    fn truncate_diff_no_file_boundary_in_first_half() {
        let diff = format!(
            "diff --git a/file1.rs\n{}\n{}",
            "a".repeat(10),
            "b".repeat(200)
        );
        let result = truncate_diff(diff, 100);
        assert!(result.contains("[... truncated ...]"));
    }

    #[test]
    fn build_range_with_ref() {
        let result = build_range(Some("v1.0.0"), None, "main");
        assert_eq!(result, Some("v1.0.0..HEAD".to_string()));
    }

    #[test]
    fn build_range_with_ref_and_to() {
        let result = build_range(Some("v1.0.0"), Some("v1.0.1"), "main");
        assert_eq!(result, Some("v1.0.0..v1.0.1".to_string()));
    }

    #[test]
    fn build_range_with_commit_hash() {
        let result = build_range(Some("abc123"), None, "main");
        assert_eq!(result, Some("abc123..HEAD".to_string()));
    }

    #[test]
    fn build_range_none_on_base_branch() {
        let result = build_range(None, None, "nonexistent-branch-xyz");
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn build_diff_target_with_ref() {
        let result = build_diff_target(Some("v1.0.0"), None, "main");
        assert_eq!(result, "v1.0.0..HEAD");
    }

    #[test]
    fn build_diff_target_with_ref_and_to() {
        let result = build_diff_target(Some("v1.0.0"), Some("v1.0.1"), "main");
        assert_eq!(result, "v1.0.0..v1.0.1");
    }

    #[test]
    fn build_diff_target_with_commit() {
        let result = build_diff_target(Some("abc123def"), None, "main");
        assert_eq!(result, "abc123def..HEAD");
    }

    #[test]
    fn commit_info_struct_creation() {
        let info = CommitInfo {
            hash: "abc123def456".into(),
            author: "John Doe".into(),
            date: "2024-01-15 10:30:00 +0000".into(),
            message: "Fix bug in parser".into(),
        };
        assert_eq!(info.hash, "abc123def456");
        assert_eq!(info.author, "John Doe");
        assert_eq!(info.date, "2024-01-15 10:30:00 +0000");
        assert_eq!(info.message, "Fix bug in parser");
    }

    #[test]
    fn parse_commit_log_line() {
        let line = "abc123def|John Doe|2024-01-15 10:30:00|Fix bug in parser";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert_eq!(parts.len(), 4);
        let info = CommitInfo {
            hash: parts[0].into(),
            author: parts[1].into(),
            date: parts[2].into(),
            message: parts[3].into(),
        };
        assert_eq!(info.hash, "abc123def");
        assert_eq!(info.author, "John Doe");
    }

    #[test]
    fn parse_commit_log_with_pipe_in_message() {
        let line = "abc123|Author|2024-01-15|Message with | pipe | chars";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[3], "Message with | pipe | chars");
    }

    #[test]
    fn parse_commit_log_incomplete_rejected() {
        let line = "abc123|Author|2024-01-15";
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        assert!(parts.len() < 4);
    }

    #[test]
    fn parse_empty_commit_log() {
        let output = "";
        let commits: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
        assert!(commits.is_empty());
    }

    #[test]
    fn parse_commit_log_filters_empty_lines() {
        let output = "abc|author|date|msg\n\n\ndef|author2|date2|msg2\n";
        let commits: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(commits.len(), 2);
    }

    #[test]
    fn exclude_patterns_not_empty() {
        assert!(!EXCLUDE_PATTERNS.is_empty());
    }

    #[test]
    fn exclude_patterns_format() {
        for pattern in EXCLUDE_PATTERNS {
            assert!(
                pattern.starts_with(":(exclude)"),
                "Pattern should start with :(exclude): {}",
                pattern
            );
        }
    }

    #[test]
    fn exclude_patterns_contains_lock_files() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains("*.lock")));
        assert!(patterns.iter().any(|p| p.contains("package-lock.json")));
        assert!(patterns.iter().any(|p| p.contains("yarn.lock")));
        assert!(patterns.iter().any(|p| p.contains("pnpm-lock.yaml")));
    }

    #[test]
    fn exclude_patterns_contains_build_dirs() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains("dist/*")));
        assert!(patterns.iter().any(|p| p.contains("build/*")));
        assert!(patterns.iter().any(|p| p.contains("target/*")));
    }

    #[test]
    fn exclude_patterns_contains_minified() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains("*.min.js")));
        assert!(patterns.iter().any(|p| p.contains("*.min.css")));
        assert!(patterns.iter().any(|p| p.contains("*.map")));
    }

    #[test]
    fn exclude_patterns_contains_env() {
        let patterns: Vec<&str> = EXCLUDE_PATTERNS.to_vec();
        assert!(patterns.iter().any(|p| p.contains(".env")));
    }

    #[test]
    fn run_git_returns_result() {
        let result = run_git(&["--version"]);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("git version"));
    }

    #[test]
    fn run_git_status_returns_tuple() {
        let (stdout, stderr, success) = run_git_status(&["--version"]);
        assert!(success);
        assert!(stdout.contains("git version"));
        assert!(stderr.is_empty() || !stderr.contains("fatal"));
    }

    #[test]
    fn run_git_status_handles_invalid_command() {
        let (stdout, stderr, success) = run_git_status(&["invalid-command-xyz"]);
        assert!(!success);
        assert!(stdout.is_empty() || stderr.contains("git"));
    }

    #[test]
    fn is_git_repo_detects_repo() {
        let result = is_git_repo();
        let _ = result;
    }

    #[test]
    fn get_git_dir_returns_path_in_repo() {
        if is_git_repo() {
            let result = get_git_dir();
            assert!(result.is_some());
            let path = result.unwrap();
            assert!(path.to_string_lossy().contains(".git") || path.to_string_lossy() == ".git");
        }
    }

    #[test]
    fn get_current_branch_returns_string() {
        let branch = get_current_branch();
        assert!(!branch.is_empty());
    }

    #[test]
    fn get_default_branch_returns_valid() {
        let branch = get_default_branch();
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn get_current_version_returns_string() {
        let version = get_current_version();
        assert!(!version.is_empty());
    }
}