// src/context/algo/mod.rs - Diff shaping algorithms for LLM context optimization
//
// Pure transformation functions: raw diff in, shaped diff out.
// No I/O, no git calls, no secret scanning.
//
// Algorithms:
// 1 - Full:     Complete diff (truncate only)
// 2 - Files:    Selective files by priority
// 3 - Hunks:    Selective hunks by importance
// 4 - Semantic: JSON IR (token-efficient)

mod file;
mod full;
mod hunk;
mod semantic;

pub(crate) use file::alg_files;
pub(crate) use full::alg_full;
pub(crate) use hunk::alg_hunks;
pub(crate) use semantic::alg_semantic;

const CHARS_PER_TOKEN: f32 = 3.5;

pub(crate) const PRIORITY_SCORES: &[(&str, i32)] = &[
    ("main.rs", 100),
    ("lib.rs", 100),
    ("mod.rs", 80),
    (".rs", 70),
    (".py", 70),
    (".go", 65),
    (".ts", 65),
    (".js", 60),
    ("Cargo.toml", 50),
    ("pyproject.toml", 50),
    ("package.json", 45),
    ("README.md", 40),
    (".md", 30),
    (".toml", 30),
    (".yaml", 25),
    (".yml", 25),
    (".json", 15),
    (".css", 10),
    (".svg", 5),
];

pub(crate) const EXCLUDE_FILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "Pipfile.lock",
    ".gitignore",
    ".DS_Store",
];

pub(crate) const EXCLUDE_PATTERNS: &[&str] = &[
    "vendor/",
    "node_modules/",
    "target/",
    "dist/",
    "__pycache__/",
    ".min.js",
    ".min.css",
    "generated",
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
    pub(crate) fn new(alg: DiffAlg, total_files: usize, total_chars: usize) -> Self {
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

    pub(crate) fn finalize(&mut self, output: &str) {
        self.output_chars = output.len();
        self.estimated_tokens = (output.len() as f32 / CHARS_PER_TOKEN) as usize;
    }

    pub fn display(&self) -> String {
        let trunc = if self.truncated { " [truncated]" } else { "" };
        let files_str = if self.file_list.len() <= 5 {
            self.file_list.join(", ")
        } else {
            format!(
                "{}, ... +{} more",
                self.file_list[..3].join(", "),
                self.file_list.len() - 3
            )
        };

        format!(
            "Alg: {} | Files: {}/{} | Chars: {} -> {} | Tokens: ~{}{}\n  {}\n",
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

pub fn shape_diff(
    raw_diff: &str,
    diff_stat: Option<&str>,
    max_chars: usize,
    alg: DiffAlg,
) -> ShapedDiff {
    match alg {
        DiffAlg::Full => alg_full(raw_diff, diff_stat, max_chars),
        DiffAlg::Files => alg_files(raw_diff, diff_stat, max_chars),
        DiffAlg::Hunks => alg_hunks(raw_diff, diff_stat, max_chars),
        DiffAlg::Semantic => alg_semantic(raw_diff, diff_stat, max_chars),
    }
}

#[allow(dead_code)]
pub fn compare_algorithms(raw_diff: &str, max_chars: usize) -> String {
    let mut out = String::from("=== Algorithm Comparison ===\n\n");

    for alg in [
        DiffAlg::Full,
        DiffAlg::Files,
        DiffAlg::Hunks,
        DiffAlg::Semantic,
    ] {
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
// SHARED HELPERS
// =============================================================================

pub(crate) fn split_diff_by_file(raw_diff: &str) -> Vec<FileChunk> {
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
            if line.starts_with('+') && !line.starts_with("+++") {
                adds += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                dels += 1;
            }
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

pub(crate) fn calculate_priority(path: &str) -> i32 {
    for ex in EXCLUDE_FILES {
        if path.ends_with(ex) {
            return -100;
        }
    }
    for pat in EXCLUDE_PATTERNS {
        if path.contains(pat) {
            return -100;
        }
    }

    let mut best = 20;
    for (pat, score) in PRIORITY_SCORES {
        if (path.ends_with(pat) || path.contains(pat)) && *score > best {
            best = *score;
        }
    }
    best
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
