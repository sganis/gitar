// src/command/plan/group.rs
use anyhow::Result;
use std::path::PathBuf;

use crate::client::LlmClient;
use crate::command::{apply_smart_diff_with_context, AnalysisContext};
use crate::context::load_all_context;
use crate::context::secret::SecretAction;
use crate::context::template::{commit_system_with_context, get_commit_user};
use crate::context::Preset;
use crate::git;

use super::analyze::AnalysisResult;
use super::context::build_planning_context;
use super::model::PlanCandidate;
use super::prompt::{create_fallback_plan, get_plan_prompts, parse_plan_response};
use super::scoring::{format_score, select_best_candidate};

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// File status indicator for display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Unknown, // Untracked or any unrecognized status
}

impl FileStatus {
    /// Short label for display
    pub fn label(&self) -> &'static str {
        match self {
            FileStatus::Added => "A",
            FileStatus::Modified => "M",
            FileStatus::Deleted => "D",
            FileStatus::Renamed => "R",
            FileStatus::Unknown => "?",
        }
    }
}

/// A file with its status in a commit group
#[derive(Debug, Clone)]
pub struct FileWithStatus {
    pub path: String,
    pub status: FileStatus,
}

#[derive(Debug, Clone)]
pub struct CommitGroup {
    #[allow(dead_code)]
    pub id: usize,
    pub title: String,
    pub message: String,
    pub files: Vec<String>,
    pub files_with_status: Vec<FileWithStatus>,
    #[allow(dead_code)]
    pub estimated_tokens: usize,
    /// If true, this group contains large files (>= 50 MB) and should be ignored/unstaged
    pub is_large_file_group: bool,
}

// Lock files that should get default messages instead of LLM analysis
const LOCK_FILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "Pipfile.lock",
    "Gemfile.lock",
    "composer.lock",
    "go.sum",
];

// =============================================================================
// SKIP FILE HANDLING (Large files and Binary files)
// =============================================================================

use super::analyze::LARGE_FILE_THRESHOLD;

/// Create a commit group for large/binary files
/// This group appears in the plan but defaults to "skip" during execution
fn create_skip_file_group(
    skip_files: &[&super::analyze::FileChange],
    _mode: &AnalysisMode,
) -> CommitGroup {
    let file_count = skip_files.len();
    let has_large = skip_files.iter().any(|f| f.is_large_file());
    let has_binary = skip_files.iter().any(|f| f.is_binary_file());

    let title = if has_large && has_binary {
        format!("Binary/Large files ({} file{}) [default: SKIP]", file_count, if file_count > 1 { "s" } else { "" })
    } else if has_large {
        let threshold_mb = LARGE_FILE_THRESHOLD / (1024 * 1024);
        format!("Large files >= {}MB ({} file{}) [default: SKIP]", threshold_mb, file_count, if file_count > 1 { "s" } else { "" })
    } else {
        format!("Binary files ({} file{}) [default: SKIP]", file_count, if file_count > 1 { "s" } else { "" })
    };

    let message = "Add binary/large files\n\n\
        Note: These files are excluded from AI analysis.\n\
        Default action is SKIP. Choose 'commit' to include them."
        .to_string();

    let files: Vec<String> = skip_files.iter().map(|f| f.path.clone()).collect();

    let files_with_status: Vec<FileWithStatus> = skip_files
        .iter()
        .map(|f| {
            let status = match &f.status {
                super::analyze::ChangeStatus::Added => FileStatus::Added,
                super::analyze::ChangeStatus::Modified => FileStatus::Modified,
                super::analyze::ChangeStatus::Deleted => FileStatus::Deleted,
                super::analyze::ChangeStatus::Renamed { .. } => FileStatus::Renamed,
                super::analyze::ChangeStatus::Unknown => FileStatus::Unknown,
            };
            FileWithStatus {
                path: f.path.clone(),
                status,
            }
        })
        .collect();

    CommitGroup {
        id: usize::MAX, // Will be reassigned
        title,
        message,
        files,
        files_with_status,
        estimated_tokens: 0,
        is_large_file_group: true,
    }
}

// =============================================================================
// PUBLIC API: CREATE GROUPS
// =============================================================================

