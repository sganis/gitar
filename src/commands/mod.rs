// src/commands/mod.rs
mod admin;
mod changelog;
mod commit;
mod diff;
mod explain;
mod history;
mod pr;
mod version;

pub use admin::{cmd_config, cmd_hook, cmd_init, cmd_models};
pub use changelog::cmd_changelog;
pub use commit::{cmd_commit, cmd_staged, cmd_unstaged};
pub use diff::cmd_diff;
pub use explain::cmd_explain;
pub use history::cmd_history;
pub use pr::cmd_pr;
pub use version::cmd_version;

use anyhow::Result;
use crate::diff::{get_llm_diff_preview, DiffAlg};

/// Shared helper: apply smart diff algorithm
pub(crate) fn apply_smart_diff(
    raw_diff: &str,
    max_chars: usize,
    silent: bool,
    alg: u8,
) -> Result<String> {
    let algorithm = DiffAlg::from_num(alg);
    let (shaped_diff, stats) = get_llm_diff_preview(raw_diff, None, max_chars, algorithm, false);

    if !silent {
        eprintln!("{}", stats.display());
    }

    Ok(shaped_diff)
}
