// src/util/mod.rs
pub mod diff;
pub mod file;

pub use diff::{apply_smart_diff, apply_smart_diff_with_context, AnalysisContext, SHORT_HASH_LEN};
pub use file::{categorize_file, group_by_heuristics, FileCategory, Groupable};
