// src/command/plan/group.rs
use anyhow::Result;
use std::collections::HashMap;

use crate::client::LlmClient;
use crate::command::apply_smart_diff;
use crate::context::load_all_context;
use crate::git;
use crate::prompt::secret::SecretAction;
use crate::prompt::template::{commit_system_with_context, COMMIT_USER};
use crate::prompt::Preset;

use super::analyze::{AnalysisResult, FileChange, FileCategory, ChangeStatus};

// =============================================================================
// DATA STRUCTURES
// =============================================================================

#[derive(Debug, Clone)]
pub struct CommitGroup {
    #[allow(dead_code)]
    pub id: usize,
    pub title: String,        // Short label
    pub message: String,      // Full commit message
    pub files: Vec<String>,   // File paths
    #[allow(dead_code)]
    pub estimated_tokens: usize,
}

// =============================================================================
// PUBLIC API: CREATE GROUPS
// =============================================================================

/// Create commit groups from analysis result
pub async fn create_groups(
    analysis: &AnalysisResult,
    client: &LlmClient,
    preset: Preset,
    algo: u8,
    max_chars: usize,
    secret_action: SecretAction,
) -> Result<Vec<CommitGroup>> {
    // Load contexts once (project + user)
    let (project_ctx, user_ctx) = load_all_context();

    // Step 1: Heuristic grouping
    let initial_groups = group_by_heuristics(&analysis.files);

    if initial_groups.is_empty() {
        return Ok(vec![]);
    }

    // Step 2: Generate commit message for each group using LLM
    let mut commit_groups = Vec::new();

    for (idx, group) in initial_groups.into_iter().enumerate() {
        let files: Vec<String> = group.iter().map(|c| c.path.clone()).collect();

        // Get diff for these files
        let diff_output = get_diff_for_files(&files, &analysis.mode)?;

        // Apply diff algorithm + secret detection
        let shaped_diff = apply_smart_diff(
            &diff_output,
            max_chars,
            true, // silent
            algo,
            secret_action,
        )?;

        // Use LLM to generate commit message
        let system_prompt = commit_system_with_context(preset, project_ctx.as_deref(), user_ctx.as_deref());
        let user_prompt = COMMIT_USER.replace("{diff}", &shaped_diff);

        let message = match client.chat(&system_prompt, &user_prompt, false).await {
            Ok(msg) => msg.trim().to_string(),
            Err(e) => {
                eprintln!("Warning: LLM error for group {}: {}", idx + 1, e);
                format!("Update {} files", files.len())
            }
        };

        // Estimate tokens (rough: chars / 3.5)
        let estimated_tokens = shaped_diff.len() / 4;

        let title = if message.len() > 60 {
            format!("{}...", &message[..57])
        } else {
            message.clone()
        };

        commit_groups.push(CommitGroup {
            id: idx,
            title,
            message,
            files,
            estimated_tokens,
        });
    }

    Ok(commit_groups)
}

// =============================================================================
// HEURISTIC GROUPING
// =============================================================================

