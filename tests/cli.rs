// tests/cli.rs
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

/// Set HOME/USERPROFILE so tests don't read/write the developer's real ~/.gitar.toml.
/// (dirs::home_dir() uses different env vars across platforms.)
fn with_isolated_home(cmd: &mut Command, home: &std::path::Path) {
    cmd.env("HOME", home);
    cmd.env("USERPROFILE", home);
    // These help on some Windows setups:
    cmd.env("HOMEDRIVE", "C:");
    cmd.env("HOMEPATH", home.to_string_lossy().to_string());
}

#[test]
fn help_works() {
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, tmp.path());

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("AI-powered Git assistant"));
}

#[test]
fn version_works() {
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, tmp.path());

    cmd.arg("--version").assert().success();
}

#[test]
fn diff_fails_outside_git_repo() {
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cmd, tmp.path());
    cmd.current_dir(tmp.path());

    // Your main.rs bails with "Not a git repository"
    cmd.arg("diff")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn init_and_config_commands_work_outside_repo() {
    let tmp = tempfile::tempdir().unwrap();

    // init
    let mut init_cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut init_cmd, tmp.path());
    init_cmd.current_dir(tmp.path());
    init_cmd.arg("init").assert().success();

    // config (should not require a repo)
    let mut cfg_cmd = Command::new(assert_cmd::cargo_bin!("gitar"));
    with_isolated_home(&mut cfg_cmd, tmp.path());
    cfg_cmd.current_dir(tmp.path());
    cfg_cmd.arg("config").assert().success();
}