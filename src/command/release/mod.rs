// src/command/release/mod.rs - Release command orchestration

mod tag;
mod version;

use anyhow::{bail, Context, Result};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::client::LlmClient;
use crate::color::{arrow, success, warning};
use crate::command::{apply_smart_diff_with_context, AnalysisContext, SHORT_HASH_LEN};
use crate::context::load_all_context;
use crate::context::secret::SecretAction;
use crate::context::template::{
    changelog_system_with_context, get_changelog_user, get_version_user,
    version_system_with_context,
};
use crate::git::{self, build_diff_target, get_current_version, get_diff};

use tag::{create_tag, get_commits_since, tag_exists};
use version::{detect_version_files, update_version_file, VersionFile};

// Re-exports for external use
pub use self::tag::{get_latest_tag, CommitInfo};

// =============================================================================
// MAIN COMMAND
// =============================================================================

pub async fn cmd_release(
    client: &LlmClient,
    apply: bool,
    skip_changelog: bool,
    skip_changelog_file: bool,
    changelog_file: String,
    from: Option<String>,
    bump: String,
    to: Option<String>,
    algo: u8,
    base_branch: &str,
    stream: bool,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    println!("===========================================================");
    println!("Gitar Release Workflow");
    println!("===========================================================\n");

    // Step 1: Determine the starting point
    let from_ref = match from {
        Some(r) => r,
        None => {
            match get_latest_tag()? {
                Some(tag) => {
                    println!("Starting from latest tag: {}", tag);
                    tag
                }
                None => {
                    println!("No tags found. Analyzing all commits from initial commit.");
                    // Get the first commit
                    let first_commit = git::run_git(&["rev-list", "--max-parents=0", "HEAD"])?
                        .trim()
                        .to_string();
                    first_commit
                }
            }
        }
    };

    // Step 2: Get commits since the starting point
    let commits = get_commits_since(&from_ref, None)?;

    if commits.is_empty() {
        println!("No new commits since {}.", from_ref);
        println!("Nothing to release.");
        return Ok(());
    }

    println!("Found {} new commit(s) since {}\n", commits.len(), from_ref);

    // Step 3: Analyze commits and suggest version bump
    let version_bump = if bump == "auto" {
        // Use LLM to analyze the diff and suggest version bump
        println!("Analyzing changes with LLM to suggest version bump...\n");
        suggest_version_bump_llm(
            client,
            &from_ref,
            to.as_deref(),
            base_branch,
            stream,
            algo,
            max_diff_chars,
            secret_action,
        )
        .await?
    } else {
        // Use explicit bump strategy
        validate_bump_strategy(&bump)?;
        bump.clone()
    };
    println!("Suggested version bump: {}", version_bump);

    // Get repo root for all path operations (works from any subdirectory)
    let repo_root = PathBuf::from(git::get_repo_root()?);

    // Step 4: Detect version files (use current directory, not repo root,
    // so subfolder projects like monorepos work correctly)
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let version_files = detect_version_files(&cwd)?;

    if version_files.is_empty() {
        println!();
        warning("No version files detected (Cargo.toml, package.json, pyproject.toml)");
        println!("   Skipping version update step.");
    } else {
        println!("\nDetected version files:");
        for file in &version_files {
            println!(
                "  {} (current: {})",
                file.path.display(),
                file.current_version
            );
        }
    }

    // Step 4a: Determine changelog file path
    let changelog_path = if skip_changelog_file || skip_changelog {
        None
    } else {
        Some(repo_root.join(&changelog_file))
    };

    // Step 5: Determine new version
    let new_version = if let Some(first_file) = version_files.first() {
        compute_new_version(&first_file.current_version, &version_bump)?
    } else {
        // No version files, ask user for version
        print!("\nEnter release version (e.g., 1.0.0): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    if new_version.is_empty() {
        bail!("Version cannot be empty");
    }

    println!("\nNew version: {}", new_version);

    // Step 6: Check if tag already exists
    let tag_name = format!("v{}", new_version);
    if tag_exists(&tag_name)? {
        bail!(
            "Tag {} already exists. Use a different version or delete the existing tag.",
            tag_name
        );
    }

    // Step 7: Generate changelog
    let changelog = if skip_changelog {
        println!("\nSkipping changelog generation (--skip-changelog)");
        format!("Release {}", new_version)
    } else {
        println!("\nGenerating changelog...");
        generate_changelog_content(
            &commits,
            &from_ref,
            &new_version,
            client,
            stream,
            algo,
            max_diff_chars,
            secret_action,
        )
        .await?
    };

    // Step 8: Display release plan
    println!("\n===========================================================");
    println!("Release Plan");
    println!("===========================================================");
    println!("Version     : {}", new_version);
    println!("Tag         : {}", tag_name);
    println!("Base        : {}", from_ref);
    println!("Commits     : {}", commits.len());
    if !version_files.is_empty() || changelog_path.is_some() {
        println!("Files to update:");
        for file in &version_files {
            println!(
                "  {} : {} {} {}",
                file.path.display(),
                file.current_version,
                arrow(),
                new_version
            );
        }
        if let Some(ref path) = changelog_path {
            let status = if path.exists() { "prepend" } else { "create" };
            println!("  {} ({})", path.display(), status);
        }
    }
    println!("\nChangelog:");
    println!("{}", changelog);
    println!("===========================================================\n");

    // Step 9: Execute or dry-run
    if !apply {
        println!("Dry run complete. Use --apply to execute release.");
        return Ok(());
    }

    // Step 10: User confirmation
    print!("Proceed with release? [y/N]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Release cancelled.");
        return Ok(());
    }

    // Step 11: Execute release
    execute_release(
        &version_files,
        &new_version,
        &tag_name,
        &changelog,
        changelog_path.as_deref(),
        !apply,
    )?;

    println!("\n===========================================================");
    success(format!("Release {} completed successfully!", new_version));
    println!("===========================================================");
    println!("\nNext steps:");
    println!("  1. Review the changes: git show {}", tag_name);
    println!("  2. Push the release: git push origin main {}", tag_name);
    println!("  3. Create GitHub release with the changelog");
    println!();

    Ok(())
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn validate_bump_strategy(bump: &str) -> Result<()> {
    match bump {
        "auto" | "major" | "minor" | "patch" => Ok(()),
        _ => bail!(
            "Invalid bump strategy: {}. Must be one of: auto, major, minor, patch",
            bump
        ),
    }
}

async fn suggest_version_bump_llm(
    client: &LlmClient,
    from_ref: &str,
    to: Option<&str>,
    base_branch: &str,
    stream: bool,
    algo: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<String> {
    // Get the current version from version files
    let current = get_current_version();

    // Build diff target
    let diff_target = build_diff_target(Some(from_ref), to, base_branch);
    let diff_target_ref = if diff_target.is_empty() {
        None
    } else {
        Some(diff_target.as_str())
    };

    // Get the diff
    let raw_diff = get_diff(diff_target_ref, false, usize::MAX)?;

    if raw_diff.trim().is_empty() {
        return Ok("patch".to_string());
    }

    let context = AnalysisContext::new()
        .with_provider(client.provider())
        .with_model(client.model());

    // Apply smart diff shaping
    let diff = apply_smart_diff_with_context(
        &raw_diff,
        max_diff_chars,
        false,
        algo,
        Some(&context),
        secret_action,
    )?;

    // Build prompt
    let prompt = get_version_user()
        .replace("{version}", &current)
        .replace("{diff}", &diff);

    let (project_ctx, user_ctx) = load_all_context();
    let system = version_system_with_context(project_ctx.as_deref(), user_ctx.as_deref());

    // Call LLM
    let response = client.chat(&system, &prompt, stream).await?;
    if stream {
        println!();
    }

    // Parse the response to extract bump type
    parse_version_bump_from_response(&response)
}

fn parse_version_bump_from_response(response: &str) -> Result<String> {
    let lower = response.to_lowercase();

    // First, look for structured "Recommendation: <type>" format
    for line in lower.lines() {
        let line = line.trim();
        if line.starts_with("recommendation:") {
            let value = line.trim_start_matches("recommendation:").trim();
            if value.starts_with("major") {
                return Ok("major".to_string());
            } else if value.starts_with("minor") {
                return Ok("minor".to_string());
            } else if value.starts_with("patch") {
                return Ok("patch".to_string());
            }
        }
    }

    // Fallback: look for "recommend <type>" pattern
    if lower.contains("recommend major") || lower.contains("recommending major") {
        return Ok("major".to_string());
    }
    if lower.contains("recommend minor") || lower.contains("recommending minor") {
        return Ok("minor".to_string());
    }
    if lower.contains("recommend patch") || lower.contains("recommending patch") {
        return Ok("patch".to_string());
    }

    // Last resort: check for breaking changes indicator
    for line in lower.lines() {
        let line = line.trim();
        if line.starts_with("breaking:") {
            let value = line.trim_start_matches("breaking:").trim();
            if value.starts_with("yes") {
                return Ok("major".to_string());
            }
        }
    }

    // Default to patch (safest choice)
    eprintln!("Warning: Could not parse version bump from LLM response. Defaulting to 'patch'.");
    eprintln!("LLM response: {}", response);
    Ok("patch".to_string())
}

#[allow(dead_code)]
fn suggest_version_bump(commits: &[CommitInfo]) -> String {
    // Simple heuristic based on commit messages
    let has_breaking = commits.iter().any(|c| {
        c.subject.to_lowercase().contains("breaking")
            || c.subject.starts_with("!")
            || c.subject.contains("BREAKING CHANGE")
    });

    let has_feature = commits.iter().any(|c| {
        c.subject.to_lowercase().starts_with("feat")
            || c.subject.to_lowercase().starts_with("feature")
    });

    if has_breaking {
        "major".to_string()
    } else if has_feature {
        "minor".to_string()
    } else {
        "patch".to_string()
    }
}

fn compute_new_version(current: &str, bump: &str) -> Result<String> {
    // Parse version (supports X.Y.Z format)
    let parts: Vec<&str> = current.split('.').collect();
    if parts.len() != 3 {
        bail!("Current version '{}' is not in X.Y.Z format", current);
    }

    let major: u32 = parts[0].parse().context("Invalid major version")?;
    let minor: u32 = parts[1].parse().context("Invalid minor version")?;
    let patch: u32 = parts[2].parse().context("Invalid patch version")?;

    let (new_major, new_minor, new_patch) = match bump {
        "major" => (major + 1, 0, 0),
        "minor" => (major, minor + 1, 0),
        "patch" => (major, minor, patch + 1),
        _ => bail!("Invalid bump type: {}", bump),
    };

    Ok(format!("{}.{}.{}", new_major, new_minor, new_patch))
}

/// Format a changelog entry with version header and date
fn format_changelog_entry(version: &str, changelog_content: &str) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d");
    format!(
        "## [{}] - {}\n\n{}\n\n",
        version,
        date,
        changelog_content.trim()
    )
}

/// Find the position after the changelog header
/// Returns the byte offset where new content should be inserted
fn find_header_end(content: &str) -> Option<usize> {
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return None;
    }

    // Check if first line is a header
    let first_line = lines[0].trim().to_lowercase();
    if !first_line.starts_with("# changelog")
        && !first_line.starts_with("# change log")
        && !first_line.starts_with("# history")
    {
        return None;
    }

    // Find end of header section (after blank lines)
    let mut pos = lines[0].len() + 1; // +1 for newline
    for line in lines.iter().skip(1) {
        if line.trim().is_empty() {
            pos += line.len() + 1;
        } else {
            break;
        }
    }

    Some(pos)
}

