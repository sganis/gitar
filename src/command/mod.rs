// src/command/mod.rs
mod changelog;
mod commit;
mod config;
mod diff;
mod explain;
mod history;
mod hook;
mod init;
mod models;
mod pr;
mod release;
mod resolve;
mod rewrite;
mod run;
mod squash;

pub use changelog::cmd_changelog;
pub use commit::{cmd_commit, cmd_staged, cmd_unstaged};
pub use config::cmd_config;
pub use diff::cmd_diff;
pub use explain::cmd_explain;
pub use history::cmd_history;
pub use hook::cmd_hook;
pub use init::cmd_init;
pub use models::cmd_models;
pub use pr::cmd_pr;
pub use release::cmd_release;
pub use resolve::cmd_resolve;
#[allow(unused_imports)]
pub use resolve::{cmd_resolve_with_resolver, ConflictInput, ConflictResolver};
pub use rewrite::cmd_rewrite;
pub use run::{cmd_run, AnalysisMode};
pub use squash::cmd_squash;

// Re-export utilities from util module for backward compatibility
pub(crate) use crate::util::{
    apply_smart_diff_with_context, AnalysisContext, SHORT_HASH_LEN,
};
