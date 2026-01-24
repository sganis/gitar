// tests/commands.rs
//! Integration tests for gitar commands
//!
//! Tests command execution without LLM calls by checking git operations
//! and command-line behavior.

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

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .expect("failed to run git");
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

// =============================================================================
// STAGED/UNSTAGED TESTS (require git repo, no LLM)
// =============================================================================

#[test]
fn staged_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("staged")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn unstaged_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("unstaged")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// =============================================================================
// RUN COMMAND TESTS
// =============================================================================

#[test]
fn run_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("run")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn run_suggest_works_in_repo() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    commit_file(repo.path(), "a.txt", "hello\n", "initial");

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    // --suggest mode doesn't require LLM
    cmd.args(["run", "--suggest"]).assert().success();
}

#[test]
fn run_detects_no_changes() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    commit_file(repo.path(), "a.txt", "hello\n", "initial");

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    // With no changes, run --suggest should indicate clean state
    cmd.args(["run", "--suggest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing").or(predicate::str::contains("clean")));
}

// =============================================================================
// RELEASE COMMAND TESTS
// =============================================================================

#[test]
fn release_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("release")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn release_dry_run_shows_plan() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    // Create Cargo.toml for version detection
    fs::write(
        repo.path().join("Cargo.toml"),
        r#"[package]
name = "test"
version = "1.0.0"
"#,
    )
    .unwrap();

    commit_file(repo.path(), "Cargo.toml", "", "Add Cargo.toml");
    commit_file(repo.path(), "a.txt", "hello\n", "initial");

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    // Dry run (default) shows release plan
    cmd.args(["release", "--bump", "patch", "--skip-changelog"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Release").or(predicate::str::contains("Dry run")));
}

// =============================================================================
// RESOLVE COMMAND TESTS
// =============================================================================

#[test]
fn resolve_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("resolve")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn resolve_fails_without_conflicts() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    commit_file(repo.path(), "a.txt", "hello\n", "initial");

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    // Should fail when no conflicts exist
    cmd.arg("resolve")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No conflict"));
}

// =============================================================================
// SQUASH COMMAND TESTS
// =============================================================================

#[test]
fn squash_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.args(["squash", "2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// =============================================================================
// REWRITE COMMAND TESTS
// =============================================================================

#[test]
fn rewrite_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.args(["rewrite", "2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// =============================================================================
// HOOK COMMAND TESTS
// =============================================================================

#[test]
fn hook_install_creates_hook_file() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(repo.path());

    cmd.args(["hook", "install"]).assert().success();

    // Check hook file exists
    let hook_path = repo.path().join(".git/hooks/prepare-commit-msg");
    assert!(hook_path.exists(), "Hook file should exist after install");

    // Check hook content
    let content = fs::read_to_string(&hook_path).unwrap();
    assert!(
        content.contains("gitar"),
        "Hook should contain gitar command"
    );
}

#[test]
fn hook_uninstall_removes_hook_file() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());

    // Install first
    let mut install_cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut install_cmd, home.path());
    install_cmd.current_dir(repo.path());
    install_cmd.args(["hook", "install"]).assert().success();

    // Then uninstall
    let mut uninstall_cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut uninstall_cmd, home.path());
    uninstall_cmd.current_dir(repo.path());
    uninstall_cmd.args(["hook", "uninstall"]).assert().success();

    // Check hook file is removed
    let hook_path = repo.path().join(".git/hooks/prepare-commit-msg");
    assert!(
        !hook_path.exists(),
        "Hook file should not exist after uninstall"
    );
}

// =============================================================================
// MODELS COMMAND TESTS
// =============================================================================

#[test]
fn models_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("models")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// =============================================================================
// PR COMMAND TESTS
// =============================================================================

#[test]
fn pr_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("pr")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// =============================================================================
// CHANGELOG COMMAND TESTS
// =============================================================================

#[test]
fn changelog_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("changelog")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// =============================================================================
// EXPLAIN COMMAND TESTS
// =============================================================================

#[test]
fn explain_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("explain")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// =============================================================================
// HISTORY COMMAND TESTS
// =============================================================================

#[test]
fn history_requires_git_repo() {
    let home = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, home.path());
    cmd.current_dir(tmp.path());

    cmd.arg("history")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}