/// Prepend a new changelog entry to the changelog file
///
/// Behavior:
/// - Creates file if it doesn't exist (with header "# Changelog\n\n")
/// - Preserves existing header (lines starting with "# Changelog" or similar)
/// - Inserts new entry after header, before existing entries
fn prepend_to_changelog_file(path: &Path, entry: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would update {} with new changelog entry", path.display());
        return Ok(());
    }

    let content = if path.exists() {
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?
    } else {
        String::new()
    };

    let new_content = if content.is_empty() {
        // Create new file with header
        format!("# Changelog\n\n{}", entry)
    } else if let Some(header_end) = find_header_end(&content) {
        // Insert after header
        format!(
            "{}{}{}",
            &content[..header_end],
            entry,
            &content[header_end..]
        )
    } else {
        // No recognizable header, prepend directly
        format!("{}{}", entry, content)
    };

    fs::write(path, new_content).with_context(|| format!("Failed to write {}", path.display()))?;
    success(format!("Updated {}", path.display()));
    Ok(())
}

async fn generate_changelog_content(
    commits: &[CommitInfo],
    from_ref: &str,
    version: &str,
    client: &LlmClient,
    stream: bool,
    algo: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<String> {
    // Use LLM-powered changelog generation (same logic as cmd_changelog)
    let end = "HEAD";
    let display = format!("{}..{}", from_ref, end);

    // Build commit list with messages (same format as cmd_changelog)
    let commit_list = commits
        .iter()
        .map(|c| {
            format!(
                "- [{}] {}",
                &c.short_hash[..SHORT_HASH_LEN.min(c.short_hash.len())],
                c.subject
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let context = AnalysisContext::new()
        .with_provider(client.provider())
        .with_model(client.model());

    // Get combined diff for the range
    let raw_diff = get_diff(Some(&format!("{}..{}", from_ref, end)), false, usize::MAX)?;
    let diff = if raw_diff.trim().is_empty() {
        String::new()
    } else {
        apply_smart_diff_with_context(
            &raw_diff,
            max_diff_chars,
            false,
            algo,
            Some(&context),
            secret_action,
        )?
    };

    // Build the prompt using the same template as cmd_changelog
    // Include version context for release notes
    let range_with_version = format!("{} (Release v{})", display, version);
    let prompt = get_changelog_user()
        .replace("{range}", &range_with_version)
        .replace("{count}", &commits.len().to_string())
        .replace("{commits}", &commit_list)
        .replace("{diff}", &diff);

    let (project_ctx, user_ctx) = load_all_context();
    let system = changelog_system_with_context(project_ctx.as_deref(), user_ctx.as_deref());

    // Call LLM to generate the changelog
    let changelog = client.chat(&system, &prompt, stream).await?;
    if stream {
        println!();
    }

    Ok(changelog)
}

fn execute_release(
    version_files: &[VersionFile],
    new_version: &str,
    tag_name: &str,
    changelog: &str,
    changelog_file_path: Option<&Path>,
    dry_run: bool,
) -> Result<()> {
    println!("\nExecuting release...\n");

    // Step 1: Update version files
    if !version_files.is_empty() {
        for file in version_files {
            update_version_file(file, new_version, dry_run)?;
        }
    }

    // Step 2: Update changelog file
    if let Some(path) = changelog_file_path {
        let entry = format_changelog_entry(new_version, changelog);
        prepend_to_changelog_file(path, &entry, dry_run)?;
    }

    // Step 3: Stage and commit all changes
    if !dry_run {
        println!("Staging files for release commit...");

        // Stage the changelog file if it exists (might be new/untracked)
        if let Some(path) = changelog_file_path {
            if path.exists() {
                println!("  Adding: {}", path.display());
                git::run_git(&["add", path.to_str().unwrap()])?;
            }
        }

        // Commit with -a to include all tracked modified files (like Cargo.toml)
        println!("Creating release commit...");
        let commit_message = format!("Release version {}", new_version);
        git::run_git(&["commit", "-a", "-m", &commit_message])?;
        success("Created release commit");
        println!();
    }

    // Step 4: Create annotated tag
    println!("Creating annotated tag...");
    create_tag(tag_name, changelog, dry_run)?;

    Ok(())
}