/// Create commit groups from analysis result using improved planning pipeline
pub async fn create_groups(
    analysis: &AnalysisResult,
    client: &LlmClient,
    preset: Preset,
    algo: u8,
    max_chars: usize,
    secret_action: SecretAction,
) -> Result<Vec<CommitGroup>> {
    // Build planning context
    let planning_context = build_planning_context(analysis);

    if planning_context.files.is_empty() {
        return Ok(vec![]);
    }

    // Separate files to skip (large or binary) from regular files
    let skip_files: Vec<_> = analysis
        .files
        .iter()
        .filter(|f| f.should_skip_llm())
        .collect();

    // Create skip file group if any exist
    let skip_file_group = if !skip_files.is_empty() {
        Some(create_skip_file_group(&skip_files, &analysis.mode))
    } else {
        None
    };

    // Filter out skip files from planning context for LLM
    let filtered_context = if !skip_files.is_empty() {
        let filtered_files: Vec<_> = planning_context
            .files
            .iter()
            .filter(|f| !f.should_skip_llm())
            .cloned()
            .collect();
        super::model::PlanningContext::new(filtered_files)
            .with_signals(planning_context.signals.clone())
            .with_constraints(planning_context.constraints.clone())
    } else {
        planning_context.clone()
    };

    // If only skip files, return just the skip file group
    if filtered_context.files.is_empty() {
        return Ok(skip_file_group.into_iter().collect());
    }

    // Helper to append skip file group and return
    let finalize_groups = |mut groups: Vec<CommitGroup>| -> Vec<CommitGroup> {
        if let Some(sfg) = skip_file_group.clone() {
            groups.push(sfg);
        }
        groups
    };

    // Get prompts for LLM (using filtered context without large files)
    let (system_prompt, user_prompt) = get_plan_prompts(&filtered_context, preset, None);

    // Call LLM for multi-candidate response
    let response = match client.chat(&system_prompt, &user_prompt, false).await {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!(
                "Warning: LLM grouping failed: {}. Using heuristic fallback.",
                e
            );
            let groups = create_groups_from_fallback(
                &filtered_context,
                analysis,
                client,
                preset,
                algo,
                max_chars,
                secret_action,
            )
            .await?;
            return Ok(finalize_groups(groups));
        }
    };

    // Parse response
    let plan_response = match parse_plan_response(&response) {
        Ok(pr) => pr,
        Err(e) => {
            eprintln!(
                "Warning: Failed to parse LLM response: {}. Using heuristic fallback.",
                e
            );
            let groups = create_groups_from_fallback(
                &filtered_context,
                analysis,
                client,
                preset,
                algo,
                max_chars,
                secret_action,
            )
            .await?;
            return Ok(finalize_groups(groups));
        }
    };

    if plan_response.candidates.is_empty() {
        eprintln!("Warning: No candidates in LLM response. Using heuristic fallback.");
        let groups = create_groups_from_fallback(
            &filtered_context,
            analysis,
            client,
            preset,
            algo,
            max_chars,
            secret_action,
        )
        .await?;
        return Ok(finalize_groups(groups));
    }

    // Score and select best candidate (using filtered_context to avoid penalizing for large files)
    let (best_idx, best_score) =
        match select_best_candidate(&plan_response.candidates, &filtered_context) {
            Some(result) => result,
            None => {
                eprintln!("Warning: Could not select candidate. Using heuristic fallback.");
                let groups = create_groups_from_fallback(
                    &filtered_context,
                    analysis,
                    client,
                    preset,
                    algo,
                    max_chars,
                    secret_action,
                )
                .await?;
                return Ok(finalize_groups(groups));
            }
        };

    let best_candidate = &plan_response.candidates[best_idx];

    // Display score info
    println!("\n{}", format_score(&best_score));
    if plan_response.candidates.len() > 1 {
        println!(
            "Selected candidate {} of {} (score: {})",
            best_idx + 1,
            plan_response.candidates.len(),
            best_score.total
        );
    }

    // Convert PlanGroups to CommitGroups with LLM-generated messages
    let commit_groups = convert_to_commit_groups(
        best_candidate,
        analysis,
        client,
        preset,
        algo,
        max_chars,
        secret_action,
    )
    .await?;

    Ok(finalize_groups(commit_groups))
}

// =============================================================================
// FALLBACK: HEURISTIC GROUPING
// =============================================================================

