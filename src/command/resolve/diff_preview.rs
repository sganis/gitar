use anyhow::{Context, Result};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn nanos_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn sanitize_path(p: &str) -> String {
    p.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn write_tmp_pair(label: &str, before: &str, after: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let tmpdir = std::env::temp_dir();
    let stamp = nanos_now();
    let label = sanitize_path(label);

    let before_path = tmpdir.join(format!("gitar-resolve-{}-before-{}", stamp, label));
    let after_path = tmpdir.join(format!("gitar-resolve-{}-after-{}", stamp, label));

    fs::write(&before_path, before.as_bytes()).ok();
    fs::write(&after_path, after.as_bytes()).ok();

    (before_path, after_path)
}

fn run_git_diff_no_index(before_path: &std::path::Path, after_path: &std::path::Path) -> Result<()> {
    let out = Command::new("git")
        .args([
            "diff",
            "--no-index",
            "--",
            before_path.to_string_lossy().as_ref(),
            after_path.to_string_lossy().as_ref(),
        ])
        .output()
        .context("git diff --no-index failed")?;

    // git diff returns 1 when different; that's normal.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    if !stdout.trim().is_empty() {
        eprintln!("{}", stdout);
    }
    if !stderr.trim().is_empty() {
        eprintln!("{}", stderr);
    }

    Ok(())
}

pub fn show_preview_diff(path: &str, before: &str, after: &str) -> Result<()> {
    let (before_path, after_path) = write_tmp_pair(path, before, after);

    let res = run_git_diff_no_index(&before_path, &after_path);

    let _ = fs::remove_file(before_path);
    let _ = fs::remove_file(after_path);

    res
}

pub fn show_preview_diff_virtual(label: &str, before: &str, after: &str) -> Result<()> {
    let (before_path, after_path) = write_tmp_pair(label, before, after);

    let res = run_git_diff_no_index(&before_path, &after_path);

    let _ = fs::remove_file(before_path);
    let _ = fs::remove_file(after_path);

    res
}
