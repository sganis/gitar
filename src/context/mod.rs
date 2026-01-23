// src/context/mod.rs
//
// Everything related to context: LLM prompt preparation and repository state.
// Re-exports maintain backward compatibility with existing commands.

// NOTE:
// This module is currently not used by the active command pipeline (commands use crate::diff).
// Because this is a binary crate, `pub use` items can still trigger `unused_imports` warnings.
// We keep the re-exports for compatibility, but suppress the lint locally.
#![allow(unused_imports)]

pub mod algo;
pub mod diff;
pub mod preset;
pub mod secret;
pub mod template;
pub mod repo;

// Re-export for backward compatibility with existing code
pub use algo::{compare_algorithms, shape_diff, DiffAlg, DiffStats, ShapedDiff};
pub use diff::{
    build_context_header, process_commit, process_raw_diff, process_ref, process_staged,
    process_unstaged, DiffOptions, ProcessedDiff,
};
pub use preset::Preset;
pub use secret::SecretAction;

// Re-export repo context functions
pub use repo::{load_all_context, load_project_context, load_user_context};
