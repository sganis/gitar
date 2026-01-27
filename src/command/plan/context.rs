// src/command/plan/context.rs
use super::analyze::{AnalysisResult, ChangeStatus, FileChange};
use super::model::{ChangeSignals, FileInfo, FileKind, PlanConstraints, PlanningContext};
use crate::git;
use crate::util::FileCategory;
use anyhow::Result;
use std::collections::HashSet;

// =============================================================================
// PLANNING CONTEXT BUILDER
// =============================================================================

/// Build a PlanningContext from AnalysisResult
pub fn build_planning_context(analysis: &AnalysisResult) -> PlanningContext {
    let files = build_file_infos(&analysis.files);
    let signals = detect_signals(&analysis.files, &files);
    let constraints = PlanConstraints::default();

    PlanningContext::new(files)
        .with_signals(signals)
        .with_constraints(constraints)
}

/// Convert FileChanges to FileInfos with metadata
fn build_file_infos(changes: &[FileChange]) -> Vec<FileInfo> {
    changes
        .iter()
        .map(|change| {
            let kind = category_to_kind(&change.category);
            let churn = change.additions + change.deletions;
            let renamed = matches!(change.status, ChangeStatus::Renamed { .. });
            let test = change.category == FileCategory::Tests;
            let doc = change.category == FileCategory::Documentation;
            let config = change.category == FileCategory::Config;
            let large_file = change.is_large_file();

            FileInfo::new(change.path.clone())
                .with_kind(kind)
                .with_churn(churn)
                .with_renamed(renamed)
                .with_test(test)
                .with_doc(doc)
                .with_config(config)
                .with_large_file(large_file)
        })
        .collect()
}

/// Convert FileCategory to FileKind
fn category_to_kind(category: &FileCategory) -> FileKind {
    match category {
        FileCategory::Code => FileKind::Code,
        FileCategory::Tests => FileKind::Test,
        FileCategory::Documentation => FileKind::Doc,
        FileCategory::Config => FileKind::Config,
        FileCategory::Formatting => FileKind::Format,
        FileCategory::Rename => FileKind::Code, // Renames are code by default
    }
}

// =============================================================================
// SIGNAL DETECTION
// =============================================================================

/// Detect signals from file changes
fn detect_signals(changes: &[FileChange], files: &[FileInfo]) -> ChangeSignals {
    let mut signals = ChangeSignals::default();

    // Count categories
    let rename_count = changes
        .iter()
        .filter(|c| matches!(c.status, ChangeStatus::Renamed { .. }))
        .count();
    let doc_count = files.iter().filter(|f| f.doc).count();
    let test_count = files.iter().filter(|f| f.test).count();
    let code_count = files
        .iter()
        .filter(|f| f.kind == FileKind::Code && !f.renamed)
        .count();
    let _config_count = files.iter().filter(|f| f.config).count();
    let large_file_count = files.iter().filter(|f| f.large_file).count();

    // Large rename set: more than 3 renames or >30% of files
    signals.large_rename_set =
        rename_count > 3 || (changes.len() > 0 && rename_count * 100 / changes.len() > 30);

    // Docs only: all files are documentation
    signals.docs_only = doc_count == files.len() && files.len() > 0;

    // Touches tests and src: has both test and code changes
    signals.touches_tests_and_src = test_count > 0 && code_count > 0;

    // Single module: all files in same top-level directory
    signals.single_module = is_single_module(changes);

    // Dependency update: has Cargo.toml, package.json, etc.
    signals.dependency_update = has_dependency_changes(changes);

    // Mostly whitespace: check if changes look like formatting
    signals.mostly_whitespace =
        detect_formatting_only(changes, files.iter().map(|f| f.churn).sum());

    // Has large files: any file >= 50 MB
    signals.has_large_files = large_file_count > 0;

    signals
}

/// Check if all files are in the same top-level directory
fn is_single_module(changes: &[FileChange]) -> bool {
    if changes.is_empty() {
        return true;
    }

    let mut dirs: HashSet<&str> = HashSet::new();
    for change in changes {
        let dir = if let Some(idx) = change.path.find('/') {
            &change.path[..idx]
        } else {
            "root"
        };
        dirs.insert(dir);
    }

    dirs.len() == 1
}

