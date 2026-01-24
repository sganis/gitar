// src/command/plan/analyze.rs
use crate::context::algo::{DiffAlg, DiffStats};
use crate::git;
pub use crate::util::FileCategory;
use crate::util::{categorize_file, Groupable};
use anyhow::Result;

// =============================================================================
// ANALYSIS MODE & RESULT
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisMode {
    Auto,        // Detect best mode from repo state
    WorkingTree, // Unstaged + untracked
    Staged,      // Only staged changes
    History {
        // Commit range
        from: String,
        to: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub mode: AnalysisMode,
    #[allow(dead_code)]
    pub diff: String, // Raw diff
    pub files: Vec<FileChange>, // Parsed changes
    #[allow(dead_code)]
    pub stats: DiffStats, // From diff algorithm (populated later)
}

// =============================================================================
// HELPERS
// =============================================================================

/// Create empty DiffStats for cases where no diff processing has occurred yet
fn empty_diff_stats() -> DiffStats {
    DiffStats {
        total_files: 0,
        included_files: 0,
        total_chars: 0,
        output_chars: 0,
        estimated_tokens: 0,
        truncated: false,
        algorithm: DiffAlg::Semantic,
        file_list: vec![],
    }
}

// =============================================================================
// FILE CHANGE STRUCTURES
// =============================================================================

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub status: ChangeStatus,
    #[allow(dead_code)]
    pub additions: usize,
    #[allow(dead_code)]
    pub deletions: usize,
    pub category: FileCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed { from: String },
}

impl Groupable for FileChange {
    fn category(&self) -> &FileCategory {
        &self.category
    }

    fn is_rename(&self) -> bool {
        matches!(self.status, ChangeStatus::Renamed { .. })
    }

    fn path(&self) -> &str {
        &self.path
    }
}

// =============================================================================
// PUBLIC API: DETECT AND ANALYZE
// =============================================================================

/// Detect repo state and analyze based on mode
pub fn detect_and_analyze(mode: Option<AnalysisMode>) -> Result<AnalysisResult> {
    let analysis_mode = match mode {
        Some(m) => m,
        None => detect_mode()?,
    };

    match &analysis_mode {
        AnalysisMode::Auto => {
            // Auto mode means clean tree or conflicts - return empty result
            Ok(AnalysisResult {
                mode: AnalysisMode::Auto,
                diff: String::new(),
                files: vec![],
                stats: empty_diff_stats(),
            })
        }
        AnalysisMode::WorkingTree => analyze_working_tree(),
        AnalysisMode::Staged => analyze_staged(),
        AnalysisMode::History { from, to } => analyze_history(from, to.as_deref()),
    }
}

/// Auto-detect best mode from repo state
fn detect_mode() -> Result<AnalysisMode> {
    // Check for conflicts first
    if git::has_conflicts()? {
        eprintln!("Warning: Conflicts detected. Run `gitar resolve` first.");
        return Ok(AnalysisMode::Auto); // Return Auto to signal conflict state
    }

    // Check for staged changes
    let staged_diff = git::run_git(&["diff", "--cached", "--name-only"])?;
    let has_staged = !staged_diff.trim().is_empty();

    // Check for unstaged changes
    let unstaged_diff = git::run_git(&["diff", "--name-only"])?;
    let has_unstaged = !unstaged_diff.trim().is_empty();

    // Check for untracked files
    let untracked = git::run_git(&["ls-files", "--others", "--exclude-standard"])?;
    let has_untracked = !untracked.trim().is_empty();

    // Decision logic
    if has_staged && (has_unstaged || has_untracked) {
        Ok(AnalysisMode::WorkingTree) // Mixed state, analyze working tree
    } else if has_staged {
        Ok(AnalysisMode::Staged) // Only staged, analyze staged
    } else if has_unstaged || has_untracked {
        Ok(AnalysisMode::WorkingTree) // Only unstaged/untracked
    } else {
        Ok(AnalysisMode::Auto) // Clean working tree, signal to user
    }
}

// =============================================================================
// WORKING TREE ANALYSIS
// =============================================================================

