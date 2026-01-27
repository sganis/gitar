// src/command/plan/editor.rs
use anyhow::{bail, Result};
use std::io::{self, Write};

use crate::client::LlmClient;
use crate::context::secret::SecretAction;
use crate::context::Preset;

use super::analyze::AnalysisResult;
use super::group::{create_groups, CommitGroup};

// =============================================================================
// DATA STRUCTURES
// =============================================================================

pub enum EditResult {
    /// Accept all and execute without per-commit prompts
    /// bool = include_large_files
    ApprovedAll(Vec<CommitGroup>, bool),
    /// Approve one by one with per-commit prompts
    /// bool = include_large_files
    ApprovedOneByOne(Vec<CommitGroup>, bool),
    Cancelled,
}

enum Action {
    AcceptAll,
    AcceptOneByOne,
    Regenerate,
    EditMessage { group_id: usize },
    IncludeLargeFiles,
    Quit,
}

// =============================================================================
// PLAN EDITOR
// =============================================================================

pub struct PlanEditor {
    pub groups: Vec<CommitGroup>,
    pub include_large_files: bool,
}

impl PlanEditor {
    pub fn new(groups: Vec<CommitGroup>) -> Self {
        Self {
            groups,
            include_large_files: false,
        }
    }

    /// Display the current plan
    pub fn display(&self) {
        let total_groups = self.groups.len();
        let skip_groups = self.groups.iter().filter(|g| g.is_large_file_group).count();

        println!("\n===========================================================");
        println!("Commit Plan ({} groups)", total_groups);
        println!("===========================================================\n");

        for (idx, group) in self.groups.iter().enumerate() {
            if group.is_large_file_group {
                // Large/binary file group - shown with SKIP indicator
                println!("Group {}/{} [BINARY/LARGE - default: SKIP]", idx + 1, total_groups);
                println!("Title: {}", group.title);
                println!("Files ({}):", group.files_with_status.len());
                for file in &group.files_with_status {
                    println!("  {} {}", file.status.label(), file.path);
                }
                println!("  (Will be SKIPPED unless you choose to commit)");
            } else {
                // Regular commit group
                println!("Group {}/{}", idx + 1, total_groups);
                println!("Message: {}", group.message);
                println!("Files ({}):", group.files_with_status.len());
                for file in &group.files_with_status {
                    println!("  {} {}", file.status.label(), file.path);
                }
            }
            println!();
        }

        println!("-----------------------------------------------------------");
        if skip_groups > 0 {
            if self.include_large_files {
                println!("Binary/large files: WILL BE COMMITTED");
            } else {
                println!("Binary/large files: WILL BE SKIPPED (default)");
            }
            println!("-----------------------------------------------------------");
        }
        println!("Options:");
        if self.include_large_files {
            println!("  [Enter] Accept all and commit (including binary/large files)");
        } else {
            println!("  [Enter] Accept all and commit (binary/large files will be SKIPPED)");
        }
        println!("  [y]     Approve commits one by one");
        println!("  [r]     Regenerate plan (re-call LLM)");
        println!("  [e]     Edit commit message");
        if skip_groups > 0 {
            if self.include_large_files {
                println!("  [i]     Exclude binary/large files (skip them)");
            } else {
                println!("  [i]     Include binary/large files (commit them)");
            }
        }
        println!("  [q]     Quit without executing");
        println!("-----------------------------------------------------------");
    }

    /// Run interactive editing loop
    pub async fn run_interactive(
        &mut self,
        client: &LlmClient,
        analysis: &AnalysisResult,
        preset: Preset,
        algo: u8,
        max_chars: usize,
        secret_action: SecretAction,
    ) -> Result<EditResult> {
        loop {
            self.display();

            print!("Choice: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let choice = input.trim().to_lowercase();

            match self.parse_action(&choice)? {
                Action::AcceptAll => {
                    return Ok(EditResult::ApprovedAll(self.groups.clone(), self.include_large_files));
                }
                Action::AcceptOneByOne => {
                    return Ok(EditResult::ApprovedOneByOne(self.groups.clone(), self.include_large_files));
                }
                Action::Regenerate => {
                    println!("\nRegenerating plan...");
                    self.regenerate(client, analysis, preset, algo, max_chars, secret_action)
                        .await?;
                    println!("Plan regenerated.");
                }
                Action::EditMessage { group_id } => {
                    self.edit_message(group_id)?;
                }
                Action::IncludeLargeFiles => {
                    self.include_large_files = !self.include_large_files;
                    if self.include_large_files {
                        println!("Binary/large files will be COMMITTED.");
                    } else {
                        println!("Binary/large files will be SKIPPED (default).");
                    }
                }
                Action::Quit => {
                    println!("Cancelled.");
                    return Ok(EditResult::Cancelled);
                }
            }
        }
    }

    /// Parse user input into Action
    fn parse_action(&self, input: &str) -> Result<Action> {
        match input {
            "" | "a" | "accept" | "all" => Ok(Action::AcceptAll),
            "y" | "yes" => Ok(Action::AcceptOneByOne),
            "r" | "regenerate" => Ok(Action::Regenerate),
            "i" | "include" => Ok(Action::IncludeLargeFiles),
            "q" | "quit" | "exit" => Ok(Action::Quit),
            "e" | "edit" => self.prompt_edit_action(),
            _ if input.starts_with("e ") || input.starts_with("edit ") => {
                self.parse_edit_action(input)
            }
            _ => bail!("Invalid choice. Try: Enter, y, r, e, i, or q."),
        }
    }

    /// Prompt for edit action arguments
    fn prompt_edit_action(&self) -> Result<Action> {
        print!("Group number to edit: ");
        io::stdout().flush()?;
        let mut num_str = String::new();
        io::stdin().read_line(&mut num_str)?;
        let group_id: usize = num_str.trim().parse().map_err(|_| anyhow::anyhow!("Invalid number"))?;

        if group_id == 0 {
            bail!("Group numbers start from 1");
        }
        if group_id > self.groups.len() {
            bail!("Group number out of range (max: {})", self.groups.len());
        }

        Ok(Action::EditMessage {
            group_id: group_id - 1,
        })
    }

    /// Parse edit action: "e <commit_number>"
    fn parse_edit_action(&self, input: &str) -> Result<Action> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() != 2 {
            bail!("Usage: e <commit_number>");
        }

        let group_id: usize = parts[1].parse()?;
        if group_id == 0 {
            bail!("Commit numbers start from 1");
        }
        if group_id > self.groups.len() {
            bail!("Commit number out of range (max: {})", self.groups.len());
        }

        Ok(Action::EditMessage {
            group_id: group_id - 1,
        })
    }

