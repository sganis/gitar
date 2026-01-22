// tests/resolve.rs
use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

use gitar::command::{cmd_resolve_with_resolver, ConflictInput, ConflictResolver};

struct FakeResolver;

impl ConflictResolver for FakeResolver {
    fn resolve(&self, input: &ConflictInput) -> Result<String> {
        // Deterministic: for each conflict, keep BOTH sides in order (ours then theirs).
        // We do this by parsing the existing marker structure ourselves and rebuilding.
        // This is intentionally simple and stable for tests.
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

fn current_branch(repo: &std::path::Path) -> Result<String> {
    let out = run(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(out.trim().to_string())
}


fn run(repo: &std::path::Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
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

fn write(repo: &std::path::Path, rel: &str, content: &str) -> Result<()> {
    let p = repo.join(rel);
    fs::write(&p, content).with_context(|| format!("write {}", rel))?;
    Ok(())
}

// #[tokio::test]
// async fn resolve_removes_markers_and_stages_file() -> Result<()> {
//     let dir = tempfile::tempdir().context("tempdir")?;
//     let repo = dir.path();

//     run(repo, &["init"])?;
//     run(repo, &["config", "user.email", "test@example.com"])?;
//     run(repo, &["config", "user.name", "Test"])?;

//     // Base commit
//     write(
//         repo,
//         "file.txt",
//         "\
// line1
// keep
// conflictA
// keep2
// conflictB
// end
// ",
//     )?;
//     run(repo, &["add", "."])?;
//     run(repo, &["commit", "-m", "base"])?;

//     // Branch A modifies both conflict spots
//     run(repo, &["checkout", "-b", "branchA"])?;
//     write(
//         repo,
//         "file.txt",
//         "\
// line1
// keep
// conflictA_from_A
// keep2
// conflictB_from_A
// end
// ",
//     )?;
//     run(repo, &["add", "file.txt"])?;
//     run(repo, &["commit", "-m", "A changes"])?;

//     // Back to main and modify in different way to create conflict
//     let default_branch = current_branch(repo)?;
//     run(repo, &["checkout", &default_branch])?;
//     write(
//         repo,
//         "file.txt",
//         "\
// line1
// keep
// conflictA_from_main
// keep2
// conflictB_from_main
// end
// ",
//     )?;
//     run(repo, &["add", "file.txt"])?;
//     run(repo, &["commit", "-m", "main changes"])?;

//     // Merge -> conflict
//     let merge_out = Command::new("git")
//         .current_dir(repo)
//         .args(["merge", "branchA"])
//         .output()
//         .context("git merge")?;
//     assert!(
//         !merge_out.status.success(),
//         "merge should conflict"
//     );

//     // Ensure conflict exists
//     let u = run(repo, &["diff", "--name-only", "--diff-filter=U"])?;
//     assert!(u.lines().any(|l| l.trim() == "file.txt"));

//     // Run resolve with fake resolver in repo dir
//     let prev = std::env::current_dir()?;
//     std::env::set_current_dir(repo)?;
//     let r = cmd_resolve_with_resolver(&FakeResolver, true, true).await;
//     std::env::set_current_dir(prev)?;
//     r?;

//     // Assert markers removed
//     let content = fs::read_to_string(repo.join("file.txt"))?;
//     assert!(!content.contains("<<<<<<<"));
//     assert!(!content.contains("======="));
//     assert!(!content.contains(">>>>>>>"));

//     // Assert staged
//     let cached = run(repo, &["diff", "--cached", "--name-only"])?;
//     assert!(cached.lines().any(|l| l.trim() == "file.txt"));

//     // Assert no longer conflicted
//     let u2 = run(repo, &["diff", "--name-only", "--diff-filter=U"])?;
//     assert!(!u2.lines().any(|l| l.trim() == "file.txt"));

//     Ok(())
// }
