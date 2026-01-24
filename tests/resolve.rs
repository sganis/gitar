// tests/resolve.rs
//! Integration tests for the resolve command
//!
//! Tests conflict detection, marker parsing, and resolution.

use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

use gitar::command::{cmd_fix_with_resolver, ConflictInput, ConflictResolver};

/// Fake resolver that keeps both sides (ours then theirs) for testing
struct FakeResolver;

impl ConflictResolver for FakeResolver {
    fn resolve(&self, input: &ConflictInput) -> Result<String> {
        let mut out = String::new();
        let mut lines = input.working.split_inclusive('\n').peekable();

        while let Some(line) = lines.next() {
            if line.starts_with("<<<<<<<") {
                // ours
                while let Some(l) = lines.next() {
                    if l.starts_with("=======") {
                        break;
                    }
                    out.push_str(l);
                }
                // theirs
                while let Some(l) = lines.next() {
                    if l.starts_with(">>>>>>>") {
                        break;
                    }
                    out.push_str(l);
                }
                continue;
            }
            out.push_str(line);
        }

        Ok(out)
    }
}

fn run(repo: &std::path::Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .with_context(|| format!("git {:?}", args))?;
    if !out.status.success() {
        anyhow::bail!(
            "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn run_allow_fail(repo: &std::path::Path, args: &[&str]) -> Result<(bool, String, String)> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .with_context(|| format!("git {:?}", args))?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    ))
}

fn write(repo: &std::path::Path, rel: &str, content: &str) -> Result<()> {
    let p = repo.join(rel);
    fs::write(&p, content).with_context(|| format!("write {}", rel))?;
    Ok(())
}

fn current_branch(repo: &std::path::Path) -> Result<String> {
    let out = run(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(out.trim().to_string())
}

/// Create a repo with a merge conflict for testing
fn setup_conflict_repo() -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let dir = tempfile::tempdir().context("tempdir")?;
    let repo = dir.path().to_path_buf();

    // Initialize repo
    run(&repo, &["init"])?;
    run(&repo, &["config", "user.email", "test@example.com"])?;
    run(&repo, &["config", "user.name", "Test"])?;

    // Base commit
    write(
        &repo,
        "file.txt",
        "line1\nkeep\nconflictA\nkeep2\nconflictB\nend\n",
    )?;
    run(&repo, &["add", "."])?;
    run(&repo, &["commit", "-m", "base"])?;

    // Branch A modifies both conflict spots
    run(&repo, &["checkout", "-b", "branchA"])?;
    write(
        &repo,
        "file.txt",
        "line1\nkeep\nconflictA_from_A\nkeep2\nconflictB_from_A\nend\n",
    )?;
    run(&repo, &["add", "file.txt"])?;
    run(&repo, &["commit", "-m", "A changes"])?;

    // Back to main and modify in different way
    let _default_branch = current_branch(&repo)?;
    // Go back to initial branch (master or main)
    run(&repo, &["checkout", "-"])?;
    write(
        &repo,
        "file.txt",
        "line1\nkeep\nconflictA_from_main\nkeep2\nconflictB_from_main\nend\n",
    )?;
    run(&repo, &["add", "file.txt"])?;
    run(&repo, &["commit", "-m", "main changes"])?;

    // Merge -> conflict (expect failure)
    let (success, _, _) = run_allow_fail(&repo, &["merge", "branchA"])?;
    assert!(!success, "merge should fail with conflict");

    Ok((dir, repo))
}

#[test]
fn fake_resolver_removes_markers() -> Result<()> {
    let input = ConflictInput {
        path: "test.txt".to_string(),
        base: Some("base content".to_string()),
        ours: Some("ours content".to_string()),
        theirs: Some("theirs content".to_string()),
        working: "before\n<<<<<<< HEAD\nours line\n=======\ntheirs line\n>>>>>>> branch\nafter\n"
            .to_string(),
        regions: vec![],
    };

    let resolver = FakeResolver;
    let result = resolver.resolve(&input)?;

    assert!(!result.contains("<<<<<<<"));
    assert!(!result.contains("======="));
    assert!(!result.contains(">>>>>>>"));
    assert!(result.contains("ours line"));
    assert!(result.contains("theirs line"));
    assert!(result.contains("before"));
    assert!(result.contains("after"));

    Ok(())
}

#[test]
fn fake_resolver_handles_multiple_conflicts() -> Result<()> {
    let input = ConflictInput {
        path: "test.txt".to_string(),
        base: Some("".to_string()),
        ours: Some("".to_string()),
        theirs: Some("".to_string()),
        working: concat!(
            "start\n",
            "<<<<<<< HEAD\n",
            "ours1\n",
            "=======\n",
            "theirs1\n",
            ">>>>>>> branch\n",
            "middle\n",
            "<<<<<<< HEAD\n",
            "ours2\n",
            "=======\n",
            "theirs2\n",
            ">>>>>>> branch\n",
            "end\n"
        )
        .to_string(),
        regions: vec![],
    };

    let resolver = FakeResolver;
    let result = resolver.resolve(&input)?;

    assert!(!result.contains("<<<<<<<"));
    assert!(result.contains("ours1"));
    assert!(result.contains("theirs1"));
    assert!(result.contains("ours2"));
    assert!(result.contains("theirs2"));
    assert!(result.contains("start"));
    assert!(result.contains("middle"));
    assert!(result.contains("end"));

    Ok(())
}

#[tokio::test]
async fn resolve_detects_conflicts_in_repo() -> Result<()> {
    let (_dir, repo) = setup_conflict_repo()?;

    // Verify conflict exists
    let u = run(&repo, &["diff", "--name-only", "--diff-filter=U"])?;
    assert!(
        u.lines().any(|l| l.trim() == "file.txt"),
        "Should have conflicted file"
    );

    // Verify markers exist in file
    let content = fs::read_to_string(repo.join("file.txt"))?;
    assert!(content.contains("<<<<<<<"), "Should have conflict markers");

    Ok(())
}

#[tokio::test]
async fn resolve_with_fake_resolver_removes_markers() -> Result<()> {
    let (_dir, repo) = setup_conflict_repo()?;

    // Change to repo directory
    let prev = std::env::current_dir()?;
    std::env::set_current_dir(&repo)?;

    // Run resolve with fake resolver
    let result = cmd_fix_with_resolver(&FakeResolver, true, true).await;

    // Restore directory
    std::env::set_current_dir(prev)?;

    result?;

    // Assert markers removed
    let content = fs::read_to_string(repo.join("file.txt"))?;
    assert!(
        !content.contains("<<<<<<<"),
        "Should not have <<<<<<< markers"
    );
    assert!(
        !content.contains("======="),
        "Should not have ======= markers"
    );
    assert!(
        !content.contains(">>>>>>>"),
        "Should not have >>>>>>> markers"
    );

    // Assert file was staged
    let cached = run(&repo, &["diff", "--cached", "--name-only"])?;
    assert!(
        cached.lines().any(|l| l.trim() == "file.txt"),
        "File should be staged"
    );

    // Assert no longer conflicted
    let u2 = run(&repo, &["diff", "--name-only", "--diff-filter=U"])?;
    assert!(
        !u2.lines().any(|l| l.trim() == "file.txt"),
        "File should not be conflicted"
    );

    Ok(())
}
