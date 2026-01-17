// src/diff.rs - Intelligent diff selection for LLM context optimization
//
// Algorithm Overview:
// 1 - Full:     Complete git diff output (no filtering)
// 2 - Files:    Selective files, ranked by priority (default)
// 3 - Hunks:    Selective hunks, ranked by importance
// 4 - Semantic: JSON IR with scored hunks (token-efficient)

use std::collections::HashMap;

/// Estimated tokens ≈ chars / 3.5 for code (conservative)
const CHARS_PER_TOKEN: f32 = 3.5;

/// File priority scores (higher = more important)
const PRIORITY_SCORES: &[(&str, i32)] = &[
    // High priority - core logic
    ("main.rs", 100),
    ("lib.rs", 100),
    ("mod.rs", 80),
    (".rs", 70),
    (".py", 70),
    (".ts", 65),
    (".js", 60),
    // Medium priority - config/docs
    ("Cargo.toml", 50),
    ("pyproject.toml", 50),
    ("README.md", 40),
    (".md", 30),
    (".toml", 30),
    (".yaml", 25),
    (".yml", 25),
    // Low priority - usually noise
    (".json", 15),
    (".css", 10),
    (".svg", 5),
];

/// Files to always exclude (noise)
const EXCLUDE_FILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "Pipfile.lock",
    ".gitignore",
    ".DS_Store",
];

