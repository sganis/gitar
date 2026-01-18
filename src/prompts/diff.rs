// src/prompts/diff.rs - High-level diff processing for LLM consumption
//
// Orchestrates: git diff → algorithm shaping → secret scanning → result

use anyhow::{bail, Result};
use std::borrow::Cow;

use super::algo::{shape_diff, DiffAlg, DiffStats, ShapedDiff};
use super::secrets::{process_secrets, SecretAction, SecretScanResult};
use crate::git;

// =============================================================================
// TYPES
// =============================================================================

/// Fully processed diff ready for LLM consumption
#[derive(Debug)]
pub struct ProcessedDiff {
    pub content: String,
    pub stats: DiffStats,
    pub secrets_redacted: bool,
}

/// Options for diff processing
#[derive(Debug, Clone)]
pub struct DiffOptions {
    pub max_chars: usize,
    pub algorithm: DiffAlg,
    pub secret_action: SecretAction,
    pub include_stats: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            max_chars: 50_000,
            algorithm: DiffAlg::Semantic,
            secret_action: SecretAction::Redact,
            include_stats: true,
        }
    }
}

impl DiffOptions {
    pub fn new(max_chars: usize, alg: u8, secret_action: SecretAction) -> Self {
        Self {
            max_chars,
            algorithm: DiffAlg::from_num(alg),
            secret_action,
            include_stats: true,
        }
    }
}

// =============================================================================
// PUBLIC API
// =============================================================================

/// Process staged changes
pub fn process_staged(opts: &DiffOptions) -> Result<ProcessedDiff> {
    let raw = git::get_diff(None, true, opts.max_chars * 2)?;
    if raw.trim().is_empty() {
        bail!("No staged changes");
    }

    let stat = if opts.include_stats { git::get_diff_stats(None, true).ok() } else { None };
    process_raw_diff(&raw, stat.as_deref(), opts)
}

/// Process unstaged (working tree) changes
pub fn process_unstaged(opts: &DiffOptions) -> Result<ProcessedDiff> {
    let raw = git::get_diff(None, false, opts.max_chars * 2)?;
    if raw.trim().is_empty() {
        bail!("No unstaged changes");
    }

    let stat = if opts.include_stats { git::get_diff_stats(None, false).ok() } else { None };
    process_raw_diff(&raw, stat.as_deref(), opts)
}

/// Process a ref comparison (branch, tag, commit range)
pub fn process_ref(target: &str, opts: &DiffOptions) -> Result<ProcessedDiff> {
    let raw = git::get_diff(Some(target), false, opts.max_chars * 2)?;
    if raw.trim().is_empty() {
        bail!("No changes between refs");
    }

    let stat = if opts.include_stats { git::get_diff_stats(Some(target), false).ok() } else { None };
    process_raw_diff(&raw, stat.as_deref(), opts)
}

/// Process a specific commit's diff
pub fn process_commit(hash: &str, opts: &DiffOptions) -> Result<Option<ProcessedDiff>> {
    let raw = match git::get_commit_diff(hash, opts.max_chars * 2)? {
        Some(d) if !d.trim().is_empty() => d,
        _ => return Ok(None),
    };

    Ok(Some(process_raw_diff(&raw, None, opts)?))
}

/// Process a raw diff string directly
pub fn process_raw_diff(raw: &str, stat: Option<&str>, opts: &DiffOptions) -> Result<ProcessedDiff> {
    let ShapedDiff { content, stats } = shape_diff(raw, stat, opts.max_chars, opts.algorithm);
    apply_secrets(content, stats, opts.secret_action)
}

// =============================================================================
// CONTEXT HEADER
// =============================================================================

/// Build a context header for LLM prompts
pub fn build_context_header(stats: &DiffStats, model: &str) -> String {
    let files = if stats.file_list.is_empty() {
        format!("{}/{}", stats.included_files, stats.total_files)
    } else if stats.file_list.len() <= 4 {
        format!("{} ({})", stats.included_files, stats.file_list.join(", "))
    } else {
        format!(
            "{} ({}, ... +{} more)",
            stats.included_files,
            stats.file_list[..2].join(", "),
            stats.file_list.len() - 2
        )
    };

    let mut h = String::from("---- Gitar Context ----\n");
    h.push_str(&format!("Model      : {}\n", model));
    h.push_str(&format!("Diff algo  : {} - {}\n", stats.algorithm.num(), stats.algorithm.name()));
    h.push_str(&format!("Files      : {}\n", files));
    h.push_str(&format!("Chars      : {}\n", stats.output_chars));
    h.push_str(&format!("Est Tokens : ~{}\n", stats.estimated_tokens));
    if stats.truncated { h.push_str("Truncated  : yes\n"); }
    h.push_str("-----------------------\n");
    h
}