/// Create groups using heuristic fallback (no LLM grouping)
/// Uses filtered_context (without large files) to ensure large files are handled separately
async fn create_groups_from_fallback(
    filtered_context: &super::model::PlanningContext,
    analysis: &AnalysisResult,
    client: &LlmClient,
    preset: Preset,
    algo: u8,
    max_chars: usize,
    secret_action: SecretAction,
) -> Result<Vec<CommitGroup>> {
    let fallback = create_fallback_plan(filtered_context);

    if fallback.candidates.is_empty() || fallback.candidates[0].groups.is_empty() {
        return Ok(vec![]);
    }

    convert_to_commit_groups(
        &fallback.candidates[0],
        analysis,
        client,
        preset,
        algo,
        max_chars,
        secret_action,
    )
    .await
}

// =============================================================================
// CONVERT PLAN TO COMMIT GROUPS
// =============================================================================

/// Convert PlanCandidate groups to CommitGroups with generated messages
async fn convert_to_commit_groups(
    candidate: &PlanCandidate,
    analysis: &AnalysisResult,
    client: &LlmClient,
    preset: Preset,
    algo: u8,
    max_chars: usize,
    secret_action: SecretAction,
) -> Result<Vec<CommitGroup>> {
    let (project_ctx, user_ctx) = load_all_context();
    let context = AnalysisContext::new()
        .with_provider(client.provider())
        .with_model(client.model());

    let mut commit_groups = Vec::new();

    for (idx, plan_group) in candidate.groups.iter().enumerate() {
        let files = plan_group.files.clone();

        // Check if this group contains only lock files
        let (message, estimated_tokens) = if is_lock_file_group(&files) {
            (get_lock_file_message(&files), 0)
        } else {
            // Get diff for this group's files
            let group_changes: Vec<_> = analysis
                .files
                .iter()
                .filter(|f| files.contains(&f.path))
                .cloned()
                .collect();

            if group_changes.is_empty() {
                // Use the plan group's title as fallback
                (plan_group.title.clone(), 0)
            } else {
                let diff_output = get_diff_for_group(&group_changes, &analysis.mode)?;

                // Apply diff algorithm + secret detection
                let shaped_diff = apply_smart_diff_with_context(
                    &diff_output,
                    max_chars,
                    false,
                    algo,
                    Some(&context),
                    secret_action,
                )?;

                // Use LLM to generate commit message
                let system_prompt =
                    commit_system_with_context(preset, project_ctx.as_deref(), user_ctx.as_deref());
                let user_prompt = get_commit_user().replace("{diff}", &shaped_diff);

                let msg = match client.chat(&system_prompt, &user_prompt, false).await {
                    Ok(m) => m.trim().to_string(),
                    Err(e) => {
                        eprintln!("Warning: LLM error for group {}: {}", idx + 1, e);
                        // Fallback to plan group title
                        plan_group.title.clone()
                    }
                };

                let tokens = shaped_diff.len() / 4;
                (msg, tokens)
            }
        };

        let title = truncate_title(&message);

        // Build files with status from analysis
        let files_with_status: Vec<FileWithStatus> = files
            .iter()
            .map(|path| {
                let status = analysis
                    .files
                    .iter()
                    .find(|f| &f.path == path)
                    .map(|f| change_status_to_file_status(&f.status))
                    .unwrap_or(FileStatus::Modified);
                FileWithStatus {
                    path: path.clone(),
                    status,
                }
            })
            .collect();

        commit_groups.push(CommitGroup {
            id: idx,
            title,
            message,
            files,
            files_with_status,
            estimated_tokens,
            is_large_file_group: false,
        });
    }

    Ok(commit_groups)
}

/// Convert ChangeStatus to FileStatus
fn change_status_to_file_status(status: &ChangeStatus) -> FileStatus {
    match status {
        ChangeStatus::Added => FileStatus::Added,
        ChangeStatus::Modified => FileStatus::Modified,
        ChangeStatus::Deleted => FileStatus::Deleted,
        ChangeStatus::Renamed { .. } => FileStatus::Renamed,
        ChangeStatus::Unknown => FileStatus::Unknown,
    }
}

/// Truncate title to max 60 chars
fn truncate_title(message: &str) -> String {
    if message.len() > 60 {
        format!("{}...", &message[..57])
    } else {
        message.to_string()
    }
}