/// Patterns indicating generated/vendored code
const EXCLUDE_PATTERNS: &[&str] = &[
    "vendor/",
    "node_modules/",
    "target/",
    "dist/",
    "__pycache__/",
    ".min.js",
    ".min.css",
    "generated",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiffAlg {
    Full = 1,     // Complete git diff
    Files = 2,    // Selective files (default)
    Hunks = 3,    // Selective hunks
    Semantic = 4, // JSON IR
}

impl DiffAlg {
    pub fn from_num(n: u8) -> Self {
        match n {
            1 => Self::Full,
            2 => Self::Files,
            3 => Self::Hunks,
            4 => Self::Semantic,
            _ => Self::Files,
        }
    }

    pub fn num(&self) -> u8 {
        *self as u8
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Full => "Full Diff",
            Self::Files => "Selective Files",
            Self::Hunks => "Selective Hunks",
            Self::Semantic => "Semantic JSON",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileChunk {
    pub path: String,
    pub content: String,
    pub priority: i32,
    pub lines_added: usize,
    pub lines_removed: usize,
}

#[derive(Debug)]
pub struct DiffStats {
    pub total_files: usize,
    pub included_files: usize,
    pub excluded_files: usize,
    pub total_chars: usize,
    pub output_chars: usize,
    pub estimated_tokens: usize,
    pub truncated: bool,
    pub algorithm: DiffAlg,
}

impl DiffStats {
    pub fn display(&self) -> String {
        let reduction_pct = if self.total_chars > 0 {
            (1.0 - self.output_chars as f64 / self.total_chars as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "╭─ Diff Stats ─────────────────────────────────╮\n\
             │ Algorithm:  {} - {}\n\
             │ Files:      {}/{} included ({} excluded)\n\
             │ Chars:      {} → {} ({:.1}% reduction)\n\
             │ Est Tokens: ~{}\n\
             │ Truncated:  {}\n\
             ╰──────────────────────────────────────────────╯",
            self.algorithm.num(),
            self.algorithm.name(),
            self.included_files,
            self.total_files,
            self.excluded_files,
            self.total_chars,
            self.output_chars,
            reduction_pct,
            self.estimated_tokens,
            if self.truncated { "yes" } else { "no" }
        )
    }
}

/// Split raw diff into file chunks
pub fn split_diff_by_file(raw_diff: &str) -> Vec<FileChunk> {
    let mut chunks = Vec::new();
    let mut current_path = String::new();
    let mut current_content = String::new();
    let mut lines_added = 0usize;
    let mut lines_removed = 0usize;

    for line in raw_diff.lines() {
        if line.starts_with("diff --git") {
            // Save previous chunk
            if !current_path.is_empty() {
                let priority = calculate_priority(&current_path);
                chunks.push(FileChunk {
                    path: current_path.clone(),
                    content: current_content.clone(),
                    priority,
                    lines_added,
                    lines_removed,
                });
            }

            // Extract path: "diff --git a/path b/path"
            current_path = line.split(" b/").last().unwrap_or("").to_string();
            current_content = format!("{}\n", line);
            lines_added = 0;
            lines_removed = 0;
        } else {
            current_content.push_str(line);
            current_content.push('\n');

            if line.starts_with('+') && !line.starts_with("+++") {
                lines_added += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                lines_removed += 1;
            }
        }
    }

    // Don't forget last chunk
    if !current_path.is_empty() {
        let priority = calculate_priority(&current_path);
        chunks.push(FileChunk {
            path: current_path,
            content: current_content,
            priority,
            lines_added,
            lines_removed,
        });
    }

    chunks
}

fn calculate_priority(path: &str) -> i32 {
    // Check exclusions first
    for exclude in EXCLUDE_FILES {
        if path.ends_with(exclude) {
            return -100;
        }
    }
    for pattern in EXCLUDE_PATTERNS {
        if path.contains(pattern) {
            return -100;
        }
    }

    // Find best matching priority
    let mut best_score = 20; // default for unknown files
    for (pattern, score) in PRIORITY_SCORES {
        if path.ends_with(pattern) || path.contains(pattern) {
            if *score > best_score {
                best_score = *score;
            }
        }
    }

    best_score
}

/// Algorithm 1: Full - complete git diff output with optional truncation
fn alg_full(raw_diff: &str, diff_stats: Option<&str>, max_chars: usize) -> (String, DiffStats) {
    let chunks = split_diff_by_file(raw_diff);
    let total_files = chunks.len();
    let total_chars = raw_diff.len();

    let mut output = String::new();

    // Add stats header if provided
    if let Some(stats) = diff_stats {
        output.push_str("=== diff --stat ===\n");
        output.push_str(stats);
        output.push_str("\n\n");
    }

    output.push_str("=== full diff ===\n\n");

    let header_len = output.len();
    let available = max_chars.saturating_sub(header_len + 50);

    let truncated = raw_diff.len() > available;

    if truncated {
        // Truncate at file boundary if possible
        let mut truncate_at = available;
        if let Some(pos) = raw_diff[..available].rfind("\ndiff --git") {
            if pos > available / 2 {
                truncate_at = pos;
            }
        }
        output.push_str(&raw_diff[..truncate_at]);
        output.push_str("\n\n[... truncated ...]\n");
    } else {
        output.push_str(raw_diff);
    }

    let stats = DiffStats {
        total_files,
        included_files: total_files,
        excluded_files: 0,
        total_chars,
        output_chars: output.len(),
        estimated_tokens: (output.len() as f32 / CHARS_PER_TOKEN) as usize,
        truncated,
        algorithm: DiffAlg::Full,
    };

    (output, stats)
}

/// Algorithm 2: Files - Selective files, ranked by priority (default)
fn alg_files(raw_diff: &str, diff_stats: Option<&str>, max_chars: usize) -> (String, DiffStats) {
    let mut chunks = split_diff_by_file(raw_diff);
    let total_files = chunks.len();
    let total_chars = raw_diff.len();

    // Filter out excluded files
    chunks.retain(|c| c.priority > 0);

    // Sort by priority (highest first), then by change size
    chunks.sort_by(|a, b| {
        b.priority.cmp(&a.priority).then_with(|| {
            (b.lines_added + b.lines_removed).cmp(&(a.lines_added + a.lines_removed))
        })
    });

    // Build header
    let mut output = String::new();
    if let Some(stats) = diff_stats {
        output.push_str("=== diff --stat ===\n");
        output.push_str(stats);
        output.push_str("\n\n");
    }

    output.push_str("=== files (by priority) ===\n");
    for chunk in &chunks {
        output.push_str(&format!(
            "  [p:{}] {} (+{}/-{})\n",
            chunk.priority, chunk.path, chunk.lines_added, chunk.lines_removed
        ));
    }
    output.push_str("\n=== patches ===\n\n");

    let header_len = output.len();
    let available = max_chars.saturating_sub(header_len + 50); // reserve for truncation msg

    // Pack whole files until budget exhausted
    let mut included = 0usize;
    let mut excluded_names: Vec<String> = Vec::new();
    let mut truncated = false;

    for chunk in &chunks {
        if output.len() + chunk.content.len() <= header_len + available {
            output.push_str(&chunk.content);
            output.push('\n');
            included += 1;
        } else {
            excluded_names.push(chunk.path.clone());
            truncated = true;
        }
    }

    if !excluded_names.is_empty() {
        output.push_str(&format!(
            "\n[... {} files excluded due to size limit: {} ...]\n",
            excluded_names.len(),
            excluded_names.join(", ")
        ));
    }

    let stats = DiffStats {
        total_files,
        included_files: included,
        excluded_files: total_files.saturating_sub(included),
        total_chars,
        output_chars: output.len(),
        estimated_tokens: (output.len() as f32 / CHARS_PER_TOKEN) as usize,
        truncated,
        algorithm: DiffAlg::Files,
    };

    (output, stats)
}

/// Algorithm 3: Hunks - Selective hunks, ranked by importance
fn alg_hunks(raw_diff: &str, diff_stats: Option<&str>, max_chars: usize) -> (String, DiffStats) {
    let chunks = split_diff_by_file(raw_diff);
    let total_files = chunks.len();
    let total_chars = raw_diff.len();

    // Parse hunks from all files
    let mut all_hunks: Vec<ScoredHunk> = Vec::new();

    for chunk in &chunks {
        if chunk.priority <= 0 {
            continue;
        }
        let hunks = extract_hunks(&chunk.content, &chunk.path, chunk.priority);
        all_hunks.extend(hunks);
    }

    // Sort by score
    all_hunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Build output
    let mut output = String::new();
    if let Some(stats) = diff_stats {
        output.push_str("=== diff --stat ===\n");
        output.push_str(stats);
        output.push_str("\n\n");
    }

    output.push_str("=== hunks (by semantic score) ===\n\n");

    let header_len = output.len();
    let available = max_chars.saturating_sub(header_len + 50);

    let mut included_files: HashMap<String, bool> = HashMap::new();
    let mut truncated = false;

    let max_hunks_per_file = 3usize;
    let mut per_file_count: HashMap<String, usize> = HashMap::new();

    for hunk in &all_hunks {
        let count = per_file_count.entry(hunk.file_path.clone()).or_insert(0);

        // Enforce per-file cap
        if *count >= max_hunks_per_file {
            continue;
        }

        if output.len() + hunk.content.len() <= header_len + available {
            // Add file header if first hunk from this file
            if !included_files.contains_key(&hunk.file_path) {
                output.push_str(&format!("--- {} ---\n", hunk.file_path));
                included_files.insert(hunk.file_path.clone(), true);
            }

            output.push_str(&hunk.content);
            output.push('\n');

            *count += 1;
        } else {
            truncated = true;
        }
    }

    if truncated {
        output.push_str("\n[... additional hunks excluded due to size limit ...]\n");
    }

    let stats = DiffStats {
        total_files,
        included_files: included_files.len(),
        excluded_files: total_files.saturating_sub(included_files.len()),
        total_chars,
        output_chars: output.len(),
        estimated_tokens: (output.len() as f32 / CHARS_PER_TOKEN) as usize,
        truncated,
        algorithm: DiffAlg::Hunks,
    };

    (output, stats)
}

#[derive(Debug)]
struct ScoredHunk {
    file_path: String,
    content: String,
    score: f32,
}

fn extract_hunks(file_diff: &str, file_path: &str, file_priority: i32) -> Vec<ScoredHunk> {
    let mut hunks = Vec::new();
    let mut current_hunk = String::new();
    let mut in_hunk = false;

    for line in file_diff.lines() {
        if line.starts_with("@@") {
            if !current_hunk.is_empty() {
                let score = score_hunk(&current_hunk, file_priority);
                hunks.push(ScoredHunk {
                    file_path: file_path.to_string(),
                    content: current_hunk.clone(),
                    score,
                });
            }
            current_hunk = format!("{}\n", line);
            in_hunk = true;
        } else if in_hunk {
            current_hunk.push_str(line);
            current_hunk.push('\n');
        }
    }

    // Last hunk
    if !current_hunk.is_empty() {
        let score = score_hunk(&current_hunk, file_priority);
        hunks.push(ScoredHunk {
            file_path: file_path.to_string(),
            content: current_hunk,
            score,
        });
    }

    hunks
}

fn score_hunk(hunk: &str, file_priority: i32) -> f32 {
    let mut score = file_priority as f32;

    // Boost for structural changes
    let structural_keywords = [
        "fn ", "pub ", "impl ", "struct ", "enum ", "trait ", "def ", "class ", "async ",
        "function ", "const ", "export ",
    ];
    for kw in structural_keywords {
        if hunk.contains(&format!("+{}", kw)) || hunk.contains(&format!("-{}", kw)) {
            score += 20.0;
        }
    }

    // Boost for meaningful line changes (not just whitespace)
    let meaningful_changes = hunk
        .lines()
        .filter(|l| (l.starts_with('+') || l.starts_with('-')) && l.trim().len() > 3)
        .count();
    score += meaningful_changes as f32 * 2.0;

    // Penalty for large hunks (prefer focused changes)
    let total_lines = hunk.lines().count();
    if total_lines > 50 {
        score -= (total_lines - 50) as f32 * 0.5;
    }

    score
}

pub fn get_llm_diff_preview(
    raw_diff: &str,
    diff_stats: Option<&str>,
    max_chars: usize,
    alg: DiffAlg,
    include_header: bool,
) -> (String, DiffStats) {
    let (shaped_diff, stats) = match alg {
        DiffAlg::Full => alg_full(raw_diff, diff_stats, max_chars),
        DiffAlg::Files => alg_files(raw_diff, diff_stats, max_chars),
        DiffAlg::Hunks => alg_hunks(raw_diff, diff_stats, max_chars),
        DiffAlg::Semantic => alg_semantic(raw_diff, diff_stats, max_chars),
    };

    if include_header {
        let header = format!(
            "=== gitar LLM DIFF PREVIEW ===\n\
             alg: {} - {}\n\
             max_chars: {}\n\
             ===============================\n\n",
            alg.num(),
            alg.name(),
            max_chars
        );
        (format!("{}{}", header, shaped_diff), stats)
    } else {
        (shaped_diff, stats)
    }
}

// =============================================================================
// Algorithm 4: Semantic - JSON IR with scored hunks
// =============================================================================
#[derive(Debug, Clone)]
struct IrFile {
    path: String,
    status: String, // M/A/D/R
    priority: i32,
    adds: usize,
    dels: usize,
}

#[derive(Debug, Clone)]
struct IrHunk {
    file: String,
    header: String,
    adds: usize,
    dels: usize,
    score: f32,
    preview: String,
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
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

fn detect_status(file_diff: &str) -> String {
    if file_diff.contains("new file mode") {
        return "A".into();
    }
    if file_diff.contains("deleted file mode") {
        return "D".into();
    }
    if file_diff.contains("rename from") || file_diff.contains("rename to") {
        return "R".into();
    }
    "M".into()
}

fn summarize_files(chunks: &[FileChunk]) -> Vec<IrFile> {
    let mut files = Vec::new();
    for c in chunks {
        if c.priority <= 0 {
            continue;
        }
        files.push(IrFile {
            path: c.path.clone(),
            status: detect_status(&c.content),
            priority: c.priority,
            adds: c.lines_added,
            dels: c.lines_removed,
        });
    }

    files.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| (b.adds + b.dels).cmp(&(a.adds + a.dels)))
    });

    files
}

fn extract_ranked_hunks_for_ir(chunks: &[FileChunk], max_hunks: usize, preview_lines: usize) -> Vec<IrHunk> {
    let mut all: Vec<ScoredHunk> = Vec::new();

    for c in chunks {
        if c.priority <= 0 {
            continue;
        }
        all.extend(extract_hunks(&c.content, &c.path, c.priority));
    }

    all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // avoid one file dominating
    let mut per_file: HashMap<String, usize> = HashMap::new();
    let per_file_cap = 3usize;

    let mut out: Vec<IrHunk> = Vec::new();
    for h in all {
        if out.len() >= max_hunks {
            break;
        }
        let cnt = per_file.entry(h.file_path.clone()).or_insert(0);
        if *cnt >= per_file_cap {
            continue;
        }

        let mut adds = 0usize;
        let mut dels = 0usize;
        let mut preview = String::new();
        let mut header = String::new();

        for (i, line) in h.content.lines().enumerate() {
            if i == 0 {
                header = line.to_string(); // @@ header
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
            file: h.file_path.clone(),
            header,
            adds,
            dels,
            score: h.score,
            preview: preview.trim_end().to_string(),
        });

        *cnt += 1;
    }

    out
}

fn build_ir_json(
    stat: Option<&str>,
    files: &[IrFile],
    hunks: &[IrHunk],
    total_files: usize,
    total_chars: usize,
) -> String {
    let (mut total_adds, mut total_dels) = (0usize, 0usize);
    for f in files {
        total_adds += f.adds;
        total_dels += f.dels;
    }

    let mut s = String::new();
    s.push('{');

    if let Some(st) = stat {
        s.push_str("\"stat\":\"");
        s.push_str(&json_escape(st.trim()));
        s.push_str("\",");
    }

    s.push_str(&format!(
        "\"totals\":{{\"files_total\":{},\"files_included\":{},\"adds\":{},\"dels\":{},\"chars_total\":{}}},",
        total_files,
        files.len(),
        total_adds,
        total_dels,
        total_chars
    ));

    s.push_str("\"files\":[");
    for (i, f) in files.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('{');
        s.push_str("\"p\":\"");
        s.push_str(&json_escape(&f.path));
        s.push_str("\",\"s\":\"");
        s.push_str(&f.status);
        s.push_str("\",");
        s.push_str(&format!("\"pri\":{},\"a\":{},\"d\":{}", f.priority, f.adds, f.dels));
        s.push('}');
    }
    s.push_str("],");

    s.push_str("\"hunks\":[");
    for (i, h) in hunks.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('{');
        s.push_str("\"f\":\"");
        s.push_str(&json_escape(&h.file));
        s.push_str("\",\"hdr\":\"");
        s.push_str(&json_escape(&h.header));
        s.push_str("\",");
        s.push_str(&format!("\"a\":{},\"d\":{},\"sc\":{:.2},", h.adds, h.dels, h.score));
        s.push_str("\"pv\":\"");
        s.push_str(&json_escape(&h.preview));
        s.push_str("\"}");
    }
    s.push_str("]");

    s.push('}');
    s
}

