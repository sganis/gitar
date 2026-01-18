// tests/diff.rs
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command;

fn with_isolated_home(cmd: &mut Command, home: &Path) {
    cmd.env("HOME", home);
    cmd.env("USERPROFILE", home);
    cmd.env("HOMEDRIVE", "C:");
    cmd.env("HOMEPATH", home.to_string_lossy().to_string());
}

// Set minimal git identity so commits work on CI/Windows.
fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .expect("failed to run git (is git installed?)");
    assert!(status.success(), "git {:?} failed", args);
}

fn init_repo(repo: &Path) {
    git(repo, &["init"]);
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["config", "user.email", "test@example.com"]);
}

fn commit_file(repo: &Path, rel: &str, content: &str, msg: &str) {
    let path = repo.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, content).unwrap();
    git(repo, &["add", rel]);
    git(repo, &["commit", "-m", msg]);
}

#[test]
fn diff_shows_changes_in_repo() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    commit_file(repo.path(), "a.txt", "hello\n", "initial");

    // Modify file (unstaged diff)
    fs::write(repo.path().join("a.txt"), "hello world\n").unwrap();

    let mut cmd = Command::cargo_bin("gitar").unwrap();
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    cmd.args(["diff", "--max-chars", "2000"])
        .assert()
        .success()
        .stdout(predicate::str::contains("diff --git"));
}

#[test]
fn diff_stats_only_has_no_patch() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    commit_file(repo.path(), "a.txt", "hello\n", "initial");
    fs::write(repo.path().join("a.txt"), "hello world\n").unwrap();

    let mut cmd = Command::cargo_bin("gitar").unwrap();
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    cmd.args(["diff", "--stats-only", "--stats"])
        .assert()
        .success()
        // should show stat header (because --stats)
        .stdout(predicate::str::contains("diff --stat").or(predicate::str::contains("=== diff --stat ===")))
        // should not show patch
        .stdout(predicate::str::contains("diff --git").not());
}

#[test]
fn diff_max_chars_truncates() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    commit_file(repo.path(), "a.txt", "line\n", "initial");

    // Create a bigger change
    let big = "x".repeat(5000);
    fs::write(repo.path().join("a.txt"), big).unwrap();

    let mut cmd = Command::cargo_bin("gitar").unwrap();
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    cmd.args(["diff", "--max-chars", "200"])
        .assert()
        .success()
        .stdout(predicate::str::contains("truncated").or(predicate::str::contains("[...")));
}

#[test]
fn diff_compare_prints_all_algorithms() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    commit_file(repo.path(), "a.txt", "hello\n", "initial");
    fs::write(repo.path().join("a.txt"), "hello world\n").unwrap();

    let mut cmd = Command::cargo_bin("gitar").unwrap();
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    cmd.args(["diff", "--compare", "--max-chars", "5000"])
        .assert()
        .success()
        // Your command prints the banner + each algorithm stats.display() line includes "Alg:"
        .stdout(predicate::str::contains("ALGORITHM COMPARISON"))
        .stdout(predicate::str::contains("Alg: Full"))
        .stdout(predicate::str::contains("Alg: Files"))
        .stdout(predicate::str::contains("Alg: Hunks"))
        .stdout(predicate::str::contains("Alg: Semantic"));
}

#[test]
fn diff_algo_flag_changes_output_format() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    commit_file(repo.path(), "a.txt", "hello\n", "initial");
    fs::write(repo.path().join("a.txt"), "hello world\n").unwrap();

    // Algo 4 (Semantic) should produce JSON-ish output (starts with '{') because your alg_semantic builds JSON.
    let mut cmd = Command::cargo_bin("gitar").unwrap();
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    cmd.args(["diff", "--algo", "4", "--max-chars", "8000"])
        .assert()
        .success()
        .stdout(predicate::str::contains('{'));
}
