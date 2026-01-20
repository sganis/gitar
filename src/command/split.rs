use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};

use crate::client::LlmClient;
use crate::git;
use crate::prompt::Preset;

// =============================================================================
// DATA STRUCTURES
// =============================================================================

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FileChange {
    path: String,
    status: ChangeStatus,
    additions: usize,
    deletions: usize,
    category: FileCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed { from: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
enum FileCategory {
    Documentation,
    Tests,
    Config,
    Data, // Optional/untracked fixtures/datasets
    Formatting,
    Rename,
    Code,
}

#[derive(Debug)]
#[allow(dead_code)]
struct CommitGroup {
    title: String,
    message: String,
    files: Vec<String>,
    needs_hunk_staging: Vec<String>,
    is_optional: bool, // optional untracked data/fixtures group
}

#[derive(Debug)]
struct CommitPlan {
    groups: Vec<CommitGroup>,
}

// =============================================================================
// MAIN COMMAND ENTRY POINT
// =============================================================================

pub async fn cmd_split(client: &LlmClient, preset: Preset, algo: u8) -> Result<()> {
    println!("\n🎸 \x1b[1mGitar Split\x1b[0m - Interactive commit splitting\n");

    // Step 1: Scan the diff
    println!("📊 Scanning working tree changes...");
    let changes = scan_diff()?;

    if changes.is_empty() {
        println!("\x1b[33m⚠️  No unstaged changes found.\x1b[0m");
        println!("   Use \x1b[36mgit add\x1b[0m to unstage files if needed.");
        return Ok(());
    }

    println!("   Found {} changed file(s)\n", changes.len());

    // Step 2: Group changes
    println!("🔍 Grouping changes by category...");
    let initial_groups = group_changes(&changes);
    println!("   Created {} initial group(s)\n", initial_groups.len());

    // Step 3: Use LLM to refine grouping and generate messages
    println!("🤖 Using AI to refine groups and generate commit messages...");
    let plan = generate_plan(client, preset, algo, &changes, initial_groups).await?;

    if plan.groups.is_empty() {
        println!("\x1b[33m⚠️  No commit groups generated.\x1b[0m");
        return Ok(());
    }

    println!("   Generated {} commit(s)\n", plan.groups.len());

    // Step 4: Print the plan
    print_plan(&plan)?;

    // Step 4.5: Optional untracked data groups prompt (once)
    let skip_groups = prompt_optional_groups(&plan)?;

    // Step 5: Execute interactively
    println!("\n\x1b[1m─────────────────────────────────────────────────────────────\x1b[0m");
    println!("\x1b[1mReady to execute plan\x1b[0m\n");

    execute_plan(&plan, &skip_groups).await?;

    println!("\n\x1b[32m✓ Split complete!\x1b[0m");
    Ok(())
}

// =============================================================================
// STEP 1: SCAN DIFF
// =============================================================================

fn scan_diff() -> Result<Vec<FileChange>> {
    // Get unstaged changes with numstat (tracked files only)
    let numstat = git::run_git(&["diff", "--numstat"])?;
    let mut changes = Vec::new();

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

    // Also check for renames and new files
    let status = git::run_git(&["status", "--porcelain"])?;
    for line in status.lines() {
        if line.len() < 4 {
            continue;
        }
        let status_code = &line[..2];
        let path = line[3..].to_string();

        match status_code {
            "??" => {
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
                if let Some(idx) = changes.iter().position(|c| c.path == path) {
                    changes[idx].status = ChangeStatus::Renamed { from: path.clone() };
                }
            }
            " D" | "D " => {
                if let Some(idx) = changes.iter().position(|c| c.path == path) {
                    changes[idx].status = ChangeStatus::Deleted;
                }
            }
            _ => {}
        }
    }

    Ok(changes)
}

fn categorize_file(path: &str) -> FileCategory {
    let lower = path.to_lowercase();

    // Optional data/fixtures/datasets
    if lower.ends_with(".csv")
        || lower.ends_with(".parquet")
        || lower.ends_with(".tsv")
        || lower.ends_with(".log")
        || lower.ends_with(".tmp")
        || lower.contains("/fixtures/")
        || lower.contains("/samples/")
        || lower.contains("/sample/")
        || lower.contains("/data/")
    {
        return FileCategory::Data;
    }

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

    // Config/tooling
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

    FileCategory::Code
}

// =============================================================================
// STEP 2: GROUP CHANGES
// =============================================================================

fn group_changes(changes: &[FileChange]) -> Vec<Vec<FileChange>> {
    let mut groups: Vec<Vec<FileChange>> = Vec::new();

    // Group 0: Optional untracked data/fixtures
    let optional_data: Vec<FileChange> = changes
        .iter()
        .filter(|c| c.status == ChangeStatus::Added && c.category == FileCategory::Data)
        .cloned()
        .collect();
    if !optional_data.is_empty() {
        groups.push(optional_data);
    }

    // Group 1: Documentation
    let docs: Vec<FileChange> = changes
        .iter()
        .filter(|c| c.category == FileCategory::Documentation)
        .cloned()
        .collect();
    if !docs.is_empty() {
        groups.push(docs);
    }

    // Group 2: Tests
    let tests: Vec<FileChange> = changes
        .iter()
        .filter(|c| c.category == FileCategory::Tests)
        .cloned()
        .collect();
    if !tests.is_empty() {
        groups.push(tests);
    }

    // Group 3: Config
    let config: Vec<FileChange> = changes
        .iter()
        .filter(|c| c.category == FileCategory::Config)
        .cloned()
        .collect();
    if !config.is_empty() {
        groups.push(config);
    }

    // Group 4: Renames
    let renames: Vec<FileChange> = changes
        .iter()
        .filter(|c| matches!(c.status, ChangeStatus::Renamed { .. }))
        .cloned()
        .collect();
    if !renames.is_empty() {
        groups.push(renames);
    }

    // Group 5+: Code changes by directory
    let code_changes: Vec<FileChange> = changes
        .iter()
        .filter(|c| {
            c.category == FileCategory::Code
                && !(c.status == ChangeStatus::Added && c.category == FileCategory::Data)
                && !matches!(c.status, ChangeStatus::Renamed { .. })
        })
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
// STEP 3: GENERATE PLAN WITH LLM
// =============================================================================

async fn generate_plan(
    client: &LlmClient,
    preset: Preset,
    _algo: u8,
    _changes: &[FileChange],
    groups: Vec<Vec<FileChange>>,
) -> Result<CommitPlan> {
    let mut commit_groups = Vec::new();

    for (idx, group) in groups.into_iter().enumerate() {
        let files: Vec<String> = group.iter().map(|c| c.path.clone()).collect();
        let untracked: Vec<String> = group
            .iter()
            .filter(|c| c.status == ChangeStatus::Added)
            .map(|c| c.path.clone())
            .collect();

        let is_optional = group
            .iter()
            .all(|c| c.status == ChangeStatus::Added && c.category == FileCategory::Data);

        // Get diff for this group of files (including untracked)
        let diff_output = get_diff_for_files(&files, &untracked)?;

        // Use LLM to generate commit message
        let system_prompt = build_system_prompt(preset, &files);
        let user_prompt = format!(
            "Generate a concise commit message (one line, imperative mood) for these changes:\n\n{}",
            diff_output
        );

        let message = match client.chat(&system_prompt, &user_prompt, false).await {
            Ok(msg) => msg.trim().to_string(),
            Err(e) => {
                eprintln!("\x1b[33m⚠️  LLM error for group {}: {}\x1b[0m", idx + 1, e);
                format!("Update {} files", files.len())
            }
        };

        let needs_hunk_staging = detect_mixed_changes(&files)?;

        let title = if message.len() > 60 {
            format!("{}...", &message[..57])
        } else {
            message.clone()
        };

        commit_groups.push(CommitGroup {
            title,
            message,
            files,
            needs_hunk_staging,
            is_optional,
        });
    }

    Ok(CommitPlan {
        groups: commit_groups,
    })
}

fn build_system_prompt(preset: Preset, _files: &[String]) -> String {
    let preset_hint = match preset {
        Preset::Rust => "Focus on Rust conventions (crate/module structure)",
        Preset::JavaScript => "Focus on JavaScript conventions (components/hooks)",
        Preset::Python => "Focus on Python conventions (modules/packages)",
        Preset::Default => "Focus on clear, concise descriptions",
    };

    format!(
        "You are a commit message generator. Generate a single-line commit message in imperative mood. {}. Be specific and concise. Never ask questions.",
        preset_hint
    )
}

// =============================================================================
// DIFF GENERATION (TRACKED + UNTRACKED)
// =============================================================================

fn get_diff_for_files(files: &[String], untracked: &[String]) -> Result<String> {
    let untracked_set: HashSet<&str> = untracked.iter().map(|s| s.as_str()).collect();
    let tracked_files: Vec<&str> = files
        .iter()
        .map(|s| s.as_str())
        .filter(|p| !untracked_set.contains(p))
        .collect();

    let mut combined = String::new();

    // Tracked modifications/deletions
    if !tracked_files.is_empty() {
        let mut args: Vec<&str> = vec!["diff", "--"];
        args.extend(tracked_files);
        let diff = git::run_git(&args)?;
        if !diff.trim().is_empty() {
            combined.push_str(&diff);
            if !combined.ends_with('\n') {
                combined.push('\n');
            }
        }
    }

    // Untracked files: synthesize diff
    for path in untracked {
        let snippet = format_untracked_as_diff(path)?;
        if !snippet.trim().is_empty() {
            combined.push_str(&snippet);
            if !combined.ends_with('\n') {
                combined.push('\n');
            }
        }
    }

    if combined.len() > 8000 {
        Ok(format!("{}...[truncated]", &combined[..8000]))
    } else {
        Ok(combined)
    }
}

fn format_untracked_as_diff(path: &str) -> Result<String> {
    let bytes = fs::read(path).unwrap_or_default();
    if bytes.is_empty() {
        return Ok(format!(
            "diff --git a/{p} b/{p}\nnew file mode 100644\n--- /dev/null\n+++ b/{p}\n",
            p = path
        ));
    }

    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => {
            return Ok(format!(
                "diff --git a/{p} b/{p}\nnew file mode 100644\nBinary files /dev/null and b/{p} differ\n",
                p = path
            ))
        }
    };

    let mut out = String::new();
    out.push_str(&format!(
        "diff --git a/{p} b/{p}\nnew file mode 100644\n--- /dev/null\n+++ b/{p}\n",
        p = path
    ));
    out.push_str("@@ -0,0 +1 @@\n");

    for line in text.lines() {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }

    Ok(out)
}

fn detect_mixed_changes(_files: &[String]) -> Result<Vec<String>> {
    Ok(vec![])
}

// =============================================================================
// STEP 4: PRINT PLAN
// =============================================================================

fn print_plan(plan: &CommitPlan) -> Result<()> {
    println!("\x1b[1m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("\x1b[1mCommit Plan ({} commits)\x1b[0m", plan.groups.len());
    println!("\x1b[1m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m\n");

    for (idx, group) in plan.groups.iter().enumerate() {
        println!("\x1b[1;36mCommit {}/{}\x1b[0m", idx + 1, plan.groups.len());
        println!("\x1b[1mMessage:\x1b[0m {}", group.message);
        if group.is_optional {
            println!(
                "\x1b[33mNote:\x1b[0m Optional untracked data/fixtures group (can be skipped)"
            );
        }
        println!("\x1b[1mFiles:\x1b[0m");
        for file in &group.files {
            println!("  • {}", file);
        }

        println!("\x1b[1mCommands:\x1b[0m");
        for file in &group.files {
            if group.needs_hunk_staging.contains(file) {
                println!(
                    "  \x1b[33mgit add -p {}\x1b[0m  \x1b[2m(interactive)\x1b[0m",
                    file
                );
            } else {
                println!("  git add {}", file);
            }
        }
        println!("  git commit -m \"{}\"", group.message);
        println!();
    }

    Ok(())
}

// =============================================================================
// OPTIONAL GROUPS PROMPT (ONCE)
// =============================================================================

fn prompt_optional_groups(plan: &CommitPlan) -> Result<HashSet<usize>> {
    let optional_idxs: Vec<usize> = plan
        .groups
        .iter()
        .enumerate()
        .filter(|(_, g)| g.is_optional)
        .map(|(i, _)| i)
        .collect();

    if optional_idxs.is_empty() {
        return Ok(HashSet::new());
    }

    println!("\n\x1b[33mOptional untracked data/fixtures detected:\x1b[0m");
    for &i in &optional_idxs {
        let g = &plan.groups[i];
        println!("  {}: {}", i + 1, g.title);
        for f in &g.files {
            println!("     • {}", f);
        }
    }

    println!("\nInclude optional files? \x1b[1m[y]\x1b[0mes / \x1b[1m[N]\x1b[0mo  (default: no)");

    print!("Choice: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim().to_lowercase();

    // Default: skip all optional groups
    if choice.is_empty() || choice == "n" || choice == "no" {
        return Ok(optional_idxs.into_iter().collect());
    }

    // Yes = include all optional groups
    if choice == "y" || choice == "yes" {
        return Ok(HashSet::new());
    }

    // Any other input = safe default: skip
    Ok(optional_idxs.into_iter().collect())

}

// =============================================================================
// STEP 5: EXECUTE PLAN
// =============================================================================

async fn execute_plan(plan: &CommitPlan, skip_groups: &HashSet<usize>) -> Result<()> {
    for (idx, group) in plan.groups.iter().enumerate() {
        if skip_groups.contains(&idx) {
            println!(
                "\n\x1b[33m⊘ Skipping optional group {}/{}: {}\x1b[0m",
                idx + 1,
                plan.groups.len(),
                group.title
            );
            continue;
        }

        println!(
            "\n\x1b[1;36m▶ Commit {}/{}\x1b[0m",
            idx + 1,
            plan.groups.len()
        );
        println!("\x1b[1m{}\x1b[0m\n", group.message);

        // Stage files
        println!("📦 Staging files...");
        for file in &group.files {
            if group.needs_hunk_staging.contains(file) {
                println!("   Launching interactive staging for: {}", file);
                let status = std::process::Command::new("git")
                    .args(&["add", "-p", file])
                    .status()?;

                if !status.success() {
                    println!("\x1b[33m   ⚠️  Interactive staging cancelled or failed\x1b[0m");
                }
            } else {
                git::run_git(&["add", file])?;
                println!("   ✓ {}", file);
            }
        }

        // Show diff summary
        println!("\n📝 Staged changes:");
        let stat = git::run_git(&["diff", "--cached", "--stat"])?;
        println!("{}", stat);

        // Prompt user
        loop {
            print!("\n\x1b[1m[Y] commit / [e] edit message / [s] skip / [v] view diff / [q] quit\x1b[0m: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let choice = input.trim().to_lowercase();

            match choice.as_str() {
                "y" | "yes" | "" => {
                    // Commit
                    git::run_git(&["commit", "-m", &group.message])?;
                    println!("\x1b[32m✓ Committed\x1b[0m");
                    break;
                }
                "e" | "edit" => {
                    // Edit message
                    print!("Enter new message: ");
                    io::stdout().flush()?;
                    let mut new_message = String::new();
                    io::stdin().read_line(&mut new_message)?;
                    let new_message = new_message.trim();

                    if !new_message.is_empty() {
                        git::run_git(&["commit", "-m", new_message])?;
                        println!("\x1b[32m✓ Committed with edited message\x1b[0m");
                        break;
                    } else {
                        println!("\x1b[33m   Message cannot be empty\x1b[0m");
                    }
                }
                "s" | "skip" => {
                    // Unstage and skip
                    for file in &group.files {
                        git::run_git(&["reset", "HEAD", file])?;
                    }
                    println!("\x1b[33m⊘ Skipped (files unstaged)\x1b[0m");
                    break;
                }
                "q" | "quit" => {
                    // Unstage and quit
                    for file in &group.files {
                        git::run_git(&["reset", "HEAD", file])?;
                    }
                    println!("\n\x1b[33m⊘ Quit (remaining files unstaged)\x1b[0m");
                    return Ok(());
                }
                "v" | "view" => {
                    // View full diff
                    let diff = git::run_git(&["diff", "--cached"])?;
                    println!("\n{}", diff);
                }
                _ => {
                    println!("\x1b[31m   Invalid choice. Please enter Y, e, s, v, or q.\x1b[0m");
                }
            }
        }
    }

    Ok(())
}