fn alg_semantic(raw_diff: &str, diff_stats: Option<&str>, max_chars: usize) -> (String, DiffStats) {
    let chunks = split_diff_by_file(raw_diff);
    let total_files = chunks.len();
    let total_chars = raw_diff.len();

    let files = summarize_files(&chunks);

    // adaptive sizing
    let mut max_hunks = 10usize;
    let mut preview_lines = 25usize;

    let mut json: String;

    loop {
        let hunks = extract_ranked_hunks_for_ir(&chunks, max_hunks, preview_lines);
        json = build_ir_json(diff_stats, &files, &hunks, total_files, total_chars);

        if json.len() <= max_chars {
            break;
        }

        if preview_lines > 5 {
            preview_lines = (preview_lines / 2).max(5);
            continue;
        }
        if max_hunks > 1 {
            max_hunks -= 1;
            continue;
        }
        if preview_lines != 0 {
            preview_lines = 0;
            continue;
        }
        break;
    }

    let truncated = json.len() > max_chars;
    if truncated {
        json.truncate(max_chars);
    }

    let stats = DiffStats {
        total_files,
        included_files: files.len(),
        excluded_files: total_files.saturating_sub(files.len()),
        total_chars,
        output_chars: json.len(),
        estimated_tokens: (json.len() as f32 / CHARS_PER_TOKEN) as usize,
        truncated,
        algorithm: DiffAlg::Semantic,
    };

    (json, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DIFF: &str = r#"diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdefg 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,6 +10,8 @@ fn main() {
     println!("Hello");
+    println!("World");
+    let x = 42;
 }
diff --git a/Cargo.lock b/Cargo.lock
index aaaaaaa..bbbbbbb 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1,3 +1,3 @@
-version = "1.0"
+version = "1.1"
diff --git a/src/lib.rs b/src/lib.rs
index ccccccc..ddddddd 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -5,4 +5,6 @@ pub fn add(a: i32, b: i32) -> i32 {
     a + b
+}
+pub fn multiply(a: i32, b: i32) -> i32 {
+    a * b
 }
"#;

    #[test]
    fn test_split_diff() {
        let chunks = split_diff_by_file(SAMPLE_DIFF);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].path, "src/main.rs");
        assert_eq!(chunks[1].path, "Cargo.lock");
        assert_eq!(chunks[2].path, "src/lib.rs");
    }

    #[test]
    fn test_priority_scoring() {
        assert!(calculate_priority("src/main.rs") > calculate_priority("Cargo.lock"));
        assert!(calculate_priority("src/lib.rs") > calculate_priority("README.md"));
        assert!(calculate_priority("Cargo.lock") < 0); // excluded
    }

    #[test]
    fn test_alg_from_num() {
        assert_eq!(DiffAlg::from_num(1), DiffAlg::Full);
        assert_eq!(DiffAlg::from_num(2), DiffAlg::Files);
        assert_eq!(DiffAlg::from_num(3), DiffAlg::Hunks);
        assert_eq!(DiffAlg::from_num(4), DiffAlg::Semantic);
        assert_eq!(DiffAlg::from_num(0), DiffAlg::Files); // fallback
        assert_eq!(DiffAlg::from_num(5), DiffAlg::Files); // fallback
    }

    #[test]
    fn test_alg_num() {
        assert_eq!(DiffAlg::Full.num(), 1);
        assert_eq!(DiffAlg::Files.num(), 2);
        assert_eq!(DiffAlg::Hunks.num(), 3);
        assert_eq!(DiffAlg::Semantic.num(), 4);
    }

    #[test]
    fn test_full_includes_all() {
        let (output, stats) = alg_full(SAMPLE_DIFF, None, 100); // max_chars ignored
        assert!(output.contains("Cargo.lock")); // full doesn't exclude
        assert_eq!(stats.algorithm, DiffAlg::Full);
        assert_eq!(stats.excluded_files, 0);
        assert!(!stats.truncated);
        assert_eq!(stats.total_chars, stats.output_chars); // no reduction
    }

    #[test]
    fn test_files_excludes_lock_files() {
        let (output, stats) = alg_files(SAMPLE_DIFF, None, 10000);
        assert!(!output.contains("Cargo.lock"));
        assert_eq!(stats.algorithm, DiffAlg::Files);
    }

    #[test]
    fn test_hunks_excludes_lock_files() {
        let (output, stats) = alg_hunks(SAMPLE_DIFF, None, 10000);
        assert!(!output.contains("Cargo.lock"));
        assert_eq!(stats.algorithm, DiffAlg::Hunks);
    }

    #[test]
    fn test_semantic_builds_json() {
        let (output, stats) = alg_semantic(SAMPLE_DIFF, Some("fake stat"), 10000);
        assert!(output.starts_with('{') && output.ends_with('}'));
        assert_eq!(stats.algorithm, DiffAlg::Semantic);
    }

    #[test]
    fn test_alg_names() {
        assert_eq!(DiffAlg::Full.name(), "Full Diff");
        assert_eq!(DiffAlg::Files.name(), "Selective Files");
        assert_eq!(DiffAlg::Hunks.name(), "Selective Hunks");
        assert_eq!(DiffAlg::Semantic.name(), "Semantic JSON");
    }

    #[test]
    fn test_diff_stats_display() {
        let stats = DiffStats {
            total_files: 5,
            included_files: 3,
            excluded_files: 2,
            total_chars: 1000,
            output_chars: 500,
            estimated_tokens: 142,
            truncated: false,
            algorithm: DiffAlg::Files,
        };
        let display = stats.display();
        assert!(display.contains("2 - Selective Files"));
        assert!(display.contains("3/5 included"));
        assert!(display.contains("50.0% reduction"));
    }
}