/// Build a minimal one-line context string
pub fn build_context_oneline(stats: &DiffStats) -> String {
    format!(
        "[{} | {} files | ~{} tokens{}]",
        stats.algorithm.name(),
        stats.included_files,
        stats.estimated_tokens,
        if stats.truncated { " | truncated" } else { "" }
    )
}

// =============================================================================
// INTERNAL
// =============================================================================

fn apply_secrets(content: String, stats: DiffStats, action: SecretAction) -> Result<ProcessedDiff> {
    match process_secrets(&content, action) {
        Ok(processed) => {
            let redacted = matches!(processed, Cow::Owned(_));
            Ok(ProcessedDiff {
                content: processed.into_owned(),
                stats,
                secrets_redacted: redacted,
            })
        }
        Err(scan) => bail!(
            "Blocked: Found {} secret(s) in diff\n{}",
            scan.total(),
            format_block_error(&scan)
        ),
    }
}

fn format_block_error(r: &SecretScanResult) -> String {
    let mut msg = String::from("\n\x1b[31mDetected secrets:\x1b[0m\n");

    for (i, m) in r.matches.iter().take(8).enumerate() {
        msg.push_str(&format!(
            "  {}. [\x1b[33m{}\x1b[0m] L{}: {}\n",
            i + 1, m.pattern_name, m.line_number,
            m.masked_preview().chars().take(60).collect::<String>()
        ));
    }

    if r.matches.len() > 8 {
        msg.push_str(&format!("  ... and {} more\n", r.matches.len() - 8));
    }

    msg.push_str("\n\x1b[36mOptions:\x1b[0m\n");
    msg.push_str("  • Remove secrets from your changes\n");
    msg.push_str("  • Set secret_action = \"redact\" in ~/.gitar.toml\n");
    msg.push_str("  • Set secret_action = \"warn\" to send anyway\n");
    msg
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default() {
        let o = DiffOptions::default();
        assert_eq!(o.max_chars, 50_000);
        assert_eq!(o.algorithm, DiffAlg::Semantic);
        assert_eq!(o.secret_action, SecretAction::Redact);
    }

    #[test]
    fn options_new() {
        let o = DiffOptions::new(10_000, 2, SecretAction::Block);
        assert_eq!(o.algorithm, DiffAlg::Files);
    }

    #[test]
    fn header_format() {
        let stats = DiffStats {
            total_files: 5, included_files: 3, total_chars: 1000,
            output_chars: 500, estimated_tokens: 142, truncated: false,
            algorithm: DiffAlg::Semantic, file_list: vec!["main.rs".into()],
        };
        let h = build_context_header(&stats, "gpt-4o");
        assert!(h.contains("gpt-4o"));
        assert!(h.contains("4 - Semantic"));
    }

    #[test]
    fn process_raw_clean() {
        let raw = "diff --git a/t.rs b/t.rs\n+fn main() {}";
        let r = process_raw_diff(raw, None, &DiffOptions::default()).unwrap();
        assert!(!r.secrets_redacted);
    }

    // #[test]
    // fn process_raw_redacts() {
    //     let raw = "diff --git a/t.rs b/t.rs\n+const K: &str = \"sk-proj-abc123def456ghi789jkl012mno345pqr\";";
    //     let r = process_raw_diff(raw, None, &DiffOptions::default()).unwrap();
    //     assert!(r.secrets_redacted);
    //     assert!(r.content.contains("[REDACTED"));
    // }

    // #[test]
    // fn process_raw_blocks() {
    //     let raw = "diff --git a/t.rs b/t.rs\n+const K: &str = \"sk-ant-api03-abcdefghijklmnopqrstuvwxyz\";";
    //     let o = DiffOptions::new(10_000, 4, SecretAction::Block);
    //     let r = process_raw_diff(raw, None, &o);
    //     assert!(r.is_err());
    // }
}