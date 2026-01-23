// src/util/mod.rs
pub mod diff;

pub use diff::{
    apply_smart_diff, apply_smart_diff_with_context, AnalysisContext, SHORT_HASH_LEN,
};
