// src/context/algo/full.rs - Algorithm 1: Full diff with truncation only

use super::{split_diff_by_file, DiffAlg, DiffStats, ShapedDiff};

pub(crate) fn alg_full(raw_diff: &str, diff_stat: Option<&str>, max_chars: usize) -> ShapedDiff {
    let chunks = split_diff_by_file(raw_diff);
    let mut stats = DiffStats::new(DiffAlg::Full, chunks.len(), raw_diff.len());
    stats.included_files = chunks.len();
    stats.file_list = chunks.iter().map(|c| c.path.clone()).collect();

    let mut output = String::new();

    if let Some(stat) = diff_stat {
        output.push_str(stat.trim());
        output.push_str("\n\n");
    }

    let header_len = output.len();
    let available = max_chars.saturating_sub(header_len + 50);

    if raw_diff.len() > available {
        stats.truncated = true;
        let mut truncate_at = available;
        if let Some(pos) = raw_diff[..available].rfind("\ndiff --git") {
            if pos > available / 2 {
                truncate_at = pos;
            }
        }
        output.push_str(&raw_diff[..truncate_at]);
        output.push_str("\n\n[... truncated ...]");
    } else {
        output.push_str(raw_diff);
    }

    stats.finalize(&output);
    ShapedDiff {
        content: output,
        stats,
    }
}
