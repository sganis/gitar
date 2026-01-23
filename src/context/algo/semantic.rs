// src/context/algo/semantic.rs - Algorithm 4: Semantic JSON IR (token-efficient)

use super::hunk::ScoredHunk;
use super::{split_diff_by_file, DiffAlg, DiffStats, FileChunk, ShapedDiff};
use std::collections::HashMap;

pub(crate) fn alg_semantic(
    raw_diff: &str,
    diff_stat: Option<&str>,
    max_chars: usize,
) -> ShapedDiff {
    let chunks = split_diff_by_file(raw_diff);
    let mut stats = DiffStats::new(DiffAlg::Semantic, chunks.len(), raw_diff.len());

    let files = summarize_files(&chunks);
    stats.file_list = files.iter().map(|f| f.path.clone()).collect();

    let mut max_hunks = 12usize;
    let mut preview_lines = 30usize;
    let mut json: String;

    loop {
        let hunks = extract_ir_hunks(&chunks, max_hunks, preview_lines);
        json = build_ir_json(diff_stat, &files, &hunks, chunks.len(), raw_diff.len());

        if json.len() <= max_chars {
            break;
        }
        if preview_lines > 8 {
            preview_lines = (preview_lines * 2 / 3).max(8);
            continue;
        }
        if max_hunks > 3 {
            max_hunks -= 2;
            continue;
        }
        if preview_lines > 0 {
            preview_lines = 0;
            continue;
        }

        stats.truncated = true;
        json.truncate(max_chars);
        break;
    }

    stats.included_files = files.len();
    stats.finalize(&json);
    ShapedDiff {
        content: json,
        stats,
    }
}

// =============================================================================
// INTERNAL TYPES
// =============================================================================

#[derive(Debug, Clone)]
struct IrFile {
    path: String,
    status: String,
    adds: usize,
    dels: usize,
}

#[derive(Debug, Clone)]
struct IrHunk {
    file: String,
    header: String,
    adds: usize,
    dels: usize,
    preview: String,
}

// =============================================================================
// HELPERS
// =============================================================================

fn summarize_files(chunks: &[FileChunk]) -> Vec<IrFile> {
    let mut files: Vec<IrFile> = chunks
        .iter()
        .filter(|c| c.priority > 0)
        .map(|c| IrFile {
            path: c.path.clone(),
            status: detect_status(&c.content),
            adds: c.adds,
            dels: c.dels,
        })
        .collect();

    files.sort_by(|a, b| (b.adds + b.dels).cmp(&(a.adds + a.dels)));
    files
}

fn detect_status(diff: &str) -> String {
    if diff.contains("new file mode") {
        "A".into()
    } else if diff.contains("deleted file mode") {
        "D".into()
    } else if diff.contains("rename from") {
        "R".into()
    } else {
        "M".into()
    }
}

fn extract_ir_hunks(chunks: &[FileChunk], max_hunks: usize, preview_lines: usize) -> Vec<IrHunk> {
    let mut all: Vec<ScoredHunk> = chunks
        .iter()
        .filter(|c| c.priority > 0)
        .flat_map(|c| super::hunk::extract_hunks(&c.content, &c.path, c.priority))
        .collect();

    all.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut per_file: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();

    for h in all {
        if out.len() >= max_hunks {
            break;
        }
        let cnt = per_file.entry(h.file.clone()).or_insert(0);
        if *cnt >= 4 {
            continue;
        }

        let mut adds = 0usize;
        let mut dels = 0usize;
        let mut preview = String::new();
        let mut header = String::new();

        for (i, line) in h.content.lines().enumerate() {
            if i == 0 {
                header = line.to_string();
            }
            if line.starts_with('+') && !line.starts_with("+++") {
                adds += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                dels += 1;
            }
            if i < preview_lines {
                preview.push_str(line);
                preview.push('\n');
            }
        }

        out.push(IrHunk {
            file: h.file.clone(),
            header,
            adds,
            dels,
            preview: preview.trim_end().to_string(),
        });
        *cnt += 1;
    }

    out
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(ch),
        }
    }
    out
}

fn build_ir_json(
    stat: Option<&str>,
    files: &[IrFile],
    hunks: &[IrHunk],
    total: usize,
    chars: usize,
) -> String {
    let (adds, dels) = files
        .iter()
        .fold((0, 0), |(a, d), f| (a + f.adds, d + f.dels));

    let mut s = String::with_capacity(chars / 2);
    s.push('{');

    if let Some(st) = stat {
        s.push_str(&format!("\"stat\":\"{}\",", json_escape(st.trim())));
    }

    s.push_str(&format!(
        "\"summary\":{{\"files\":{},\"included\":{},\"adds\":{},\"dels\":{}}},",
        total,
        files.len(),
        adds,
        dels
    ));

    s.push_str("\"files\":[");
    for (i, f) in files.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"p\":\"{}\",\"s\":\"{}\",\"a\":{},\"d\":{}}}",
            json_escape(&f.path),
            f.status,
            f.adds,
            f.dels
        ));
    }
    s.push_str("],");

    s.push_str("\"hunks\":[");
    for (i, h) in hunks.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"f\":\"{}\",\"h\":\"{}\",\"a\":{},\"d\":{},\"pv\":\"{}\"}}",
            json_escape(&h.file),
            json_escape(&h.header),
            h.adds,
            h.dels,
            json_escape(&h.preview)
        ));
    }
    s.push_str("]}");

    s
}
