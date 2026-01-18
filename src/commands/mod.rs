// src/commands/mod.rs
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

use anyhow::Result;
use crate::diff::{get_llm_diff_preview, DiffAlg, DiffStats};

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
            "---- Gitar Context -----------------------------\n\
             Model      : {}/{}\n\
             Diff algo  : {} - {}\n\
             Files      : {}/{} included (truncated: {})\n\
             Chars      : {} → {} ({:.1}% reduction)\n\
             Est Tokens : ~{}\n\
             --------------------------------------------------\n",
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

/// Shared helper: apply smart diff algorithm
pub(crate) fn apply_smart_diff(
    raw_diff: &str,
    max_chars: usize,
    silent: bool,
    alg: u8,
) -> Result<String> {
    apply_smart_diff_with_context(raw_diff, max_chars, silent, alg, None)
}

/// Shared helper: apply smart diff algorithm with context header
pub(crate) fn apply_smart_diff_with_context(
    raw_diff: &str,
    max_chars: usize,
    silent: bool,
    alg: u8,
    context: Option<&AnalysisContext>,
) -> Result<String> {
    let algorithm = DiffAlg::from_num(alg);
    let (shaped_diff, stats) = get_llm_diff_preview(raw_diff, None, max_chars, algorithm, false);

    if !silent {
        match context {
            Some(ctx) => eprintln!("{}", ctx.display(&stats)),
            None => eprintln!("{}", stats.display()),
        }
    }

    Ok(shaped_diff)
}