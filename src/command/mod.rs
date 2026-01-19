// src/command/mod.rs
mod changelog;
mod commit;
mod config;
mod diff;
mod explain;
mod history;
mod hook;
mod models;
mod pr;
mod version;

pub use changelog::cmd_changelog;
pub use commit::{cmd_commit, cmd_staged, cmd_unstaged};
pub use config::{cmd_config, cmd_init};
pub use diff::cmd_diff;
pub use explain::cmd_explain;
pub use history::cmd_history;
pub use hook::cmd_hook;
pub use models::cmd_models;
pub use pr::cmd_pr;
pub use version::cmd_version;

use anyhow::{bail, Result};
use crate::prompt::algo::{DiffAlg, DiffStats};
use crate::prompt::diff::get_llm_diff_preview;
use crate::prompt::secret::{process_secrets, SecretAction};

/// Short hash length for display
pub const SHORT_HASH_LEN: usize = 8;

/// Context information for analysis header
#[derive(Debug, Default)]
pub struct AnalysisContext {
    pub provider: Option<String>,
    pub model: Option<String>,
}

impl AnalysisContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn display(&self, stats: &DiffStats) -> String {
        let provider = self.provider.as_deref().unwrap_or("default");
        let model = self.model.as_deref().unwrap_or("default");

        let reduction_pct = if stats.total_chars > 0 {
            (1.0 - stats.output_chars as f64 / stats.total_chars as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "---- Gitar Context ----\n\
             Model      : {}/{}\n\
             Diff algo  : {} - {}\n\
             Files      : {}/{} (truncated: {})\n\
             Chars      : {} → {} ({:.1}% reduction)\n\
             Est Tokens : ~{}\n\
             -----------------------\n",
            provider,
            model,
            stats.algorithm.num(),
            stats.algorithm.name(),
            stats.included_files,
            stats.total_files,
            if stats.truncated { "yes" } else { "no" },
            stats.total_chars,
            stats.output_chars,
            reduction_pct,
            stats.estimated_tokens
        )
    }
}

/// Shared helper: apply smart diff algorithm with secret scanning
pub(crate) fn apply_smart_diff(
    raw_diff: &str,
    max_chars: usize,
    silent: bool,
    alg: u8,
    secret_action: SecretAction,
) -> Result<String> {
    apply_smart_diff_with_context(raw_diff, max_chars, silent, alg, None, secret_action)
}

/// Shared helper: apply smart diff algorithm with context header and secret scanning
pub(crate) fn apply_smart_diff_with_context(
    raw_diff: &str,
    max_chars: usize,
    silent: bool,
    alg: u8,
    context: Option<&AnalysisContext>,
    secret_action: SecretAction,
) -> Result<String> {
    let algorithm = DiffAlg::from_num(alg);
    let (shaped_diff, stats) = get_llm_diff_preview(raw_diff, None, max_chars, algorithm, false);

    // Apply secret processing
    let final_diff = match process_secrets(&shaped_diff, secret_action) {
        Ok(processed) => processed.into_owned(),
        Err(scan_result) => {
            bail!(
                "Blocked: Found {} secret(s) in diff. Remove secrets or set secret_action = \"redact\" in ~/.gitar.toml",
                scan_result.total()
            );
        }
    };

    if !silent {
        match context {
            Some(ctx) => eprintln!("{}", ctx.display(&stats)),
            None => eprintln!("{}", stats.display()),
        }
    }

    Ok(final_diff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_context_display() {
        let ctx = AnalysisContext::new()
            .with_provider("openai")
            .with_model("gpt-4o");
        let stats = DiffStats {
            total_files: 5,
            included_files: 3,
            total_chars: 1000,
            output_chars: 500,
            estimated_tokens: 142,
            truncated: false,
            algorithm: DiffAlg::Semantic,
            file_list: vec!["main.rs".into()],
        };
        let display = ctx.display(&stats);
        assert!(display.contains("openai/gpt-4o"));
        assert!(display.contains("4 - Semantic"));
        assert!(display.contains("50.0% reduction"));
    }

    #[test]
    fn apply_smart_diff_blocks_secrets() {
        // Use algo=1 (Full) to test secret scanning without diff transformation
        // Key must be long enough: sk-proj- (8) + 20+ chars
        let diff = "diff --git a/config.rs b/config.rs\n+API_KEY=sk-proj-abcdefghij1234567890abcdef";
        let result = apply_smart_diff(diff, 10000, true, 1, SecretAction::Block);
        assert!(result.is_err(), "Should block diff containing secrets");
        assert!(result.unwrap_err().to_string().contains("Blocked"));
    }

    #[test]
    fn apply_smart_diff_redacts_secrets() {
        // Use algo=1 (Full) to test secret scanning without diff transformation
        let diff = "diff --git a/config.rs b/config.rs\n+API_KEY=sk-proj-abcdefghij1234567890abcdef";
        let result = apply_smart_diff(diff, 10000, true, 1, SecretAction::Redact);
        assert!(result.is_ok(), "Should succeed with redaction");
        let output = result.unwrap();
        assert!(output.contains("[REDACTED"), "Should contain redaction marker");
        assert!(!output.contains("sk-proj-abcdefghij"), "Should not contain original secret");
    }

    #[test]
    fn apply_smart_diff_clean_diff_passes() {
        let diff = "diff --git a/main.rs b/main.rs\n+fn main() { println!(\"hello\"); }";
        let result = apply_smart_diff(diff, 10000, true, 1, SecretAction::Block);
        assert!(result.is_ok(), "Clean diff should pass even with Block action");
    }

    #[test]
    fn apply_smart_diff_warn_allows_secrets() {
        let diff = "diff --git a/config.rs b/config.rs\n+API_KEY=sk-proj-abcdefghij1234567890abcdef";
        let result = apply_smart_diff(diff, 10000, true, 1, SecretAction::Warn);
        assert!(result.is_ok(), "Warn should allow secrets through");
        let output = result.unwrap();
        assert!(output.contains("sk-proj-"), "Warn should preserve original secret");
    }
}