// =============================================================================
// LOCK FILE HELPERS
// =============================================================================

/// Check if all files in the group are lock files
fn is_lock_file_group(files: &[String]) -> bool {
    !files.is_empty()
        && files.iter().all(|f| {
            let filename = f.rsplit('/').next().unwrap_or(f);
            LOCK_FILES.iter().any(|lock| filename == *lock)
        })
}

/// Generate appropriate message for lock file groups
fn get_lock_file_message(files: &[String]) -> String {
    if files.len() == 1 {
        let file = &files[0];
        let filename = file.rsplit('/').next().unwrap_or(file);
        match filename {
            "Cargo.lock" => "Update Rust dependencies".to_string(),
            "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" => {
                "Update npm dependencies".to_string()
            }
            "poetry.lock" | "Pipfile.lock" => "Update Python dependencies".to_string(),
            "Gemfile.lock" => "Update Ruby dependencies".to_string(),
            "composer.lock" => "Update PHP dependencies".to_string(),
            "go.sum" => "Update Go dependencies".to_string(),
            _ => "Update dependencies".to_string(),
        }
    } else {
        "Update dependencies".to_string()
    }
}

// =============================================================================
// DIFF HELPERS
// =============================================================================

use super::analyze::{ChangeStatus, FileChange};
use super::AnalysisMode;
use std::fs;

/// Get comprehensive diff for a group of files
fn get_diff_for_group(group: &[FileChange], mode: &AnalysisMode) -> Result<String> {
    let repo_root = PathBuf::from(git::get_repo_root()?);
    let mut full_diff = String::new();

    for file_change in group {
        let file_diff = match &file_change.status {
            ChangeStatus::Added => get_added_file_diff(&file_change.path, &repo_root)?,
            ChangeStatus::Deleted => get_deleted_file_diff(&file_change.path, mode)?,
            ChangeStatus::Modified => get_modified_file_diff(&file_change.path, mode)?,
            ChangeStatus::Renamed { from } => get_renamed_file_diff(from, &file_change.path, mode)?,
            ChangeStatus::Unknown => get_added_file_diff(&file_change.path, &repo_root)?,
        };

        if !file_diff.is_empty() {
            full_diff.push_str(&file_diff);
            full_diff.push('\n');
        }
    }

    Ok(full_diff)
}

fn get_added_file_diff(path: &str, repo_root: &PathBuf) -> Result<String> {
    let abs_path = repo_root.join(path);
    let content = fs::read_to_string(&abs_path).unwrap_or_else(|_| String::from("(binary file)"));

    let mut diff = String::new();
    diff.push_str(&format!("diff --git a/{} b/{}\n", path, path));
    diff.push_str("new file mode 100644\n");
    diff.push_str("--- /dev/null\n");
    diff.push_str(&format!("+++ b/{}\n", path));
    diff.push_str("@@ -0,0 +1,");
    let line_count = content.lines().count();
    diff.push_str(&format!("{} @@\n", line_count));

    for line in content.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }

    Ok(diff)
}

fn get_deleted_file_diff(path: &str, mode: &AnalysisMode) -> Result<String> {
    match mode {
        AnalysisMode::WorkingTree | AnalysisMode::Staged => {
            match git::run_git_optional(&["show", &format!("HEAD:{}", path)]) {
                Ok(Some(content)) => {
                    let mut diff = String::new();
                    diff.push_str(&format!("diff --git a/{} b/{}\n", path, path));
                    diff.push_str("deleted file mode 100644\n");
                    diff.push_str(&format!("--- a/{}\n", path));
                    diff.push_str("+++ /dev/null\n");
                    diff.push_str("@@ -1,");
                    let line_count = content.lines().count();
                    diff.push_str(&format!("{} +0,0 @@\n", line_count));

                    for line in content.lines() {
                        diff.push('-');
                        diff.push_str(line);
                        diff.push('\n');
                    }
                    Ok(diff)
                }
                _ => Ok(format!("deleted file: {}\n", path)),
            }
        }
        AnalysisMode::History { from, to } => {
            let to_ref = to.as_deref().unwrap_or("HEAD");
            git::run_git(&["diff", from, to_ref, "--", path])
        }
        AnalysisMode::Auto => Ok(String::new()),
    }
}

