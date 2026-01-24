// src/context/mod.rs
//
// Everything related to context: LLM prompt preparation and repository state.
// Re-exports maintain backward compatibility with existing commands.

pub mod algo;
pub mod diff;
pub mod preset;
pub mod repo;
pub mod secret;
pub mod template;

// Re-export for backward compatibility with existing code
#[allow(unused_imports)]
pub use algo::{compare_algorithms, shape_diff, DiffAlg, DiffStats, ShapedDiff};
#[allow(unused_imports)]
pub use diff::{
    build_context_header, process_commit, process_raw_diff, process_ref, process_staged,
    process_unstaged, DiffOptions, ProcessedDiff,
};
#[allow(unused_imports)]
pub use preset::Preset;
#[allow(unused_imports)]
pub use secret::SecretAction;

// Re-export repo context functions
#[allow(unused_imports)]
pub use repo::{load_all_context, load_project_context, load_user_context};
