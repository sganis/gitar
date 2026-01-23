// src/context/algo/hunk.rs - Algorithm 3: Selective hunks by importance

use super::{split_diff_by_file, DiffAlg, DiffStats, ShapedDiff};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct ScoredHunk {
    pub file: String,
    pub content: String,
    pub score: f32,
}

pub(crate) fn alg_hunks(raw_diff: &str, diff_stat: Option<&str>, max_chars: usize) -> ShapedDiff {
    let chunks = split_diff_by_file(raw_diff);
    let mut stats = DiffStats::new(DiffAlg::Hunks, chunks.len(), raw_diff.len());

    let mut all_hunks: Vec<ScoredHunk> = chunks
        .iter()
        .filter(|c| c.priority > 0)
        .flat_map(|c| extract_hunks(&c.content, &c.path, c.priority))
        .collect();

    all_hunks.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut output = String::new();

    if let Some(stat) = diff_stat {
        output.push_str(stat.trim());
        output.push_str("\n\n");
    }

    let header_len = output.len();
    let available = max_chars.saturating_sub(header_len + 100);

    let mut included_files: HashMap<String, bool> = HashMap::new();
    let mut per_file_count: HashMap<String, usize> = HashMap::new();
    let max_per_file = 4usize;

    for hunk in &all_hunks {
        let count = per_file_count.entry(hunk.file.clone()).or_insert(0);
        if *count >= max_per_file {
            continue;
        }

        if output.len() + hunk.content.len() <= header_len + available {
            if !included_files.contains_key(&hunk.file) {
                output.push_str(&format!("--- {} ---\n", hunk.file));
                included_files.insert(hunk.file.clone(), true);
            }
            output.push_str(&hunk.content);
            output.push('\n');
            *count += 1;
        } else {
            stats.truncated = true;
        }
    }

    if stats.truncated {
        output.push_str("\n[... additional hunks excluded ...]\n");
    }

    stats.included_files = included_files.len();
    stats.file_list = included_files.keys().cloned().collect();
    stats.finalize(&output);
    ShapedDiff {
        content: output,
        stats,
    }
}

pub(crate) fn extract_hunks(file_diff: &str, file_path: &str, file_priority: i32) -> Vec<ScoredHunk> {
    let mut hunks = Vec::new();
    let mut current = String::new();
    let mut in_hunk = false;

    for line in file_diff.lines() {
        if line.starts_with("@@") {
            if !current.is_empty() {
                hunks.push(ScoredHunk {
                    file: file_path.to_string(),
                    content: std::mem::take(&mut current),
                    score: score_hunk(&current, file_priority),
                });
            }
            current = format!("{}\n", line);
            in_hunk = true;
        } else if in_hunk {
            current.push_str(line);
            current.push('\n');
        }
    }

    if !current.is_empty() {
        let score = score_hunk(&current, file_priority);
        hunks.push(ScoredHunk {
            file: file_path.to_string(),
            content: current,
            score,
        });
    }

    hunks
}

fn score_hunk(hunk: &str, file_priority: i32) -> f32 {
    let mut score = file_priority as f32;

    const STRUCTURAL: &[&str] = &[
        "fn ",
        "pub ",
        "impl ",
        "struct ",
        "enum ",
        "trait ",
        "mod ",
        "def ",
        "class ",
        "async ",
        "function ",
        "const ",
        "export ",
        "import ",
    ];

    for kw in STRUCTURAL {
        if hunk.contains(&format!("+{}", kw)) || hunk.contains(&format!("-{}", kw)) {
            score += 15.0;
        }
    }

    let meaningful = hunk
        .lines()
        .filter(|l| (l.starts_with('+') || l.starts_with('-')) && l.trim().len() > 3)
        .count();
    score += meaningful as f32 * 2.0;

    let lines = hunk.lines().count();
    if lines > 50 {
        score -= (lines - 50) as f32 * 0.5;
    }

    score
}
