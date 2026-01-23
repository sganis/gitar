// src/command/mod.rs
mod changelog;
mod commit;
mod init;
mod config;
mod diff;
mod explain;
mod history;
mod hook;
mod models;
mod pr;
mod version;
mod resolve;
mod plan;
mod release;

pub use changelog::cmd_changelog;
pub use commit::{cmd_commit, cmd_staged, cmd_unstaged};
pub use config::cmd_config;
pub use init::cmd_init;
pub use diff::cmd_diff;
pub use explain::cmd_explain;
pub use history::cmd_history;
pub use hook::cmd_hook;
pub use models::cmd_models;
pub use pr::cmd_pr;
pub use version::cmd_version;
pub use resolve::cmd_resolve;
#[allow(unused_imports)]
pub use resolve::{cmd_resolve_with_resolver, ConflictInput, ConflictResolver};
pub use plan::{cmd_plan, AnalysisMode};
pub use release::cmd_release;

// Re-export utilities from util module for backward compatibility
pub(crate) use crate::util::{
    apply_smart_diff, apply_smart_diff_with_context, AnalysisContext, SHORT_HASH_LEN,
};


