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
use crate::diff::{get_llm_diff_preview, split_diff_by_file, DiffAlg};
use crate::git::get_current_branch;

/// Context information for analysis header
#[derive(Debug, Default)]
pub struct AnalysisContext {
    pub branch: Option<String>,
    pub range: Option<String>,
    pub commit_count: Option<usize>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

impl AnalysisContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_branch(mut self) -> Self {
        self.branch = Some(get_current_branch());
        self
    }

    pub fn with_range(mut self, range: impl Into<String>) -> Self {
        self.range = Some(range.into());
        self
    }

    pub fn with_commits(mut self, count: usize) -> Self {
        self.commit_count = Some(count);
        self
    }

    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn display(&self, files_changed: usize) -> String {
        let mut lines = Vec::new();

        // Line 1: Analysis summary
        let mut summary = String::from("Analyzing ");
        if let Some(count) = self.commit_count {
            summary.push_str(&format!("{} commit{}", count, if count == 1 { "" } else { "s" }));
            if let Some(ref range) = self.range {
                summary.push_str(&format!(" ({})", range));
            }
        } else {
            summary.push_str("working changes");
        }
        if let Some(ref branch) = self.branch {
            summary.push_str(&format!(" on {}", branch));
        }
        lines.push(summary);

        // Line 2: Files changed
        lines.push(format!("Files changed: {}", files_changed));

        // Line 3: Provider/Model (if available)
        if self.provider.is_some() || self.model.is_some() {
            let provider = self.provider.as_deref().unwrap_or("default");
            let model = self.model.as_deref().unwrap_or("default");
            lines.push(format!("Provider: {} | Model: {}", provider, model));
        }

        // Build bordered box
        let max_len = lines.iter().map(|l| l.len()).max().unwrap_or(40).max(40);
        let border = "─".repeat(max_len + 2);

        let mut output = String::new();
        output.push_str(&format!("╭─ Context {}╮\n", border.chars().skip(10).collect::<String>()));
        for line in &lines {
            output.push_str(&format!("│ {:<width$} │\n", line, width = max_len));
        }
        output.push_str(&format!("╰{}╯", border));

        output
    }
}

/// Shared helper: apply smart diff algorithm with optional context header
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
        // Print context header if provided
        if let Some(ctx) = context {
            let files_changed = split_diff_by_file(raw_diff)
                .iter()
                .filter(|c| c.priority > 0)
                .count();
            eprintln!("{}", ctx.display(files_changed));
        }
        eprintln!("{}", stats.display());
    }

    Ok(shaped_diff)
}