/// Analyze unstaged + untracked changes
pub fn analyze_working_tree() -> Result<AnalysisResult> {
    let files = scan_working_tree()?;

    if files.is_empty() {
        return Ok(AnalysisResult {
            mode: AnalysisMode::WorkingTree,
            diff: String::new(),
            files: vec![],
            stats: empty_diff_stats(),
        });
    }

    // Get raw diff
    let diff = git::run_git(&["diff"])?;

    Ok(AnalysisResult {
        mode: AnalysisMode::WorkingTree,
        diff,
        files,
        stats: empty_diff_stats(), // Will be populated by diff algorithm later
    })
}

/// Scan unstaged and untracked files (extracted from split.rs)
fn scan_working_tree() -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();

    // Get numstat for modified files
    let numstat = git::run_git(&["diff", "--numstat"])?;

    if !numstat.trim().is_empty() {
        for line in numstat.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            let additions = parts[0].parse().unwrap_or(0);
            let deletions = parts[1].parse().unwrap_or(0);
            let path = parts[2..].join(" ");

            let category = categorize_file(&path);

            changes.push(FileChange {
                path: path.clone(),
                status: ChangeStatus::Modified,
                additions,
                deletions,
                category,
            });
        }
    }

    // Get status for added, deleted, renamed files
    let status = git::run_git(&["status", "--porcelain"])?;
    for line in status.lines() {
        if line.len() < 4 {
            continue;
        }
        let status_code = &line[..2];
        let path = line[3..].to_string();

        match status_code {
            "??" => {
                // Untracked file
                if !changes.iter().any(|c| c.path == path) {
                    changes.push(FileChange {
                        path: path.clone(),
                        status: ChangeStatus::Added,
                        additions: 0,
                        deletions: 0,
                        category: categorize_file(&path),
                    });
                }
            }
            "R " | "RM" => {
                // Renamed file
                if let Some(change) = changes.iter_mut().find(|c| c.path == path) {
                    change.status = ChangeStatus::Renamed { from: path.clone() };
                }
            }
            " D" | "D " => {
                // Deleted file
                if let Some(change) = changes.iter_mut().find(|c| c.path == path) {
                    change.status = ChangeStatus::Deleted;
                }
            }
            _ => {}
        }
    }

    Ok(changes)
}

// =============================================================================
// STAGED ANALYSIS
// =============================================================================

/// Analyze only staged changes
pub fn analyze_staged() -> Result<AnalysisResult> {
    let files = scan_staged()?;

    if files.is_empty() {
        return Ok(AnalysisResult {
            mode: AnalysisMode::Staged,
            diff: String::new(),
            files: vec![],
            stats: empty_diff_stats(),
        });
    }

    // Get raw diff of staged changes
    let diff = git::run_git(&["diff", "--cached"])?;

    Ok(AnalysisResult {
        mode: AnalysisMode::Staged,
        diff,
        files,
        stats: empty_diff_stats(), // Will be populated by diff algorithm later
    })
}

/// Scan staged files
fn scan_staged() -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();

    // Get numstat for staged files
    let numstat = git::run_git(&["diff", "--cached", "--numstat"])?;

    if !numstat.trim().is_empty() {
        for line in numstat.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            let additions = parts[0].parse().unwrap_or(0);
            let deletions = parts[1].parse().unwrap_or(0);
            let path = parts[2..].join(" ");

            let category = categorize_file(&path);

            changes.push(FileChange {
                path: path.clone(),
                status: ChangeStatus::Modified, // Default to modified
                additions,
                deletions,
                category,
            });
        }
    }

    // Get status for staged additions and deletions
    let status = git::run_git(&["diff", "--cached", "--name-status"])?;
    for line in status.lines() {
        if line.len() < 3 {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let status_code = parts[0];
        let path = parts[1..].join(" ");

        match status_code {
            "A" => {
                // Added file
                if let Some(change) = changes.iter_mut().find(|c| c.path == path) {
                    change.status = ChangeStatus::Added;
                }
            }
            "D" => {
                // Deleted file
                if let Some(change) = changes.iter_mut().find(|c| c.path == path) {
                    change.status = ChangeStatus::Deleted;
                }
            }
            s if s.starts_with('R') => {
                // Renamed file
                if parts.len() >= 3 {
                    let from = parts[1].to_string();
                    let to = parts[2].to_string();
                    if let Some(change) = changes.iter_mut().find(|c| c.path == to) {
                        change.status = ChangeStatus::Renamed { from };
                    }
                }
            }
            _ => {}
        }
    }

    Ok(changes)
}

// =============================================================================
// HISTORY ANALYSIS
// =============================================================================

