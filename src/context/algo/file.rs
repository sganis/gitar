// src/context/algo/file.rs - Algorithm 2: Selective files by priority

use super::{split_diff_by_file, DiffAlg, DiffStats, ShapedDiff};

pub(crate) fn alg_files(raw_diff: &str, diff_stat: Option<&str>, max_chars: usize) -> ShapedDiff {
    let mut chunks = split_diff_by_file(raw_diff);
    let mut stats = DiffStats::new(DiffAlg::Files, chunks.len(), raw_diff.len());

    chunks.retain(|c| c.priority > 0);
    chunks.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| (b.adds + b.dels).cmp(&(a.adds + a.dels)))
    });

    let mut output = String::new();

    if let Some(stat) = diff_stat {
        output.push_str(stat.trim());
        output.push_str("\n\n");
    }

    output.push_str("Files:\n");
    for c in &chunks {
        output.push_str(&format!("  {} (+{}/-{})\n", c.path, c.adds, c.dels));
    }
    output.push('\n');

    let header_len = output.len();
    let available = max_chars.saturating_sub(header_len + 100);

    let mut included = Vec::new();
    let mut excluded = Vec::new();
    let mut current_len = 0usize;

    for chunk in &chunks {
        if current_len + chunk.content.len() <= available {
            output.push_str(&chunk.content);
            output.push('\n');
            current_len += chunk.content.len() + 1;
            included.push(chunk.path.clone());
        } else {
            excluded.push(chunk.path.clone());
            stats.truncated = true;
        }
    }

    if !excluded.is_empty() {
        output.push_str(&format!(
            "\n[{} file(s) excluded: {}]\n",
            excluded.len(),
            excluded.join(", ")
        ));
    }

    stats.included_files = included.len();
    stats.file_list = included;
    stats.finalize(&output);
    ShapedDiff {
        content: output,
        stats,
    }
}
