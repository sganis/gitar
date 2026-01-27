// src/context/repo.rs
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use crate::config::Config;

static CACHED_CONTEXT: OnceLock<(Option<String>, Option<String>)> = OnceLock::new();
static CACHED_REPO_ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Context filename
const CONTEXT_FILENAME: &str = "context.md";

fn read_text_file_if_exists(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Err(_) => None,
    }
}

fn repo_root_dir_uncached() -> Option<PathBuf> {
    // Use git to find repo root, so running from subfolders works.
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let s = String::from_utf8_lossy(&out.stdout);
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(PathBuf::from(t))
    }
}

fn repo_root_dir() -> Option<PathBuf> {
    // Cache repo root to avoid repeated `git rev-parse` calls.
    CACHED_REPO_ROOT
        .get_or_init(|| repo_root_dir_uncached())
        .clone()
}

fn strip_html_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let bytes = s.as_bytes();

    while i < bytes.len() {
        if i + 4 <= bytes.len() && &bytes[i..i + 4] == b"<!--" {
            i += 4;
            while i + 3 <= bytes.len() && &bytes[i..i + 3] != b"-->" {
                i += 1;
            }
            if i + 3 <= bytes.len() {
                i += 3; // skip -->
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

fn clean(opt: Option<String>) -> Option<String> {
    opt.and_then(|s| {
        let s = strip_html_comments(&s);
        let s = s.trim();
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    })
}

/// Load user context from ~/.gitar/context.md
/// Uses Config::config_base_dir() which respects GITAR_CONFIG_PATH env var
pub fn load_user_context() -> Option<String> {
    let base = Config::config_base_dir()?;
    let path = base.join(CONTEXT_FILENAME);
    clean(read_text_file_if_exists(&path))
}

/// Load project context from .gitar/context.md
pub fn load_project_context() -> Option<String> {
    let root = repo_root_dir()?;
    let path = root.join(".gitar").join(CONTEXT_FILENAME);
    clean(read_text_file_if_exists(&path))
}

pub fn load_all_context() -> (Option<String>, Option<String>) {
    // Cache both contexts so commands that call this repeatedly don't re-read files.
    CACHED_CONTEXT
        .get_or_init(|| (load_project_context(), load_user_context()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_comments_empty() {
        assert_eq!(strip_html_comments(""), "");
    }

    #[test]
    fn strip_html_comments_no_comments() {
        assert_eq!(strip_html_comments("hello world"), "hello world");
    }

    #[test]
    fn strip_html_comments_single() {
        assert_eq!(strip_html_comments("before<!-- comment -->after"), "beforeafter");
    }

    #[test]
    fn strip_html_comments_multiple() {
        let input = "a<!-- c1 -->b<!-- c2 -->c";
        assert_eq!(strip_html_comments(input), "abc");
    }

    #[test]
    fn clean_removes_comments() {
        let input = "  <!-- comment -->\nactual content\n<!-- another -->  ";
        let result = clean(Some(input.to_string()));
        assert_eq!(result, Some("actual content".to_string()));
    }

    #[test]
    fn clean_empty_after_strip() {
        let input = "<!-- only comments -->";
        let result = clean(Some(input.to_string()));
        assert!(result.is_none());
    }
}