/// Check if changes include dependency files
fn has_dependency_changes(changes: &[FileChange]) -> bool {
    const DEP_FILES: &[&str] = &[
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "Pipfile",
        "requirements.txt",
        "go.mod",
        "Gemfile",
        "composer.json",
    ];

    changes.iter().any(|c| {
        let filename = c.path.rsplit('/').next().unwrap_or(&c.path);
        DEP_FILES.iter().any(|dep| filename == *dep)
    })
}

/// Detect if changes are mostly formatting (heuristic)
fn detect_formatting_only(changes: &[FileChange], total_churn: usize) -> bool {
    if changes.is_empty() || total_churn == 0 {
        return false;
    }

    // If total churn is small and many files, likely formatting
    let avg_churn_per_file = total_churn / changes.len();
    if avg_churn_per_file < 5 && changes.len() > 3 {
        return true;
    }

    // Check for formatting-related commit patterns (optional: git log check)
    false
}

// =============================================================================
// WHITESPACE DETECTION
// =============================================================================

/// Analyze a diff to detect if it's mostly whitespace changes
#[allow(dead_code)]
pub fn is_whitespace_only_diff(diff: &str) -> bool {
    if diff.trim().is_empty() {
        return false;
    }

    let mut content_changes = 0;
    let mut whitespace_changes = 0;

    for line in diff.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            let content = &line[1..];
            if is_whitespace_change(content) {
                whitespace_changes += 1;
            } else {
                content_changes += 1;
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            let content = &line[1..];
            if is_whitespace_change(content) {
                whitespace_changes += 1;
            } else {
                content_changes += 1;
            }
        }
    }

    let total = content_changes + whitespace_changes;
    if total == 0 {
        return false;
    }

    // More than 90% whitespace changes
    whitespace_changes * 100 / total > 90
}

/// Check if a line change is whitespace-only
#[allow(dead_code)]
fn is_whitespace_change(line: &str) -> bool {
    // Empty line or whitespace-only
    if line.trim().is_empty() {
        return true;
    }

    // Line with only trailing whitespace changes would appear as identical after trim
    // This is a simplified heuristic
    false
}

// =============================================================================
// FORMATTING FILE DETECTION
// =============================================================================

/// Detect if a file is likely a formatting-only change
#[allow(dead_code)]
pub fn detect_formatting_file(path: &str) -> Result<bool> {
    // Get the diff for this specific file
    let diff = git::run_git_optional(&["diff", "--", path])?;
    if let Some(diff_content) = diff {
        return Ok(is_whitespace_only_diff(&diff_content));
    }

    // Check staged diff
    let staged_diff = git::run_git_optional(&["diff", "--cached", "--", path])?;
    if let Some(diff_content) = staged_diff {
        return Ok(is_whitespace_only_diff(&diff_content));
    }

    Ok(false)
}

// =============================================================================
// COMPACT JSON OUTPUT
// =============================================================================

