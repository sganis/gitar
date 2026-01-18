// src/prompts/algo.rs - Diff shaping algorithms for LLM context optimization
//
// Pure transformation functions: raw diff in, shaped diff out.
// No I/O, no git calls, no secret scanning.
//
// Algorithms:
// 1 - Full:     Complete diff (truncate only)
// 2 - Files:    Selective files by priority
// 3 - Hunks:    Selective hunks by importance
// 4 - Semantic: JSON IR (token-efficient)

use std::collections::HashMap;

const CHARS_PER_TOKEN: f32 = 3.5;

const PRIORITY_SCORES: &[(&str, i32)] = &[
    ("main.rs", 100), ("lib.rs", 100), ("mod.rs", 80),
    (".rs", 70), (".py", 70), (".go", 65), (".ts", 65), (".js", 60),
    ("Cargo.toml", 50), ("pyproject.toml", 50), ("package.json", 45),
    ("README.md", 40), (".md", 30), (".toml", 30), (".yaml", 25), (".yml", 25),
    (".json", 15), (".css", 10), (".svg", 5),
];

const EXCLUDE_FILES: &[&str] = &[
    "Cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml",
    "poetry.lock", "Pipfile.lock", ".gitignore", ".DS_Store",
];

const EXCLUDE_PATTERNS: &[&str] = &[
    "vendor/", "node_modules/", "target/", "dist/", "__pycache__/",
    ".min.js", ".min.css", "generated",
];

// =============================================================================
// PUBLIC TYPES
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffAlg {
    Full = 1,
    Files = 2,
    Hunks = 3,
    #[default]
    Semantic = 4,
}

impl DiffAlg {
    pub fn from_num(n: u8) -> Self {
        match n {
            1 => Self::Full,
            2 => Self::Files,
            3 => Self::Hunks,
            4 => Self::Semantic,
            _ => Self::default(),
        }
    }