/// Group files by category (docs, tests, config, code by directory)
pub fn group_by_heuristics(files: &[FileChange]) -> Vec<Vec<FileChange>> {
    let mut groups: Vec<Vec<FileChange>> = Vec::new();

    // Group 1: Documentation
    let docs: Vec<FileChange> = files
        .iter()
        .filter(|c| c.category == FileCategory::Documentation)
        .cloned()
        .collect();
    if !docs.is_empty() {
        groups.push(docs);
    }

    // Group 2: Tests
    let tests: Vec<FileChange> = files
        .iter()
        .filter(|c| c.category == FileCategory::Tests)
        .cloned()
        .collect();
    if !tests.is_empty() {
        groups.push(tests);
    }

    // Group 3: Config
    let config: Vec<FileChange> = files
        .iter()
        .filter(|c| c.category == FileCategory::Config)
        .cloned()
        .collect();
    if !config.is_empty() {
        groups.push(config);
    }

    // Group 4: Renames
    let renames: Vec<FileChange> = files
        .iter()
        .filter(|c| matches!(c.status, ChangeStatus::Renamed { .. }))
        .cloned()
        .collect();
    if !renames.is_empty() {
        groups.push(renames);
    }

    // Group 5: Code changes (grouped by top-level directory)
    let code_changes: Vec<FileChange> = files
        .iter()
        .filter(|c| c.category == FileCategory::Code && !matches!(c.status, ChangeStatus::Renamed { .. }))
        .cloned()
        .collect();

    if !code_changes.is_empty() {
        let mut dir_groups: HashMap<String, Vec<FileChange>> = HashMap::new();

        for change in code_changes {
            let dir = if let Some(idx) = change.path.find('/') {
                change.path[..idx].to_string()
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

// =============================================================================
// DIFF HELPERS
// =============================================================================

use super::AnalysisMode;

/// Get diff for specific files based on analysis mode
fn get_diff_for_files(files: &[String], mode: &AnalysisMode) -> Result<String> {
    let diff = match mode {
        AnalysisMode::WorkingTree => {
            // Unstaged changes
            let mut args = vec!["diff"];
            args.extend(files.iter().map(|s| s.as_str()));
            git::run_git(&args)?
        }
        AnalysisMode::Staged => {
            // Staged changes
            let mut args = vec!["diff", "--cached"];
            args.extend(files.iter().map(|s| s.as_str()));
            git::run_git(&args)?
        }
        AnalysisMode::History { from, to } => {
            // History range
            let to_ref = to.as_deref().unwrap_or("HEAD");
            let mut args = vec!["diff", from.as_str(), to_ref];
            args.extend(files.iter().map(|s| s.as_str()));
            git::run_git(&args)?
        }
        AnalysisMode::Auto => String::new(),
    };

    Ok(diff)
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::plan::analyze::{FileChange, ChangeStatus, FileCategory};

    #[test]
    fn group_by_heuristics_separates_categories() {
        let files = vec![
            FileChange {
                path: "README.md".to_string(),
                status: ChangeStatus::Modified,
                additions: 10,
                deletions: 5,
                category: FileCategory::Documentation,
            },
            FileChange {
                path: "test.rs".to_string(),
                status: ChangeStatus::Modified,
                additions: 20,
                deletions: 10,
                category: FileCategory::Tests,
            },
            FileChange {
                path: "Cargo.toml".to_string(),
                status: ChangeStatus::Modified,
                additions: 5,
                deletions: 2,
                category: FileCategory::Config,
            },
            FileChange {
                path: "src/main.rs".to_string(),
                status: ChangeStatus::Modified,
                additions: 30,
                deletions: 15,
                category: FileCategory::Code,
            },
        ];

        let groups = group_by_heuristics(&files);

        // Should have 4 groups: docs, tests, config, code
        assert_eq!(groups.len(), 4);

        // First group should be documentation
        assert_eq!(groups[0].len(), 1);
        assert_eq!(groups[0][0].category, FileCategory::Documentation);

        // Second group should be tests
        assert_eq!(groups[1].len(), 1);
        assert_eq!(groups[1][0].category, FileCategory::Tests);

        // Third group should be config
        assert_eq!(groups[2].len(), 1);
        assert_eq!(groups[2][0].category, FileCategory::Config);

        // Fourth group should be code
        assert_eq!(groups[3].len(), 1);
        assert_eq!(groups[3][0].category, FileCategory::Code);
    }

    #[test]
    fn group_by_heuristics_groups_code_by_directory() {
        let files = vec![
            FileChange {
                path: "src/main.rs".to_string(),
                status: ChangeStatus::Modified,
                additions: 10,
                deletions: 5,
                category: FileCategory::Code,
            },
            FileChange {
                path: "src/lib.rs".to_string(),
                status: ChangeStatus::Modified,
                additions: 20,
                deletions: 10,
                category: FileCategory::Code,
            },
            FileChange {
                path: "tests/integration.rs".to_string(),
                status: ChangeStatus::Modified,
                additions: 15,
                deletions: 7,
                category: FileCategory::Code,
            },
        ];

        let groups = group_by_heuristics(&files);

        // Should have 2 groups: src and tests directories
        assert_eq!(groups.len(), 2);

        // Each group should have files from same directory
        for group in groups {
            let first_dir = if let Some(idx) = group[0].path.find('/') {
                &group[0].path[..idx]
            } else {
                "root"
            };

            for file in &group {
                let file_dir = if let Some(idx) = file.path.find('/') {
                    &file.path[..idx]
                } else {
                    "root"
                };
                assert_eq!(file_dir, first_dir);
            }
        }
    }

    #[test]
    fn group_by_heuristics_handles_renames() {
        let files = vec![
            FileChange {
                path: "new_name.rs".to_string(),
                status: ChangeStatus::Renamed {
                    from: "old_name.rs".to_string(),
                },
                additions: 0,
                deletions: 0,
                category: FileCategory::Code,
            },
            FileChange {
                path: "src/main.rs".to_string(),
                status: ChangeStatus::Modified,
                additions: 10,
                deletions: 5,
                category: FileCategory::Code,
            },
        ];

        let groups = group_by_heuristics(&files);

        // Should have 2 groups: renames and code
        assert_eq!(groups.len(), 2);

        // First group should be renames
        assert!(matches!(groups[0][0].status, ChangeStatus::Renamed { .. }));

        // Second group should be code
        assert_eq!(groups[1][0].status, ChangeStatus::Modified);
    }

    #[test]
    fn group_by_heuristics_empty_input() {
        let files: Vec<FileChange> = vec![];
        let groups = group_by_heuristics(&files);
        assert_eq!(groups.len(), 0);
    }

    #[test]
    fn commit_group_title_truncation() {
        let long_message = "This is a very long commit message that exceeds the sixty character limit and should be truncated";
        let group = CommitGroup {
            id: 0,
            title: if long_message.len() > 60 {
                format!("{}...", &long_message[..57])
            } else {
                long_message.to_string()
            },
            message: long_message.to_string(),
            files: vec!["file.rs".to_string()],
            estimated_tokens: 100,
        };

        assert_eq!(group.title.len(), 60);
        assert!(group.title.ends_with("..."));
    }

    #[test]
    fn commit_group_title_no_truncation() {
        let short_message = "Short message";
        let group = CommitGroup {
            id: 0,
            title: if short_message.len() > 60 {
                format!("{}...", &short_message[..57])
            } else {
                short_message.to_string()
            },
            message: short_message.to_string(),
            files: vec!["file.rs".to_string()],
            estimated_tokens: 50,
        };

        assert_eq!(group.title, short_message);
        assert!(!group.title.ends_with("..."));
    }
}