/// Convert PlanningContext to compact JSON for LLM
pub fn context_to_json(context: &PlanningContext) -> String {
    serde_json::to_string_pretty(context).unwrap_or_else(|_| "{}".to_string())
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::plan::analyze::ChangeStatus;

    fn make_file_change(path: &str, category: FileCategory) -> FileChange {
        FileChange {
            path: path.to_string(),
            status: ChangeStatus::Modified,
            additions: 10,
            deletions: 5,
            category,
            size_bytes: Some(1000),
        }
    }

    fn make_renamed_change(path: &str, from: &str) -> FileChange {
        FileChange {
            path: path.to_string(),
            status: ChangeStatus::Renamed {
                from: from.to_string(),
            },
            additions: 0,
            deletions: 0,
            category: FileCategory::Code,
            size_bytes: Some(1000),
        }
    }

    fn make_large_file_change(path: &str) -> FileChange {
        FileChange {
            path: path.to_string(),
            status: ChangeStatus::Added,
            additions: 0,
            deletions: 0,
            category: FileCategory::Code,
            size_bytes: Some(100 * 1024 * 1024), // 100 MB
        }
    }

    #[test]
    fn build_file_infos_basic() {
        let changes = vec![
            make_file_change("src/main.rs", FileCategory::Code),
            make_file_change("README.md", FileCategory::Documentation),
        ];

        let infos = build_file_infos(&changes);
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].kind, FileKind::Code);
        assert!(!infos[0].doc);
        assert_eq!(infos[1].kind, FileKind::Doc);
        assert!(infos[1].doc);
    }

    #[test]
    fn detect_signals_docs_only() {
        let changes = vec![
            make_file_change("README.md", FileCategory::Documentation),
            make_file_change("docs/guide.md", FileCategory::Documentation),
        ];
        let infos = build_file_infos(&changes);
        let signals = detect_signals(&changes, &infos);

        assert!(signals.docs_only);
        assert!(!signals.touches_tests_and_src);
    }

    #[test]
    fn detect_signals_tests_and_src() {
        let changes = vec![
            make_file_change("src/main.rs", FileCategory::Code),
            make_file_change("tests/main_test.rs", FileCategory::Tests),
        ];
        let infos = build_file_infos(&changes);
        let signals = detect_signals(&changes, &infos);

        assert!(signals.touches_tests_and_src);
        assert!(!signals.docs_only);
    }

    #[test]
    fn detect_signals_large_rename_set() {
        let changes = vec![
            make_renamed_change("new1.rs", "old1.rs"),
            make_renamed_change("new2.rs", "old2.rs"),
            make_renamed_change("new3.rs", "old3.rs"),
            make_renamed_change("new4.rs", "old4.rs"),
        ];
        let infos = build_file_infos(&changes);
        let signals = detect_signals(&changes, &infos);

        assert!(signals.large_rename_set);
    }

    #[test]
    fn detect_signals_single_module() {
        let changes = vec![
            make_file_change("src/main.rs", FileCategory::Code),
            make_file_change("src/lib.rs", FileCategory::Code),
            make_file_change("src/util.rs", FileCategory::Code),
        ];
        let infos = build_file_infos(&changes);
        let signals = detect_signals(&changes, &infos);

        assert!(signals.single_module);
    }

    #[test]
    fn detect_signals_multi_module() {
        let changes = vec![
            make_file_change("src/main.rs", FileCategory::Code),
            make_file_change("tests/main_test.rs", FileCategory::Tests),
        ];
        let infos = build_file_infos(&changes);
        let signals = detect_signals(&changes, &infos);

        assert!(!signals.single_module);
    }

    #[test]
    fn is_whitespace_only_diff_true() {
        let diff = r#"
diff --git a/file.rs b/file.rs
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,3 @@
 fn main() {
-
+
 }
"#;
        assert!(is_whitespace_only_diff(diff));
    }

    #[test]
    fn is_whitespace_only_diff_false() {
        let diff = r#"
diff --git a/file.rs b/file.rs
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("Hello");
 }
"#;
        assert!(!is_whitespace_only_diff(diff));
    }

    #[test]
    fn category_to_kind_conversion() {
        assert_eq!(category_to_kind(&FileCategory::Code), FileKind::Code);
        assert_eq!(category_to_kind(&FileCategory::Tests), FileKind::Test);
        assert_eq!(
            category_to_kind(&FileCategory::Documentation),
            FileKind::Doc
        );
        assert_eq!(category_to_kind(&FileCategory::Config), FileKind::Config);
        assert_eq!(
            category_to_kind(&FileCategory::Formatting),
            FileKind::Format
        );
    }

    #[test]
    fn detect_signals_large_files() {
        let changes = vec![
            make_file_change("src/main.rs", FileCategory::Code),
            make_large_file_change("assets/video.mp4"),
        ];
        let infos = build_file_infos(&changes);
        let signals = detect_signals(&changes, &infos);

        assert!(signals.has_large_files);
        assert!(infos[1].large_file);
        assert!(!infos[0].large_file);
    }

    #[test]
    fn detect_signals_no_large_files() {
        let changes = vec![
            make_file_change("src/main.rs", FileCategory::Code),
            make_file_change("src/lib.rs", FileCategory::Code),
        ];
        let infos = build_file_infos(&changes);
        let signals = detect_signals(&changes, &infos);

        assert!(!signals.has_large_files);
    }

    #[test]
    fn context_to_json_serializes() {
        let context = PlanningContext::new(vec![FileInfo::new("test.rs".to_string())]);
        let json = context_to_json(&context);
        assert!(json.contains("test.rs"));
        assert!(json.contains("files"));
    }
}
