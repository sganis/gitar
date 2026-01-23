// src/command/release/tag.rs - Git tag creation helpers

use anyhow::{Context, Result};
use crate::git;

// =============================================================================
// TAG OPERATIONS
// =============================================================================

/// Get the latest tag in the repository
pub fn get_latest_tag() -> Result<Option<String>> {
    let output = git::run_git_optional(&["describe", "--tags", "--abbrev=0"])?;
    Ok(output.map(|s| s.trim().to_string()))
}

/// Get all tags sorted by creation date
pub fn get_all_tags() -> Result<Vec<String>> {
    let output = git::run_git_optional(&["tag", "--sort=-creatordate"])?;

    match output {
        Some(tags) => Ok(tags
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()),
        None => Ok(Vec::new()),
    }
}

/// Check if a tag exists
pub fn tag_exists(tag: &str) -> Result<bool> {
    let output = git::run_git_optional(&["tag", "-l", tag])?;
    Ok(output.map_or(false, |s| !s.trim().is_empty()))
}

/// Create an annotated tag
pub fn create_tag(tag: &str, message: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would create tag: {}", tag);
        println!("Tag message:\n{}", message);
        return Ok(());
    }

    git::run_git(&["tag", "-a", tag, "-m", message])
        .context("Failed to create git tag")?;

    println!("✓ Created tag: {}", tag);
    Ok(())
}

/// Get the commit hash for a reference
pub fn get_commit_hash(reference: &str) -> Result<String> {
    git::run_git(&["rev-parse", reference])
        .map(|s| s.trim().to_string())
        .context(format!("Failed to resolve reference: {}", reference))
}

/// Get commits since a reference
pub fn get_commits_since(from: &str, to: Option<&str>) -> Result<Vec<CommitInfo>> {
    let to_ref = to.unwrap_or("HEAD");
    let range = format!("{}..{}", from, to_ref);

    let output = git::run_git(&[
        "log",
        &range,
        "--pretty=format:%H|%h|%s|%an|%ae|%ad",
        "--date=short",
    ])?;

    let mut commits = Vec::new();
    for line in output.lines() {
        if let Some(commit) = parse_commit_line(line) {
            commits.push(commit);
        }
    }

    Ok(commits)
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub subject: String,
    pub author_name: String,
    pub author_email: String,
    pub date: String,
}

fn parse_commit_line(line: &str) -> Option<CommitInfo> {
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() < 6 {
        return None;
    }

    Some(CommitInfo {
        hash: parts[0].to_string(),
        short_hash: parts[1].to_string(),
        subject: parts[2].to_string(),
        author_name: parts[3].to_string(),
        author_email: parts[4].to_string(),
        date: parts[5].to_string(),
    })
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_commit_info() {
        let line = "abc123def456|abc123d|Initial commit|John Doe|john@example.com|2024-01-15";
        let commit = parse_commit_line(line).unwrap();

        assert_eq!(commit.hash, "abc123def456");
        assert_eq!(commit.short_hash, "abc123d");
        assert_eq!(commit.subject, "Initial commit");
        assert_eq!(commit.author_name, "John Doe");
        assert_eq!(commit.date, "2024-01-15");
    }
}
