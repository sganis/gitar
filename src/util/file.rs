// src/util/file.rs - Shared file categorization and grouping utilities

use std::collections::HashMap;

// =============================================================================
// FILE CATEGORIZATION
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileCategory {
    Documentation,
    Tests,
    Config,
    #[allow(dead_code)]
    Formatting,
    #[allow(dead_code)]
    Rename,
    Code,
}

/// Categorize file by path (extracted from split.rs and analyze.rs)
pub fn categorize_file(path: &str) -> FileCategory {
    let lower = path.to_lowercase();

    // Documentation
    if lower.ends_with(".md")
        || lower.ends_with(".txt")
        || lower.ends_with(".rst")
        || lower.contains("readme")
        || lower.contains("changelog")
        || lower.contains("/docs/")
        || lower.contains("/doc/")
    {
        return FileCategory::Documentation;
    }

    // Tests
    if lower.contains("test")
        || lower.contains("spec")
        || lower.contains("__tests__")
        || lower.ends_with("_test.rs")
        || lower.ends_with("_test.go")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".test.js")
        || lower.ends_with(".spec.ts")
        || lower.ends_with(".spec.js")
    {
        return FileCategory::Tests;
    }

    // Config
    if lower.ends_with(".toml")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".json")
        || lower.ends_with(".lock")
        || lower.ends_with(".config.js")
        || lower.ends_with(".config.ts")
        || lower.ends_with("dockerfile")
        || lower.contains(".github/")
        || lower.contains(".vscode/")
        || lower == "makefile"
    {
        return FileCategory::Config;
    }

    // Default to code
    FileCategory::Code
}

// =============================================================================
// HEURISTIC GROUPING
// =============================================================================

/// Trait for types that can be grouped by heuristics
pub trait Groupable {
    fn category(&self) -> &FileCategory;
    fn is_rename(&self) -> bool;
    fn path(&self) -> &str;
}

/// Group files by category (docs, tests, config, code by directory)
pub fn group_by_heuristics<T: Groupable + Clone>(files: &[T]) -> Vec<Vec<T>> {
    let mut groups: Vec<Vec<T>> = Vec::new();

    // Group 1: Documentation
    let docs: Vec<T> = files
        .iter()
        .filter(|c| c.category() == &FileCategory::Documentation)
        .cloned()
        .collect();
    if !docs.is_empty() {
        groups.push(docs);
    }

    // Group 2: Tests
    let tests: Vec<T> = files
        .iter()
        .filter(|c| c.category() == &FileCategory::Tests)
        .cloned()
        .collect();
    if !tests.is_empty() {
        groups.push(tests);
    }

    // Group 3: Config
    let config: Vec<T> = files
        .iter()
        .filter(|c| c.category() == &FileCategory::Config)
        .cloned()
        .collect();
    if !config.is_empty() {
        groups.push(config);
    }

    // Group 4: Renames
    let renames: Vec<T> = files
        .iter()
        .filter(|c| c.is_rename())
        .cloned()
        .collect();
    if !renames.is_empty() {
        groups.push(renames);
    }

    // Group 5: Code changes (grouped by top-level directory)
    let code_changes: Vec<T> = files
        .iter()
        .filter(|c| c.category() == &FileCategory::Code && !c.is_rename())
        .cloned()
        .collect();

    if !code_changes.is_empty() {
        let mut dir_groups: HashMap<String, Vec<T>> = HashMap::new();

        for change in code_changes {
            let dir = if let Some(idx) = change.path().find('/') {
                change.path()[..idx].to_string()
            } else {
                "root".to_string()
            };
            dir_groups.entry(dir).or_default().push(change);
        }

        for (_dir, group) in dir_groups {
            groups.push(group);
        }
    }

    groups
}
