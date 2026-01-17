// src/diff.rs - Intelligent diff selection for LLM context optimization
//
// Algorithm Overview:
// Standard  - File-aware packing with priority scoring (default)
// Think     - Hunk-level semantic packing (--think flag, for large refactors)

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
pub enum SmartDiffAlg {
    Standard, // File-aware structured (default)
    Think,    // Hunk-level semantic (--think)
}

impl SmartDiffAlg {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Standard => "File-Aware Structured",
            Self::Think => "Hunk-Level Semantic",
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
    pub algorithm: SmartDiffAlg,
}

impl DiffStats {
    pub fn display(&self) -> String {
        let alg_label = match self.algorithm {
            SmartDiffAlg::Standard => "standard",
            SmartDiffAlg::Think => "think",
        };
        format!(
            "╭─ Diff Stats ─────────────────────────╮\n\
             │ Algorithm:  {} ({})        \n\
             │ Files:      {}/{} included          \n\
             │ Chars:      {} → {} ({:.1}% reduction)\n\
             │ Est Tokens: ~{}                     \n\
             │ Truncated:  {}                      \n\
             ╰──────────────────────────────────────╯",
            alg_label,
            self.algorithm.name(),
            self.included_files,
            self.total_files,
            self.total_chars,
            self.output_chars,
            if self.total_chars > 0 {
                (1.0 - self.output_chars as f64 / self.total_chars as f64) * 100.0
            } else {
                0.0
            },
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
    let mut lines_added = 0;
    let mut lines_removed = 0;

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
            current_path = line
                .split(" b/")
                .last()
                .unwrap_or("")
                .to_string();
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

/// Standard algorithm: File-aware structured packing (default)
fn alg_standard(
    raw_diff: &str,
    diff_stats: Option<&str>,
    max_chars: usize,
) -> (String, DiffStats) {
    let mut chunks = split_diff_by_file(raw_diff);
    let total_files = chunks.len();
    let total_chars = raw_diff.len();

    // Filter out excluded files
    chunks.retain(|c| c.priority > 0);

    // Sort by priority (highest first), then by change size
    chunks.sort_by(|a, b| {
        b.priority.cmp(&a.priority)
            .then_with(|| (b.lines_added + b.lines_removed).cmp(&(a.lines_added + a.lines_removed)))
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
    let mut included = 0;
    let mut excluded_names = Vec::new();
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
        excluded_files: total_files - included,
        total_chars,
        output_chars: output.len(),
        estimated_tokens: (output.len() as f32 / CHARS_PER_TOKEN) as usize,
        truncated,
        algorithm: SmartDiffAlg::Standard,
    };

    (output, stats)
}

/// Think algorithm: Hunk-level semantic packing (--think flag)
fn alg_think(
    raw_diff: &str,
    diff_stats: Option<&str>,
    max_chars: usize,
) -> (String, DiffStats) {
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
    all_hunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

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

    let max_hunks_per_file = 3;
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
        excluded_files: total_files - included_files.len(),
        total_chars,
        output_chars: output.len(),
        estimated_tokens: (output.len() as f32 / CHARS_PER_TOKEN) as usize,
        truncated,
        algorithm: SmartDiffAlg::Think,
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
    let structural_keywords = ["fn ", "pub ", "impl ", "struct ", "enum ", "trait ", 
                               "def ", "class ", "async ", "function ", "const ", "export "];
    for kw in structural_keywords {
        if hunk.contains(&format!("+{}", kw)) || hunk.contains(&format!("-{}", kw)) {
            score += 20.0;
        }
    }

    // Boost for meaningful line changes (not just whitespace)
    let meaningful_changes = hunk.lines()
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

/// Main entry point for LLM diff preview
pub fn get_llm_diff_preview(
    raw_diff: &str,
    diff_stats: Option<&str>,
    max_chars: usize,
    alg: SmartDiffAlg,
    include_header: bool,
) -> (String, DiffStats) {
    let (shaped_diff, stats) = match alg {
        SmartDiffAlg::Standard => alg_standard(raw_diff, diff_stats, max_chars),
        SmartDiffAlg::Think => alg_think(raw_diff, diff_stats, max_chars),
    };

    if include_header {
        let alg_label = match alg {
            SmartDiffAlg::Standard => "standard",
            SmartDiffAlg::Think => "think",
        };
        let header = format!(
            "=== gitar LLM DIFF PREVIEW ===\n\
             alg: {} ({})\n\
             max_chars: {}\n\
             ===============================\n\n",
            alg_label,
            alg.name(),
            max_chars
        );
        (format!("{}{}", header, shaped_diff), stats)
    } else {
        (shaped_diff, stats)
    }
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
    fn test_standard_excludes_lock_files() {
        let (output, stats) = alg_standard(SAMPLE_DIFF, None, 10000);
        assert!(!output.contains("Cargo.lock"));
        assert!(stats.excluded_files > 0 || stats.included_files < stats.total_files);
    }

    #[test]
    fn test_think_excludes_lock_files() {
        let (output, stats) = alg_think(SAMPLE_DIFF, None, 10000);
        assert!(!output.contains("Cargo.lock"));
        assert!(stats.algorithm == SmartDiffAlg::Think);
    }

    #[test]
    fn test_alg_names() {
        assert_eq!(SmartDiffAlg::Standard.name(), "File-Aware Structured");
        assert_eq!(SmartDiffAlg::Think.name(), "Hunk-Level Semantic");
    }
}