    /// Edit commit message
    fn edit_message(&mut self, group_id: usize) -> Result<()> {
        let group = &mut self.groups[group_id];

        println!("\nCurrent message:");
        println!("{}", group.message);
        println!();

        print!("Enter new message: ");
        io::stdout().flush()?;

        let mut new_message = String::new();
        io::stdin().read_line(&mut new_message)?;
        let new_message = new_message.trim();

        if new_message.is_empty() {
            bail!("Message cannot be empty");
        }

        group.message = new_message.to_string();
        group.title = if new_message.len() > 60 {
            format!("{}...", &new_message[..57])
        } else {
            new_message.to_string()
        };

        println!("Message updated.");
        Ok(())
    }

    /// Regenerate plan by calling LLM again
    async fn regenerate(
        &mut self,
        client: &LlmClient,
        analysis: &AnalysisResult,
        preset: Preset,
        algo: u8,
        max_chars: usize,
        secret_action: SecretAction,
    ) -> Result<()> {
        let new_groups =
            create_groups(analysis, client, preset, algo, max_chars, secret_action).await?;

        if new_groups.is_empty() {
            bail!("Failed to regenerate plan");
        }

        self.groups = new_groups;
        Ok(())
    }
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::plan::group::{FileStatus, FileWithStatus};

    fn create_test_editor() -> PlanEditor {
        let groups = vec![
            CommitGroup {
                id: 0,
                title: "Update docs".to_string(),
                message: "Update documentation".to_string(),
                files: vec!["README.md".to_string(), "CHANGELOG.md".to_string()],
                files_with_status: vec![
                    FileWithStatus {
                        path: "README.md".to_string(),
                        status: FileStatus::Modified,
                    },
                    FileWithStatus {
                        path: "CHANGELOG.md".to_string(),
                        status: FileStatus::Modified,
                    },
                ],
                estimated_tokens: 100,
                is_large_file_group: false,
            },
            CommitGroup {
                id: 1,
                title: "Add tests".to_string(),
                message: "Add unit tests".to_string(),
                files: vec!["test.rs".to_string()],
                files_with_status: vec![FileWithStatus {
                    path: "test.rs".to_string(),
                    status: FileStatus::Added,
                }],
                estimated_tokens: 150,
                is_large_file_group: false,
            },
        ];

        PlanEditor::new(groups)
    }

    #[test]
    fn parse_action_accept_all() {
        let editor = create_test_editor();
        assert!(matches!(
            editor.parse_action("").unwrap(),
            Action::AcceptAll
        ));
        assert!(matches!(
            editor.parse_action("a").unwrap(),
            Action::AcceptAll
        ));
        assert!(matches!(
            editor.parse_action("all").unwrap(),
            Action::AcceptAll
        ));
    }

    #[test]
    fn parse_action_accept_one_by_one() {
        let editor = create_test_editor();
        assert!(matches!(
            editor.parse_action("y").unwrap(),
            Action::AcceptOneByOne
        ));
        assert!(matches!(
            editor.parse_action("yes").unwrap(),
            Action::AcceptOneByOne
        ));
    }

    #[test]
    fn parse_action_regenerate() {
        let editor = create_test_editor();
        assert!(matches!(
            editor.parse_action("r").unwrap(),
            Action::Regenerate
        ));
    }

    #[test]
    fn parse_action_quit() {
        let editor = create_test_editor();
        assert!(matches!(editor.parse_action("q").unwrap(), Action::Quit));
    }
}