    pub fn num(&self) -> u8 {
        *self as u8
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Files => "Files",
            Self::Hunks => "Hunks",
            Self::Semantic => "Semantic",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileChunk {
    pub path: String,
    pub content: String,
    pub priority: i32,
    pub adds: usize,
    pub dels: usize,
}

#[derive(Debug, Clone)]
pub struct DiffStats {
    pub total_files: usize,
    pub included_files: usize,
    pub total_chars: usize,
    pub output_chars: usize,
    pub estimated_tokens: usize,
    pub truncated: bool,
    pub algorithm: DiffAlg,
    pub file_list: Vec<String>,
}

impl DiffStats {
    fn new(alg: DiffAlg, total_files: usize, total_chars: usize) -> Self {
        Self {
            total_files,
            included_files: 0,
            total_chars,
            output_chars: 0,
            estimated_tokens: 0,
            truncated: false,
            algorithm: alg,
            file_list: Vec::new(),
        }
    }

    fn finalize(&mut self, output: &str) {
        self.output_chars = output.len();
        self.estimated_tokens = (output.len() as f32 / CHARS_PER_TOKEN) as usize;
    }

    pub fn display(&self) -> String {
        let trunc = if self.truncated { " [truncated]" } else { "" };
        let files_str = if self.file_list.len() <= 5 {
            self.file_list.join(", ")
        } else {
            format!("{}, ... +{} more",
                self.file_list[..3].join(", "),
                self.file_list.len() - 3)
        };

        format!(
            "Alg: {} | Files: {}/{} | Chars: {} → {} | Tokens: ~{}{}\n  {}",
            self.algorithm.name(),
            self.included_files,
            self.total_files,
            self.total_chars,
            self.output_chars,
            self.estimated_tokens,
            trunc,
            files_str
        )
    }
}

#[derive(Debug)]
pub struct ShapedDiff {
    pub content: String,
    pub stats: DiffStats,
}

// =============================================================================
// MAIN ENTRY POINT
// =============================================================================

pub fn shape_diff(raw_diff: &str, diff_stat: Option<&str>, max_chars: usize, alg: DiffAlg) -> ShapedDiff {
    match alg {
        DiffAlg::Full => alg_full(raw_diff, diff_stat, max_chars),
        DiffAlg::Files => alg_files(raw_diff, diff_stat, max_chars),
        DiffAlg::Hunks => alg_hunks(raw_diff, diff_stat, max_chars),
        DiffAlg::Semantic => alg_semantic(raw_diff, diff_stat, max_chars),
    }
}

pub fn compare_algorithms(raw_diff: &str, max_chars: usize) -> String {
    let mut out = String::from("=== Algorithm Comparison ===\n\n");

    for alg in [DiffAlg::Full, DiffAlg::Files, DiffAlg::Hunks, DiffAlg::Semantic] {
        let result = shape_diff(raw_diff, None, max_chars, alg);
        out.push_str(&format!(
            "--- {} ---\nFiles: {}/{} | Chars: {} | Tokens: ~{} | Truncated: {}\n\n",
            alg.name(),
            result.stats.included_files,
            result.stats.total_files,
            result.stats.output_chars,
            result.stats.estimated_tokens,
            result.stats.truncated,
        ));
    }
    out
}

// =============================================================================
// ALGORITHM 1: FULL
// =============================================================================

fn alg_full(raw_diff: &str, diff_stat: Option<&str>, max_chars: usize) -> ShapedDiff {
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
    ShapedDiff { content: output, stats }
}

// =============================================================================
// ALGORITHM 2: FILES
// =============================================================================

fn alg_files(raw_diff: &str, diff_stat: Option<&str>, max_chars: usize) -> ShapedDiff {
    let mut chunks = split_diff_by_file(raw_diff);
    let mut stats = DiffStats::new(DiffAlg::Files, chunks.len(), raw_diff.len());

    chunks.retain(|c| c.priority > 0);
    chunks.sort_by(|a, b| {
        b.priority.cmp(&a.priority)
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
        output.push_str(&format!("\n[{} file(s) excluded: {}]\n", excluded.len(), excluded.join(", ")));
    }

    stats.included_files = included.len();
    stats.file_list = included;
    stats.finalize(&output);
    ShapedDiff { content: output, stats }
}

// =============================================================================
// ALGORITHM 3: HUNKS
// =============================================================================

fn alg_hunks(raw_diff: &str, diff_stat: Option<&str>, max_chars: usize) -> ShapedDiff {
    let chunks = split_diff_by_file(raw_diff);
    let mut stats = DiffStats::new(DiffAlg::Hunks, chunks.len(), raw_diff.len());

    let mut all_hunks: Vec<ScoredHunk> = chunks
        .iter()
        .filter(|c| c.priority > 0)
        .flat_map(|c| extract_hunks(&c.content, &c.path, c.priority))
        .collect();

    all_hunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

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
    ShapedDiff { content: output, stats }
}

// =============================================================================
// ALGORITHM 4: SEMANTIC (JSON IR)
// =============================================================================

fn alg_semantic(raw_diff: &str, diff_stat: Option<&str>, max_chars: usize) -> ShapedDiff {
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

        if json.len() <= max_chars { break; }
        if preview_lines > 8 { preview_lines = (preview_lines * 2 / 3).max(8); continue; }
        if max_hunks > 3 { max_hunks -= 2; continue; }
        if preview_lines > 0 { preview_lines = 0; continue; }

        stats.truncated = true;
        json.truncate(max_chars);
        break;
    }

    stats.included_files = files.len();
    stats.finalize(&json);
    ShapedDiff { content: json, stats }
}

// =============================================================================
// HELPERS
// =============================================================================

pub fn split_diff_by_file(raw_diff: &str) -> Vec<FileChunk> {
    let mut chunks = Vec::new();
    let mut path = String::new();
    let mut content = String::new();
    let mut adds = 0usize;
    let mut dels = 0usize;

    for line in raw_diff.lines() {
        if line.starts_with("diff --git") {
            if !path.is_empty() {
                chunks.push(FileChunk {
                    priority: calculate_priority(&path),
                    path: std::mem::take(&mut path),
                    content: std::mem::take(&mut content),
                    adds,
                    dels,
                });
            }
            path = line.split(" b/").last().unwrap_or("").to_string();
            content = format!("{}\n", line);
            adds = 0;
            dels = 0;
        } else {
            content.push_str(line);
            content.push('\n');
            if line.starts_with('+') && !line.starts_with("+++") { adds += 1; }
            else if line.starts_with('-') && !line.starts_with("---") { dels += 1; }
        }
    }

    if !path.is_empty() {
        chunks.push(FileChunk {
            priority: calculate_priority(&path),
            path,
            content,
            adds,
            dels,
        });
    }

    chunks
}

fn calculate_priority(path: &str) -> i32 {
    for ex in EXCLUDE_FILES {
        if path.ends_with(ex) { return -100; }
    }
    for pat in EXCLUDE_PATTERNS {
        if path.contains(pat) { return -100; }
    }

    let mut best = 20;
    for (pat, score) in PRIORITY_SCORES {
        if (path.ends_with(pat) || path.contains(pat)) && *score > best {
            best = *score;
        }
    }
    best
}

#[derive(Debug)]
struct ScoredHunk {
    file: String,
    content: String,
    score: f32,
}

fn extract_hunks(file_diff: &str, file_path: &str, file_priority: i32) -> Vec<ScoredHunk> {
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
        hunks.push(ScoredHunk { file: file_path.to_string(), content: current, score });
    }

    hunks
}

fn score_hunk(hunk: &str, file_priority: i32) -> f32 {
    let mut score = file_priority as f32;

    const STRUCTURAL: &[&str] = &[
        "fn ", "pub ", "impl ", "struct ", "enum ", "trait ", "mod ",
        "def ", "class ", "async ", "function ", "const ", "export ", "import ",
    ];

    for kw in STRUCTURAL {
        if hunk.contains(&format!("+{}", kw)) || hunk.contains(&format!("-{}", kw)) {
            score += 15.0;
        }
    }

    let meaningful = hunk.lines()
        .filter(|l| (l.starts_with('+') || l.starts_with('-')) && l.trim().len() > 3)
        .count();
    score += meaningful as f32 * 2.0;

    let lines = hunk.lines().count();
    if lines > 50 { score -= (lines - 50) as f32 * 0.5; }

    score
}

// --- Semantic IR ---

#[derive(Debug, Clone)]
struct IrFile { path: String, status: String, adds: usize, dels: usize }

#[derive(Debug, Clone)]
struct IrHunk { file: String, header: String, adds: usize, dels: usize, preview: String }

fn summarize_files(chunks: &[FileChunk]) -> Vec<IrFile> {
    let mut files: Vec<IrFile> = chunks.iter()
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
    if diff.contains("new file mode") { "A".into() }
    else if diff.contains("deleted file mode") { "D".into() }
    else if diff.contains("rename from") { "R".into() }
    else { "M".into() }
}

fn extract_ir_hunks(chunks: &[FileChunk], max_hunks: usize, preview_lines: usize) -> Vec<IrHunk> {
    let mut all: Vec<ScoredHunk> = chunks.iter()
        .filter(|c| c.priority > 0)
        .flat_map(|c| extract_hunks(&c.content, &c.path, c.priority))
        .collect();

    all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let mut per_file: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();

    for h in all {
        if out.len() >= max_hunks { break; }
        let cnt = per_file.entry(h.file.clone()).or_insert(0);
        if *cnt >= 4 { continue; }

        let mut adds = 0usize;
        let mut dels = 0usize;
        let mut preview = String::new();
        let mut header = String::new();

        for (i, line) in h.content.lines().enumerate() {
            if i == 0 { header = line.to_string(); }
            if line.starts_with('+') && !line.starts_with("+++") { adds += 1; }
            else if line.starts_with('-') && !line.starts_with("---") { dels += 1; }
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

fn build_ir_json(stat: Option<&str>, files: &[IrFile], hunks: &[IrHunk], total: usize, chars: usize) -> String {
    let (adds, dels) = files.iter().fold((0, 0), |(a, d), f| (a + f.adds, d + f.dels));

    let mut s = String::with_capacity(chars / 2);
    s.push('{');

    if let Some(st) = stat {
        s.push_str(&format!("\"stat\":\"{}\",", json_escape(st.trim())));
    }

    s.push_str(&format!(
        "\"summary\":{{\"files\":{},\"included\":{},\"adds\":{},\"dels\":{}}},",
        total, files.len(), adds, dels
    ));

    s.push_str("\"files\":[");
    for (i, f) in files.iter().enumerate() {
        if i > 0 { s.push(','); }
        s.push_str(&format!(
            "{{\"p\":\"{}\",\"s\":\"{}\",\"a\":{},\"d\":{}}}",
            json_escape(&f.path), f.status, f.adds, f.dels
        ));
    }
    s.push_str("],");

    s.push_str("\"hunks\":[");
    for (i, h) in hunks.iter().enumerate() {
        if i > 0 { s.push(','); }
        s.push_str(&format!(
            "{{\"f\":\"{}\",\"h\":\"{}\",\"a\":{},\"d\":{},\"pv\":\"{}\"}}",
            json_escape(&h.file), json_escape(&h.header), h.adds, h.dels, json_escape(&h.preview)
        ));
    }
    s.push_str("]}");

    s
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,6 +10,8 @@ fn main() {
+    println!("World");
+    let x = 42;
 }
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1,3 +1,3 @@
-version = "1.0"
+version = "1.1"
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -5,4 +5,6 @@
+pub fn multiply(a: i32, b: i32) -> i32 {
+    a * b
 }
"#;

    #[test]
    fn split_correct_count() {
        let chunks = split_diff_by_file(SAMPLE);
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn priority_excludes_lock() {
        assert!(calculate_priority("Cargo.lock") < 0);
    }

    #[test]
    fn alg_from_num_default() {
        assert_eq!(DiffAlg::from_num(99), DiffAlg::Semantic);
    }

    #[test]
    fn files_excludes_lock() {
        let r = shape_diff(SAMPLE, None, 10_000, DiffAlg::Files);
        assert!(!r.content.contains("Cargo.lock"));
    }

    #[test]
    fn semantic_produces_json() {
        let r = shape_diff(SAMPLE, None, 10_000, DiffAlg::Semantic);
        assert!(r.content.starts_with('{') && r.content.ends_with('}'));
    }
}