fn get_modified_file_diff(path: &str, mode: &AnalysisMode) -> Result<String> {
    match mode {
        AnalysisMode::WorkingTree => git::run_git(&["diff", "--", path]),
        AnalysisMode::Staged => git::run_git(&["diff", "--cached", "--", path]),
        AnalysisMode::History { from, to } => {
            let to_ref = to.as_deref().unwrap_or("HEAD");
            git::run_git(&["diff", from, to_ref, "--", path])
        }
        AnalysisMode::Auto => Ok(String::new()),
    }
}

fn get_renamed_file_diff(from: &str, to: &str, mode: &AnalysisMode) -> Result<String> {
    match mode {
        AnalysisMode::Staged => git::run_git(&["diff", "--cached", "-M", "--", from, to]),
        _ => {
            let mut diff = String::new();
            diff.push_str(&format!("renamed: {} -> {}\n", from, to));

            if let Ok(changes) = get_modified_file_diff(to, mode) {
                if !changes.is_empty() {
                    diff.push_str(&changes);
                }
            }

            Ok(diff)
        }
    }
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_lock_file_group_true() {
        let files = vec!["Cargo.lock".to_string()];
        assert!(is_lock_file_group(&files));

        let files = vec!["package-lock.json".to_string()];
        assert!(is_lock_file_group(&files));
    }

    #[test]
    fn is_lock_file_group_false() {
        let files = vec!["src/main.rs".to_string()];
        assert!(!is_lock_file_group(&files));

        let files = vec!["Cargo.lock".to_string(), "src/main.rs".to_string()];
        assert!(!is_lock_file_group(&files));
    }

    #[test]
    fn get_lock_file_message_rust() {
        let files = vec!["Cargo.lock".to_string()];
        assert_eq!(get_lock_file_message(&files), "Update Rust dependencies");
    }

    #[test]
    fn get_lock_file_message_npm() {
        let files = vec!["package-lock.json".to_string()];
        assert_eq!(get_lock_file_message(&files), "Update npm dependencies");
    }

    #[test]
    fn get_lock_file_message_multiple() {
        let files = vec!["Cargo.lock".to_string(), "package-lock.json".to_string()];
        assert_eq!(get_lock_file_message(&files), "Update dependencies");
    }

    #[test]
    fn truncate_title_short() {
        let short = "Short message";
        assert_eq!(truncate_title(short), short);
    }

    #[test]
    fn truncate_title_long() {
        let long = "This is a very long commit message that exceeds the sixty character limit";
        let truncated = truncate_title(long);
        assert_eq!(truncated.len(), 60);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn commit_group_creation() {
        let group = CommitGroup {
            id: 0,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            files: vec!["file.rs".to_string()],
            files_with_status: vec![FileWithStatus {
                path: "file.rs".to_string(),
                status: FileStatus::Added,
            }],
            estimated_tokens: 100,
            is_large_file_group: false,
        };

        assert_eq!(group.id, 0);
        assert_eq!(group.title, "Test");
        assert_eq!(group.files.len(), 1);
        assert_eq!(group.files_with_status.len(), 1);
        assert_eq!(group.files_with_status[0].status, FileStatus::Added);
        assert!(!group.is_large_file_group);
    }

    #[test]
    fn file_status_labels() {
        assert_eq!(FileStatus::Added.label(), "A");
        assert_eq!(FileStatus::Modified.label(), "M");
        assert_eq!(FileStatus::Deleted.label(), "D");
        assert_eq!(FileStatus::Renamed.label(), "R");
        assert_eq!(FileStatus::Unknown.label(), "?");
    }

    #[test]
    fn change_status_conversion() {
        assert_eq!(
            change_status_to_file_status(&ChangeStatus::Added),
            FileStatus::Added
        );
        assert_eq!(
            change_status_to_file_status(&ChangeStatus::Modified),
            FileStatus::Modified
        );
        assert_eq!(
            change_status_to_file_status(&ChangeStatus::Deleted),
            FileStatus::Deleted
        );
        assert_eq!(
            change_status_to_file_status(&ChangeStatus::Renamed {
                from: "old.rs".to_string()
            }),
            FileStatus::Renamed
        );
        assert_eq!(
            change_status_to_file_status(&ChangeStatus::Unknown),
            FileStatus::Unknown
        );
    }
}
