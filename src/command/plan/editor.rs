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
    Approved(Vec<CommitGroup>),
    Cancelled,
}

enum Action {
    Accept,
    Regenerate,
    MoveFile {
        file: String,
        from: usize,
        to: usize,
    },
    ExcludeFile(String),
    EditMessage {
        group_id: usize,
    },
    Quit,
}

// =============================================================================
// PLAN EDITOR
// =============================================================================

pub struct PlanEditor {
    pub groups: Vec<CommitGroup>,
    pub excluded: Vec<String>,
}

impl PlanEditor {
    pub fn new(groups: Vec<CommitGroup>) -> Self {
        Self {
            groups,
            excluded: Vec::new(),
        }
    }

    /// Display the current plan
    pub fn display(&self) {
        println!("\n===========================================================");
        println!("Commit Plan ({} commits)", self.groups.len());
        println!("===========================================================\n");

        for (idx, group) in self.groups.iter().enumerate() {
            println!("Commit {}/{}", idx + 1, self.groups.len());
            println!("Message: {}", group.message);
            println!("Files ({}):", group.files_with_status.len());
            for file in &group.files_with_status {
                println!("  {} {}", file.status.label(), file.path);
            }
            println!();
        }

        if !self.excluded.is_empty() {
            println!("Excluded files ({}):", self.excluded.len());
            for file in &self.excluded {
                println!("  - {}", file);
            }
            println!();
        }

        println!("-----------------------------------------------------------");
        println!("Options:");
        println!("  [Enter] Accept plan and execute");
        println!("  [r]     Regenerate plan (re-call LLM)");
        println!("  [m]     Move file between commits");
        println!("  [x]     Exclude file from all commits");
        println!("  [e]     Edit commit message");
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
                Action::Accept => {
                    return Ok(EditResult::Approved(self.groups.clone()));
                }
                Action::Regenerate => {
                    println!("\nRegenerating plan...");
                    self.regenerate(client, analysis, preset, algo, max_chars, secret_action)
                        .await?;
                    println!("Plan regenerated.");
                }
                Action::MoveFile { file, from, to } => {
                    self.move_file(&file, from, to)?;
                    println!(
                        "Moved {} from commit {} to commit {}",
                        file,
                        from + 1,
                        to + 1
                    );
                }
                Action::ExcludeFile(file) => {
                    self.exclude_file(&file)?;
                    println!("Excluded {}", file);
                }
                Action::EditMessage { group_id } => {
                    self.edit_message(group_id)?;
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
            "" | "y" | "yes" | "a" | "accept" => Ok(Action::Accept),
            "r" | "regenerate" => Ok(Action::Regenerate),
            "q" | "quit" | "exit" => Ok(Action::Quit),
            _ if input.starts_with("m ") || input.starts_with("move ") => {
                self.parse_move_action(input)
            }
            _ if input.starts_with("x ") || input.starts_with("exclude ") => {
                self.parse_exclude_action(input)
            }
            _ if input.starts_with("e ") || input.starts_with("edit ") => {
                self.parse_edit_action(input)
            }
            _ => bail!("Invalid choice. Try: Enter, r, m, x, e, or q."),
        }
    }

    /// Parse move action: "m <file> <from> <to>"
    fn parse_move_action(&self, input: &str) -> Result<Action> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() < 4 {
            bail!("Usage: m <file> <from_commit_number> <to_commit_number>");
        }

        let file = parts[1..parts.len() - 2].join(" ");
        let from: usize = parts[parts.len() - 2].parse()?;
        let to: usize = parts[parts.len() - 1].parse()?;

        if from == 0 || to == 0 {
            bail!("Commit numbers start from 1");
        }

        if from > self.groups.len() || to > self.groups.len() {
            bail!("Commit number out of range (max: {})", self.groups.len());
        }

        Ok(Action::MoveFile {
            file,
            from: from - 1,
            to: to - 1,
        })
    }

    /// Parse exclude action: "x <file>"
    fn parse_exclude_action(&self, input: &str) -> Result<Action> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() < 2 {
            bail!("Usage: x <file>");
        }

        let file = parts[1..].join(" ");
        Ok(Action::ExcludeFile(file))
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

    /// Move file from one group to another
    fn move_file(&mut self, file: &str, from: usize, to: usize) -> Result<()> {
        let from_group = &mut self.groups[from];
        let file_idx = from_group
            .files
            .iter()
            .position(|f| f == file)
            .ok_or_else(|| anyhow::anyhow!("File {} not found in commit {}", file, from + 1))?;

        from_group.files.remove(file_idx);

        let to_group = &mut self.groups[to];
        to_group.files.push(file.to_string());

        Ok(())
    }

    /// Exclude file from all groups
    fn exclude_file(&mut self, file: &str) -> Result<()> {
        let mut found = false;

        for group in &mut self.groups {
            if let Some(idx) = group.files.iter().position(|f| f == file) {
                group.files.remove(idx);
                found = true;
            }
        }

        if !found {
            bail!("File {} not found in any commit", file);
        }

        self.excluded.push(file.to_string());
        Ok(())
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
            },
        ];

        PlanEditor::new(groups)
    }

    #[test]
    fn parse_action_accept() {
        let editor = create_test_editor();
        assert!(matches!(editor.parse_action("").unwrap(), Action::Accept));
        assert!(matches!(editor.parse_action("y").unwrap(), Action::Accept));
        assert!(matches!(
            editor.parse_action("yes").unwrap(),
            Action::Accept
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

    #[test]
    fn parse_move_action_valid() {
        let editor = create_test_editor();
        match editor.parse_action("m README.md 1 2").unwrap() {
            Action::MoveFile { file, from, to } => {
                assert_eq!(file, "README.md");
                assert_eq!(from, 0);
                assert_eq!(to, 1);
            }
            _ => panic!("Expected MoveFile action"),
        }
    }

    #[test]
    fn move_file_between_groups() {
        let mut editor = create_test_editor();
        editor.move_file("README.md", 0, 1).unwrap();

        assert_eq!(editor.groups[0].files.len(), 1);
        assert_eq!(editor.groups[1].files.len(), 2);
    }

    #[test]
    fn exclude_file_from_groups() {
        let mut editor = create_test_editor();
        editor.exclude_file("README.md").unwrap();

        assert_eq!(editor.groups[0].files.len(), 1);
        assert_eq!(editor.excluded.len(), 1);
    }
}