/// Analyze commit history (placeholder for Phase 6)
pub fn analyze_history(from: &str, to: Option<&str>) -> Result<AnalysisResult> {
    let to_ref = to.unwrap_or("HEAD");

    // Get cumulative diff
    let diff = git::run_git(&["diff", from, to_ref])?;

    // Parse file changes from diff
    let files = parse_diff_files(from, to_ref)?;

    Ok(AnalysisResult {
        mode: AnalysisMode::History {
            from: from.to_string(),
            to: to.map(ToString::to_string),
        },
        diff,
        files,
        stats: empty_diff_stats(),
    })
}

/// Parse file changes from a diff
fn parse_diff_files(from: &str, to: &str) -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();

    let numstat = git::run_git(&["diff", "--numstat", from, to])?;

    for line in numstat.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }

        let additions = parts[0].parse().unwrap_or(0);
        let deletions = parts[1].parse().unwrap_or(0);
        let path = parts[2..].join(" ");

        let category = categorize_file(&path);

        changes.push(FileChange {
            path: path.clone(),
            status: ChangeStatus::Modified,
            additions,
            deletions,
            category,
        });
    }

    Ok(changes)
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorize_file_documentation() {
        assert_eq!(categorize_file("README.md"), FileCategory::Documentation);
        assert_eq!(categorize_file("CHANGELOG.md"), FileCategory::Documentation);
        assert_eq!(
            categorize_file("docs/guide.md"),
            FileCategory::Documentation
        );
        assert_eq!(categorize_file("notes.txt"), FileCategory::Documentation);
    }

    #[test]
    fn categorize_file_tests() {
        assert_eq!(categorize_file("test_main.rs"), FileCategory::Tests);
        assert_eq!(categorize_file("main_test.rs"), FileCategory::Tests);
        assert_eq!(categorize_file("app.test.ts"), FileCategory::Tests);
        assert_eq!(categorize_file("app.spec.js"), FileCategory::Tests);
        assert_eq!(categorize_file("__tests__/app.js"), FileCategory::Tests);
    }

    #[test]
    fn categorize_file_config() {
        assert_eq!(categorize_file("Cargo.toml"), FileCategory::Config);
        assert_eq!(categorize_file("config.yaml"), FileCategory::Config);
        assert_eq!(categorize_file("package.json"), FileCategory::Config);
        assert_eq!(categorize_file("Dockerfile"), FileCategory::Config);
        assert_eq!(
            categorize_file(".github/workflows/ci.yml"),
            FileCategory::Config
        );
    }

    #[test]
    fn categorize_file_code() {
        assert_eq!(categorize_file("main.rs"), FileCategory::Code);
        assert_eq!(categorize_file("app.ts"), FileCategory::Code);
        assert_eq!(categorize_file("utils.py"), FileCategory::Code);
    }

    #[test]
    fn analysis_mode_equality() {
        assert_eq!(AnalysisMode::Auto, AnalysisMode::Auto);
        assert_eq!(AnalysisMode::WorkingTree, AnalysisMode::WorkingTree);
        assert_eq!(AnalysisMode::Staged, AnalysisMode::Staged);

        let history1 = AnalysisMode::History {
            from: "v1.0.0".to_string(),
            to: None,
        };
        let history2 = AnalysisMode::History {
            from: "v1.0.0".to_string(),
            to: None,
        };
        assert_eq!(history1, history2);
    }

    #[test]
    fn change_status_variants() {
        let added = ChangeStatus::Added;
        let modified = ChangeStatus::Modified;
        let deleted = ChangeStatus::Deleted;
        let renamed = ChangeStatus::Renamed {
            from: "old.rs".to_string(),
        };

        assert_eq!(added, ChangeStatus::Added);
        assert_eq!(modified, ChangeStatus::Modified);
        assert_eq!(deleted, ChangeStatus::Deleted);
        assert_eq!(
            renamed,
            ChangeStatus::Renamed {
                from: "old.rs".to_string()
            }
        );
    }

    #[test]
    fn file_change_creation() {
        let change = FileChange {
            path: "src/main.rs".to_string(),
            status: ChangeStatus::Modified,
            additions: 10,
            deletions: 5,
            category: FileCategory::Code,
        };

        assert_eq!(change.path, "src/main.rs");
        assert_eq!(change.additions, 10);
        assert_eq!(change.deletions, 5);
        assert_eq!(change.category, FileCategory::Code);
    }
}
